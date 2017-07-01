use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;

use byteorder::{ByteOrder, LittleEndian};

use Partition;

const SECTOR_SIZE: usize = 512;

/// Read a DOS/MBR partition table from a 512-byte boot sector, providing a disc sector size.
pub fn parse_partition_table(sector: &[u8; SECTOR_SIZE]) -> Result<Vec<Partition>> {
    let mut partitions = Vec::with_capacity(4);

    for entry_id in 0..4 {
        let first_entry_offset = 446;
        let entry_size = 16;
        let entry_offset = first_entry_offset + entry_id * entry_size;
        let partition = &sector[entry_offset..entry_offset + entry_size];
        let status = partition[0];
        let bootable = match status {
            0x00 => false,
            0x80 => true,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "invalid status code in partition {}: {:x}",
                        entry_id,
                        status
                    ),
                ))
            }
        };

        let type_code = partition[4];

        if 0 == type_code {
            continue;
        }

        let first_byte = LittleEndian::read_u32(&partition[8..]) as u64 * SECTOR_SIZE as u64;
        let len = first_byte + LittleEndian::read_u32(&partition[12..]) as u64 * SECTOR_SIZE as u64;

        partitions.push(Partition {
            id: entry_id,
            first_byte,
            len,
            attributes: ::Attributes::MBR {
                type_code,
                bootable,
            },
        });
    }

    Ok(partitions)
}
