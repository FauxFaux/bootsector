use core::convert::TryFrom;
use core::convert::TryInto;
use std::io;

use crc::Crc;
use snafu::ResultExt;

use crate::errors::IoSnafu;
use crate::{le, Attributes, Error, Partition};

// Apparently we have to pick a name from a random page on sourceforge.
// Random sourceforge page: https://reveng.sourceforge.io/crc-catalogue/all.htm

// There's no clarification on *which* "crc32" to use in the GPT spec.
// OSDev: > For the checksums in the header, the CCITT32 ANSI CRC method is used, the one with the polynomial 0x04c11db7
//          (same as in gzip, and different to the Castagnoli CRC32 [...]
// "CCITT" is apparently the French name for ITU-T.

// Random sourceforge page:
// > Alias: CRC-32, CRC-32/ADCCP, CRC-32/V-42, CRC-32/XZ, PKZIP. HDLC is defined in ISO/IEC 13239.
// > ITU-T Recommendation V.42 (March 2002). "HDLC" is some networking thing; why not, eh.

// (and the values check out)
const CRC: Crc<u32> = Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);

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

pub fn read<R>(mut reader: R, sector_size: u64) -> Result<Vec<Partition>, Error>
where
    R: io::Read + io::Seek,
{
    reader
        .seek(io::SeekFrom::Start(sector_size))
        .context(IoSnafu {})?;

    let sector_size_mem = usize::try_from(sector_size).map_err(|_| Error::BiggerThanMemory)?;

    let mut lba1 = vec![0u8; sector_size_mem];
    reader.read_exact(&mut lba1).context(IoSnafu {})?;

    if b"EFI PART" != &lba1[0x00..0x08] {
        return Err(Error::InvalidStatic {
            message: "bad EFI signature",
        });
    }

    if [0, 0, 1, 0] != lba1[0x08..0x0c] {
        return Err(Error::InvalidStatic {
            message: "unsupported revision",
        });
    }

    let header_size = le::read_u32(&lba1[0x0c..0x10]);
    if header_size < 92 {
        return Err(Error::InvalidStatic {
            message: "header too short",
        });
    }

    let header_size = usize::try_from(header_size).map_err(|_| Error::InvalidStatic {
        message: "header size must fit in memory",
    })?;

    let header_crc = le::read_u32(&lba1[0x10..0x14]);

    // CRC is calculated with the CRC zero'd out
    for crc_part in 0x10..0x14 {
        lba1[crc_part] = 0;
    }

    if header_crc != CRC.checksum(&lba1[..header_size]) {
        return Err(Error::InvalidStatic {
            message: "header checksum mismatch",
        });
    }

    if 0 != le::read_u32(&lba1[0x14..0x18]) {
        return Err(Error::InvalidStatic {
            message: "unsupported data in reserved field 0x0c",
        });
    }

    if 1 != le::read_u64(&lba1[0x18..0x20]) {
        return Err(Error::InvalidStatic {
            message: "current lba must be '1' for first header",
        });
    }

    // backup lba [ignored]

    let first_usable_lba = le::read_u64(&lba1[0x28..0x30]);
    let last_usable_lba = le::read_u64(&lba1[0x30..0x38]);

    if first_usable_lba > last_usable_lba {
        return Err(Error::InvalidStatic {
            message: "usable lbas are backwards?!",
        });
    }

    if last_usable_lba > (u64::MAX / sector_size) {
        return Err(Error::InvalidStatic {
            message: "everything must be below the 2^64 point (~ eighteen million TB)",
        });
    }

    let mut guid = [0u8; 16];
    guid.copy_from_slice(&lba1[0x38..0x48]);

    if 2 != le::read_u64(&lba1[0x48..0x50]) {
        return Err(Error::InvalidStatic {
            message: "starting lba must be '2' for first header",
        });
    }

    let entries = le::read_u32(&lba1[0x50..0x54]);

    let entries = u16::try_from(entries).map_err(|_| Error::InvalidStatic {
        message: "entry count is implausible",
    })?;

    let entry_size = le::read_u32(&lba1[0x54..0x58]);
    let entry_size = u16::try_from(entry_size).map_err(|_| Error::InvalidStatic {
        message: "entry size is implausibly large",
    })?;

    if entry_size < 128 {
        return Err(Error::InvalidStatic {
            message: "entry size is implausibly small",
        });
    }

    // TODO: off-by-1? Not super important.
    if first_usable_lba < 2 + ((u64::from(entry_size) * u64::from(entries)) / sector_size) {
        return Err(Error::InvalidStatic {
            message: "first usable lba is too low",
        });
    }

    let table_crc = le::read_u32(&lba1[0x58..0x5c]);

    if !all_zero(&lba1[header_size..]) {
        return Err(Error::InvalidStatic {
            message: "reserved header tail is not all empty",
        });
    }

    let mut table = vec![0u8; usize::from(entry_size) * usize::from(entries)];
    reader.read_exact(&mut table).context(IoSnafu {})?;

    if table_crc != CRC.checksum(&table) {
        return Err(Error::InvalidStatic {
            message: "table crc invalid",
        });
    }

    let mut ret = Vec::with_capacity(16);
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
            return Err(Error::InvalidStatic {
                message: "partition entry is out of range",
            });
        }

        let attributes = entry[0x30..0x38].try_into().expect("fixed size slice");
        let name_data = &entry[0x38..0x80];
        let name_le: Vec<u16> = (0..(0x80 - 0x38) / 2)
            .map(|idx| le::read_u16(&name_data[2 * idx..2 * (idx + 1)]))
            .take_while(|val| 0 != *val)
            .collect();

        let name = match String::from_utf16(&name_le) {
            Ok(name) => name,
            Err(e) => {
                return Err(Error::InvalidData {
                    message: format!("partition {} has an invalid name: {:?}", id, e),
                });
            }
        };

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
