/*!

Support for reading MBR (not GPT) partition tables, and getting an `io::Read` for a partition.
*/

use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Result;
use std::io::Seek;

use byteorder::{ByteOrder, LittleEndian};

use rangereader::RangeReader;

/// An entry in the partition table.
#[derive(Debug)]
pub struct Partition {
    pub id: usize,
    pub bootable: bool,
    pub type_code: u8,
    pub first_byte: u64,
    pub len: u64,
}

/// Read a DOS/MBR partition table from a reader positioned at the appropriate sector.
/// The sector size for the disc is assumed to be 512 bytes.
pub fn read_partition_table<R: Read>(mut reader: R) -> Result<Vec<Partition>> {
    let mut sector = [0u8; 512];
    reader.read_exact(&mut sector)?;

    parse_partition_table(&sector, 512)
}

/// Read a DOS/MBR partition table from a 512-byte boot sector, providing a disc sector size.
pub fn parse_partition_table(sector: &[u8], sector_size: u16) -> Result<Vec<Partition>> {
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

        let first_byte = LittleEndian::read_u32(&partition[8..]) as u64 * sector_size as u64;
        let len = first_byte + LittleEndian::read_u32(&partition[12..]) as u64 * sector_size as u64;

        partitions.push(Partition {
            id: entry_id,
            bootable,
            type_code,
            first_byte,
            len,
        });
    }

    Ok(partitions)
}

/// Open the contents of a partition for reading.
pub fn read_partition<R>(inner: R, part: &Partition) -> Result<RangeReader<R>>
where
    R: Read + Seek,
{
    RangeReader::new(inner, part.first_byte, part.len)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse() {
        let parts = ::mbr::parse_partition_table(
            include_bytes!("test-data/mbr-ubuntu-raspi3-16.04.img"),
            512,
        ).expect("success");

        assert_eq!(2, parts.len());

        assert_eq!(0, parts[0].id);
        assert_eq!(true, parts[0].bootable);
        assert_eq!(12, parts[0].type_code);
        assert_eq!(4194304, parts[0].first_byte);
        assert_eq!(138412032, parts[0].len);

        assert_eq!(1, parts[1].id);
        assert_eq!(false, parts[1].bootable);
        assert_eq!(131, parts[1].type_code);
        assert_eq!(138412032, parts[1].first_byte);
        assert_eq!(3999268864, parts[1].len);
    }
}
