#![cfg_attr(not(feature = "std"), no_std)]
#![warn(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
//! Read basic MBR and GPT partition tables from a reader.
//!
//! # Examples
//!
//! Load MBR or GPT partitions from a `reader`:
//!
//! ```rust
//! use std::io;
//! use bootsector::{list_partitions, open_partition, Options, Attributes};
//!
//! # fn go<R>(mut reader: R) -> io::Result<()>
//! # where R: io::Read + io::Seek {
//! // let reader = ...;
//! let partitions = list_partitions(&mut reader, &Options::default())?;
//! let part = &partitions[0];
//!
//! // See what type of partition this is
//! match part.attributes {
//!     Attributes::GPT {
//!         type_uuid,
//!         ..
//!     } => println!("gpt: {:?}", type_uuid),
//!     Attributes::MBR {
//!         type_code,
//!         ..
//!     } => println!("mbr: {:x}", type_code),
//! }
//!
//! let part_reader = open_partition(reader, part);
//! // part_reader.read_exact(...
//! # Ok(())
//! # }
//! ```

mod gpt;
mod le;
mod mbr;
#[doc(hidden)]
pub(crate) mod no_std;
mod rangereader;
pub mod read;

use smallvec::SmallVec;

use read::{Read, Seek, SeekFrom};

pub use rangereader::{RangeError, RangeReader};

/// Maximum number of partition amongst mbr (4) and gpt (16)
const MAX_PARTITIONS: usize = 16;

/// Table-specific information about a partition.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Attributes {
    MBR {
        bootable: bool,
        type_code: u8,
    },
    GPT {
        type_uuid: [u8; 16],
        partition_uuid: [u8; 16],
        attributes: [u8; 8],
        name: SmallVec<[u16; 36]>,
    },
}

/// An entry in the partition table.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Partition {
    /// The number of this partition, 0-indexed.
    pub id: usize,

    /// The first byte of the reader that this partition represents.
    pub first_byte: u64,

    /// The length of this partition, in bytes.
    pub len: u64,

    /// Table-specific attributes about this partition.
    pub attributes: Attributes,
}

/// What type of MBR partition tables should we attempt to read?
pub enum ReadMBR {
    /// A compliant, modern MBR: CHS addressing is correctly set to the blind value.
    Modern,
    /// Require there to be a GPT partition present. The protective MBR is allowed, but ignored.
    Never,
}

/// What type of GPT partition tables should we attempt to read?
pub enum ReadGPT {
    /// A valid GPT partition table as of revision 1 (2010-2017 and counting)
    RevisionOne,

    /// Require that there be an MBR partition present. The protective MBR will be read literally.
    Never,
}

/// Settings for handling sector size
pub enum SectorSize {
    /// Attempt to identify a valid GPT partition table at various locations, and use this
    /// information to derive the sector size. For MBR, it's very likely that 512 is a safe
    /// assumption.
    GuessOrAssume,

    /// Use a specific known sector size.
    Known(u16),
}

/// Configuration for listing partitions.
pub struct Options {
    /// What type of MBR partitions should we read?
    pub mbr: ReadMBR,

    /// What type of GPT partitions should we read?
    pub gpt: ReadGPT,

    /// How should we handle sector sizes?
    pub sector_size: SectorSize,
}

impl Default for Options {
    /// The default options are to read any type of modern partition table,
    /// having guessed the sector size.
    fn default() -> Self {
        Options {
            mbr: ReadMBR::Modern,
            gpt: ReadGPT::RevisionOne,
            sector_size: SectorSize::GuessOrAssume,
        }
    }
}

/// Read the list of partitions.
///
/// # Returns
///
/// * A possibly empty list of partitions.
/// * `ErrorKind::NotFound` if the boot magic is not found,
///        or you asked for partition types that are not there
/// * `ErrorKind::InvalidData` if anything is not as we expect,
///       including it looking like there should be GPT but its magic is missing.
/// * Other IO errors directly from the underlying reader, including `UnexpectedEOF`.
#[cfg(feature = "std")]
pub fn list_partitions<R>(
    reader: &mut R,
    options: &Options,
) -> Result<Vec<Partition>, Error<R::Error>>
where
    R: Read + Seek,
{
    list_partitions_(reader, options).map(|o| o.into_vec())
}

/// Read the list of partitions.
///
/// # Returns
///
/// * A possibly empty list of partitions.
/// * `ErrorKind::NotFound` if the boot magic is not found,
///        or you asked for partition types that are not there
/// * `ErrorKind::InvalidData` if anything is not as we expect,
///       including it looking like there should be GPT but its magic is missing.
/// * Other IO errors directly from the underlying reader, including `UnexpectedEOF`.
#[cfg(not(feature = "std"))]
pub fn list_partitions<R>(
    reader: &mut R,
    options: &Options,
) -> Result<SmallVec<[Partition; MAX_PARTITIONS]>, Error<R::Error>>
where
    R: Read + Seek,
{
    list_partitions_(reader, options)
}

fn list_partitions_<R>(
    reader: &mut R,
    options: &Options,
) -> Result<SmallVec<[Partition; MAX_PARTITIONS]>, Error<R::Error>>
where
    R: Read + Seek,
{
    let header_table = {
        reader.seek(SeekFrom::Start(0)).map(Error::Io)?;

        let mut disc_header = [0u8; 512];
        reader.read_exact(&mut disc_header).map(Error::Io)?;

        if 0x55 != disc_header[510] || 0xAA != disc_header[511] {
            return Err(Error::HeaderNotFound);
        }

        mbr::parse_partition_table(&disc_header)?
    };

    match header_table.len() {
        1 if gpt::is_protective(&header_table[0]) => {}
        _ => {
            return match options.mbr {
                ReadMBR::Modern => Ok(header_table),
                ReadMBR::Never => Err(Error::HeaderNotFound),
            }
        }
    }

    match options.gpt {
        ReadGPT::Never => Ok(header_table),
        ReadGPT::RevisionOne => {
            let sector_size = match options.sector_size {
                SectorSize::Known(size) => u64::from(size),
                SectorSize::GuessOrAssume => header_table[0].first_byte,
            };

            gpt::read(reader, sector_size).map_err(|e| e.into())
        }
    }
}

/// Open the contents of a partition for reading.
pub fn open_partition<R>(
    inner: R,
    part: &Partition,
) -> Result<RangeReader<R>, Error<RangeError<Error<R::Error>>>>
where
    R: Read + Seek,
{
    RangeReader::new(inner, part.first_byte, part.len).map_err(Error::Io)
}

#[cfg_attr(feature = "std", derive(Error))]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Error<E> {
    #[cfg_attr(feature = "std", error("IO error: {}"))]
    Io(E),
    #[cfg_attr(feature = "std", error("GPT error: {}"))]
    Gpt(gpt::GptError),
    #[cfg_attr(feature = "std", error("MBR error: {}"))]
    Mbr(mbr::MbrError),
    #[cfg_attr(feature = "std", error("partition header not found"))]
    HeaderNotFound,
}

impl<E> From<gpt::GptError> for Error<E> {
    fn from(e: gpt::GptError) -> Self {
        Error::Gpt(e)
    }
}

impl<E> From<mbr::MbrError> for Error<E> {
    fn from(e: mbr::MbrError) -> Self {
        Error::Mbr(e)
    }
}
