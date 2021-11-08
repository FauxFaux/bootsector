#[cfg(feature = "std")]
use std::io::{self, Error, ErrorKind::InvalidData};

use crc::{Crc, CRC_32_ISO_HDLC};
use smallvec::{smallvec, SmallVec};

use crate::le;
use crate::no_std::convert::{TryFrom, TryInto};
use crate::read::{Read, Seek, SeekFrom};
use crate::Attributes;
use crate::Error;
use crate::Partition;
use crate::MAX_PARTITIONS;

const MAX_SECTOR_SIZE: usize = 520;

pub fn is_protective(partition: &Partition) -> bool {
    const MAXIMUM_SECTOR_SIZE: u64 = 16 * 1024;
    const PROTECTIVE_TYPE: u8 = 0xee;

    match partition.attributes {
        Attributes::MBR {
            type_code,
            bootable: false,
        } if type_code == PROTECTIVE_TYPE => {}
        _ => return false,
    };

    0 == partition.id && partition.first_byte <= MAXIMUM_SECTOR_SIZE
}

#[cfg(feature = "std")]
pub fn read<R>(reader: &mut R, sector_size: u64) -> io::Result<Vec<Partition>>
where
    R: io::Read + io::Seek,
{
    read_(reader, sector_size)
        .map(|r| r.into_vec())
        .map_err(|e| e.into())
}

#[cfg(not(feature = "std"))]
pub fn read<R>(
    reader: &mut R,
    sector_size: u64,
) -> Result<SmallVec<[Partition; MAX_PARTITIONS]>, Error<R::Error>>
where
    R: Read + Seek,
{
    read_(reader, sector_size)
}

fn read_<R>(
    reader: &mut R,
    sector_size: u64,
) -> Result<SmallVec<[Partition; MAX_PARTITIONS]>, Error<R::Error>>
where
    R: Read + Seek,
{
    reader.seek(SeekFrom::Start(sector_size))?;

    let sector_size_mem =
        usize::try_from(sector_size).map_err(|_| GptError::SectorSizeBiggerThanMemory)?;

    assert!(
        sector_size_mem <= MAX_SECTOR_SIZE,
        "MAX_SECTOR_SIZE is not large enough"
    );

    let mut lba1 = [0u8; MAX_SECTOR_SIZE];
    reader.read_exact(&mut lba1[..sector_size as usize])?;

    if b"EFI PART" != &lba1[0x00..0x08] {
        return Err(GptError::BadEFISignature).map_err(Error::Gpt);
    }

    if [0, 0, 1, 0] != lba1[0x08..0x0c] {
        return Err(GptError::UnsupportedRevision).map_err(Error::Gpt);
    }

    let header_size = le::read_u32(&lba1[0x0c..0x10]);
    if header_size < 92 {
        return Err(GptError::HeaderTooShort).map_err(Error::Gpt);
    }

    let header_size = usize::try_from(header_size)
        .map_err(|_| GptError::HeaderSizeMustFitInMemory)
        .map_err(Error::Gpt)?;

    let header_crc = le::read_u32(&lba1[0x10..0x14]);

    // CRC is calculated with the CRC zero'd out
    for crc_part in 0x10..0x14 {
        lba1[crc_part] = 0;
    }

    if header_crc != Crc::<u32>::new(&CRC_32_ISO_HDLC).checksum(&lba1[..header_size]) {
        return Err(GptError::HeaderChecksumMismatch).map_err(Error::Gpt);
    }

    if 0 != le::read_u32(&lba1[0x14..0x18]) {
        return Err(GptError::UnsupportedDataInReservedField).map_err(Error::Gpt);
    }

    if 1 != le::read_u64(&lba1[0x18..0x20]) {
        return Err(GptError::CurrentLbaMustBeOneInFirstHeader).map_err(Error::Gpt);
    }

    // backup lba [ignored]

    let first_usable_lba = le::read_u64(&lba1[0x28..0x30]);
    let last_usable_lba = le::read_u64(&lba1[0x30..0x38]);

    if first_usable_lba > last_usable_lba {
        return Err(GptError::BackwardLbas).map_err(Error::Gpt);
    }

    if last_usable_lba
        > (crate::no_std::u64::MAX / u64::try_from(sector_size).expect("u64 conversion"))
    {
        return Err(GptError::EverythingMustBeBelowSixtyFourToThePowerOfTwo).map_err(Error::Gpt);
    }

    let mut guid = [0u8; 16];
    guid.copy_from_slice(&lba1[0x38..0x48]);

    if 2 != le::read_u64(&lba1[0x48..0x50]) {
        return Err(GptError::StartingLbaMustBeTwo).map_err(Error::Gpt);
    }

    let entries = le::read_u32(&lba1[0x50..0x54]);

    let entries = u16::try_from(entries)
        .map_err(|_| GptError::EntryCountIsImplausible)
        .map_err(Error::Gpt)?;

    let entry_size = le::read_u32(&lba1[0x54..0x58]);
    let entry_size = u16::try_from(entry_size)
        .map_err(|_| GptError::EntrySizeIsImplausiblyLarge)
        .map_err(Error::Gpt)?;

    if entry_size < 128 {
        return Err(GptError::EntrySizeIsImplausiblySmall).map_err(Error::Gpt);
    }

    // TODO: off-by-1? Not super important.
    if first_usable_lba < 2 + ((u64::from(entry_size) * u64::from(entries)) / sector_size) {
        return Err(GptError::FirstUsableLbaIsTooLow).map_err(Error::Gpt);
    }

    let table_crc = le::read_u32(&lba1[0x58..0x5c]);

    if !all_zero(&lba1[header_size..]) {
        return Err(GptError::ReservedHeaderTailIsNotAllEmpty).map_err(Error::Gpt);
    }

    // Partitions are stored on LBA2-33
    // 32 * MAX_SECTOR_SIZE(520) = 16640
    // We need to align that to the closest backing array: 24576
    let mut table: SmallVec<[_; 24576]> = smallvec![0u8; 24576];
    table.truncate(usize::from(entry_size) * usize::from(entries));

    reader.read_exact(&mut table)?;

    if table_crc != Crc::<u32>::new(&CRC_32_ISO_HDLC).checksum(&table) {
        return Err(GptError::InvalidTableCRC(table.len())).map_err(Error::Gpt);
    }

    let mut ret = SmallVec::with_capacity(MAX_PARTITIONS);
    for id in 0..usize::from(entries) {
        let entry_size = usize::from(entry_size);
        let entry = &table[id * entry_size..(id + 1) * entry_size];
        let type_uuid = &entry[0x00..0x10];
        if all_zero(type_uuid) {
            continue;
        }

        let type_uuid = type_uuid.try_into().expect("fixed size slice");

        let partition_uuid = entry[0x10..0x20].try_into().expect("fixed sized slice");
        let first_lba = le::read_u64(&entry[0x20..0x28]);
        let last_lba = le::read_u64(&entry[0x28..0x30]);

        if first_lba > last_lba || first_lba < first_usable_lba || last_lba > last_usable_lba {
            return Err(GptError::PartitionEntryOutOfRange).map_err(Error::Gpt);
        }

        let attributes = entry[0x30..0x38].try_into().expect("fixed size slice");
        let name_data = &entry[0x38..0x80];
        let name_le: SmallVec<[u16; 36]> = (0..(0x80 - 0x38) / 2)
            .map(|idx| le::read_u16(&name_data[2 * idx..2 * (idx + 1)]))
            .take_while(|val| 0 != *val)
            .collect();

        let name = name_le;

        ret.push(Partition {
            id,
            first_byte: first_lba * sector_size,
            len: (last_lba - first_lba + 1) * sector_size,
            attributes: Attributes::GPT {
                type_uuid,
                partition_uuid,
                attributes,
                name,
            },
        });
    }

    Ok(ret)
}

fn all_zero(val: &[u8]) -> bool {
    val.iter().all(|x| 0 == *x)
}

#[cfg_attr(feature = "std", derive(Error))]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum GptError {
    #[cfg_attr(feature = "std", error("sector size is bigger than memory"))]
    SectorSizeBiggerThanMemory,
    #[cfg_attr(feature = "std", error("bad EFI signature"))]
    BadEFISignature,
    #[cfg_attr(feature = "std", error("unsupported revision"))]
    UnsupportedRevision,
    #[cfg_attr(feature = "std", error("header too short"))]
    HeaderTooShort,
    #[cfg_attr(feature = "std", error("header size must fit in memory"))]
    HeaderSizeMustFitInMemory,
    #[cfg_attr(feature = "std", error("header checksum mismatch"))]
    HeaderChecksumMismatch,
    #[cfg_attr(feature = "std", error("unsupported data in reserved field 0x0c"))]
    UnsupportedDataInReservedField,
    #[cfg_attr(feature = "std", error("current lba must be '1' for first header"))]
    CurrentLbaMustBeOneInFirstHeader,
    #[cfg_attr(feature = "std", error("usable lbas are backwards?!"))]
    BackwardLbas,
    #[cfg_attr(
        feature = "std",
        error("everything must be below the 2^64 point (~ eighteen million TB)")
    )]
    EverythingMustBeBelowSixtyFourToThePowerOfTwo,
    #[cfg_attr(feature = "std", error("starting lba must be '2' for first header"))]
    StartingLbaMustBeTwo,
    #[cfg_attr(feature = "std", error("entry count is implausible"))]
    EntryCountIsImplausible,
    #[cfg_attr(feature = "std", error("entry size is implausibly large"))]
    EntrySizeIsImplausiblyLarge,
    #[cfg_attr(feature = "std", error("entry size is implausibly small"))]
    EntrySizeIsImplausiblySmall,
    #[cfg_attr(feature = "std", error("first usable lba is too low"))]
    FirstUsableLbaIsTooLow,
    #[cfg_attr(feature = "std", error("reserved header tail is not all empty"))]
    ReservedHeaderTailIsNotAllEmpty,
    #[cfg_attr(feature = "std", error("table crc invalid: {}"))]
    InvalidTableCRC(usize),
    #[cfg_attr(feature = "std", error("partition entry is out of range"))]
    PartitionEntryOutOfRange,
}

#[cfg(feature = "std")]
impl Into<io::Error> for GptError {
    fn into(e: Self) -> io::Error {
        Error::new(InvalidData, format!("{}", e))
    }
}
