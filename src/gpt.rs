use std;
use std::io;

use std::io::Error;
use std::io::ErrorKind::InvalidData;

use byteorder::{ByteOrder, LittleEndian};

use crc::crc32::checksum_ieee;

use ::Attributes;
use ::Partition;

pub fn protective(partition: &Partition) -> bool {
    let protective_type = 0xee;
    let maximum_sector_size = 16 * 1024;
    let sector_size_guess = partition.first_byte;
    let minimum_gpt_length = 128 * 128 + sector_size_guess;

    match partition.attributes {
        Attributes::MBR {
            type_code,
            bootable: false,
        } if type_code == protective_type => {}
        _ => return false,
    };

    0 == partition.id &&
        partition.first_byte <= maximum_sector_size &&
        partition.len >= minimum_gpt_length
}

pub fn read<R>(mut reader: R, sector_size: usize) -> io::Result<Vec<Partition>>
    where R: io::Read + io::Seek {

    reader.seek(io::SeekFrom::Start(sector_size as u64))?;

    let mut lba1 = vec![0u8; sector_size];
    reader.read_exact(&mut lba1)?;

    if b"EFI PART" != &lba1[0x00..0x08] {
        return Err(Error::new(InvalidData, "bad EFI signature"));
    }

    if &[0, 0, 1, 0] != &lba1[0x08..0x0c] {
        return Err(Error::new(InvalidData, "unsupported revision"));
    }

    let header_size = LittleEndian::read_u32(&lba1[0x0c..0x10]);
    if header_size < 92 {
        return Err(Error::new(InvalidData, "header too short"));
    }

    let header_crc = LittleEndian::read_u32(&lba1[0x10..0x14]);

    // CRC is calculated with the CRC zero'd out
    for crc_part in 0x10..0x14 {
        lba1[crc_part] = 0;
    }

    if header_crc != checksum_ieee(&lba1[..header_size as usize]) {
        return Err(Error::new(InvalidData, "header checksum mismatch"));
    }

    if 0 != LittleEndian::read_u32(&lba1[0x14..0x18]) {
        return Err(Error::new(InvalidData, "unsupported data in reserved field 0x0c"));
    }

    if 1 != LittleEndian::read_u64(&lba1[0x18..0x20]) {
        return Err(Error::new(InvalidData, "current lba must be '1' for first header"));
    }

    // backup lba [ignored]

    let first_usable_lba = LittleEndian::read_u64(&lba1[0x28..0x30]);
    let last_usable_lba = LittleEndian::read_u64(&lba1[0x30..0x38]);

    if first_usable_lba > last_usable_lba {
        return Err(Error::new(InvalidData, "usable lbas are backwards?!"));
    }

    if last_usable_lba > (std::u64::MAX / sector_size as u64) {
        return Err(Error::new(InvalidData,
                              "everything must be below the 2^64 point (~ eighteen million TB)"));
    }

    let mut guid = [0u8; 16];
    guid.copy_from_slice(&lba1[0x38..0x48]);

    if 2 != LittleEndian::read_u64(&lba1[0x48..0x50]) {
        return Err(Error::new(InvalidData, "starting lba must be '2' for first header"));
    }

    let entries = LittleEndian::read_u32(&lba1[0x50..0x54]);

    if entries >= 65536 {
        return Err(Error::new(InvalidData, "entry count is implausible"));
    }

    let entry_size = LittleEndian::read_u32(&lba1[0x54..0x58]);
    if entry_size < 128 || entry_size >= 65536 {
        return Err(Error::new(InvalidData, "entry size is implausible"));
    }

    let entries = entries as usize;
    let entry_size = entry_size as usize;

    // TODO: off-by-1? Not super important.
    if first_usable_lba < 2 + ((entry_size * entries) / sector_size) as u64 {
        return Err(Error::new(InvalidData, "first usable lba is too low"));
    }

    let table_crc = LittleEndian::read_u32(&lba1[0x58..0x5c]);

    if !all_zero(&lba1[header_size as usize..]) {
        return Err(Error::new(InvalidData, "reserved header tail is not all empty"));
    }

    let mut table = vec![0u8; entry_size * entries];
    reader.read_exact(&mut table)?;

    if table_crc != checksum_ieee(&table) {
        return Err(Error::new(InvalidData, "table crc invalid"));
    }

    let mut ret = Vec::with_capacity(16);
    for id in 0..entries {
        let entry = &table[id * entry_size..(id + 1) * entry_size];
        let type_uuid = &entry[0x00..0x10];
        if all_zero(type_uuid) {
            continue;
        }

        let type_uuid = clone_into_array(type_uuid);

        let partition_uuid = clone_into_array(&entry[0x10..0x20]);
        let first_lba = LittleEndian::read_u64(&entry[0x20..0x28]);
        let last_lba = LittleEndian::read_u64(&entry[0x28..0x30]);

        if first_lba > last_lba || first_lba < first_usable_lba || last_lba > last_usable_lba {
            return Err(Error::new(InvalidData, "partition entry is out of range"));
        }

        let attributes = clone_into_array(&entry[0x30..0x38]);
        let name_data = &entry[0x38..0x80];
        let name_le: Vec<u16> = (0..32)
            .map(|idx| LittleEndian::read_u16(&name_data[idx..]))
            .take_while(|val| 0 != *val)
            .collect();

        let name = match String::from_utf16(&name_le) {
            Ok(name) => name,
            Err(e) => {
                return Err(Error::new(InvalidData, format!("partition {} has an invalid name: {:?}", id, e)));
            }
        };

        ret.push(Partition {
            id,
            first_byte: first_lba * sector_size as u64,
            len: (last_lba - first_lba + 1) * sector_size as u64,
            attributes: Attributes::GPT {
                type_uuid,
                partition_uuid,
                attributes,
                name,
            }
        });
    }

    Ok(ret)
}

fn all_zero(val: &[u8]) -> bool {
    val.iter().all(|x| 0 == *x)
}

use std::convert::AsMut;

/// https://stackoverflow.com/questions/37678698/function-to-build-a-fixed-sized-array-from-slice/37679019#37679019
fn clone_into_array<A, T>(slice: &[T]) -> A
    where A: Sized + Default + AsMut<[T]>,
          T: Clone
{
    let mut a = Default::default();
    <A as AsMut<[T]>>::as_mut(&mut a).clone_from_slice(slice);
    a
}
