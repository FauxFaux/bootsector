use crate::no_std::convert::TryFrom;
#[cfg(feature = "std")]
use std::io::{self, Error, ErrorKind::InvalidData};

use smallvec::SmallVec;

use crate::le;
use crate::Partition;
use crate::MAX_PARTITIONS;

const SECTOR_SIZE: usize = 512;

/// Read a DOS/MBR partition table from a 512-byte boot sector, providing a disc sector size.
#[cfg(feature = "std")]
pub fn parse_partition_table(sector: &[u8; SECTOR_SIZE]) -> io::Result<Vec<Partition>> {
    parse_partition_table_(sector)
        .map(|o| o.into_vec())
        .map_err(|e| e.into())
}

/// Read a DOS/MBR partition table from a 512-byte boot sector, providing a disc sector size.
#[cfg(not(feature = "std"))]
pub fn parse_partition_table(
    sector: &[u8; SECTOR_SIZE],
) -> Result<SmallVec<[Partition; MAX_PARTITIONS]>, MbrError> {
    parse_partition_table_(sector)
}

fn parse_partition_table_(
    sector: &[u8; SECTOR_SIZE],
) -> Result<SmallVec<[Partition; MAX_PARTITIONS]>, MbrError> {
    let mut partitions = SmallVec::<[_; MAX_PARTITIONS]>::with_capacity(4);

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
                return Err(MbrError::InvalidStatusCode { entry_id, status });
            }
        };

        let type_code = partition[4];

        if 0 == type_code {
            continue;
        }

        let sector_size = u64::try_from(SECTOR_SIZE).expect("u64 constant");
        let first_byte = u64::from(le::read_u32(&partition[8..])) * sector_size;
        let len = u64::from(le::read_u32(&partition[12..])) * sector_size;

        partitions.push(Partition {
            id: entry_id,
            first_byte,
            len,
            attributes: crate::Attributes::MBR {
                type_code,
                bootable,
            },
        });
    }

    Ok(partitions)
}

#[cfg_attr(feature = "std", derive(Error))]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum MbrError {
    #[cfg_attr(
        feature = "std",
        error("invalid status code in partition {entry_id}: {status:x}")
    )]
    InvalidStatusCode { entry_id: usize, status: u8 },
}

#[cfg(feature = "std")]
impl Into<io::Error> for MbrError {
    fn into(e: Self) -> io::Error {
        Error::new(InvalidData, format!("{}", e))
    }
}
