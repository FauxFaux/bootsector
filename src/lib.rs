extern crate byteorder;
extern crate crc;

use std::io;

mod gpt;
mod mbr;
mod rangereader;

#[cfg_attr(rustfmt, rustfmt_skip)]
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
        name: String,
    },
}

/// An entry in the partition table.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Partition {
    pub id: usize,
    pub first_byte: u64,
    pub attributes: Attributes,
    pub len: u64,
}

pub enum ReadMBR {
    /// A compliant, modern MBR: CHS addressing is correctly set to the blind value.
    Modern,
    /// Require there to be a GPT partition present. The protective MBR is allowed, but ignored.
    Never,
}

pub enum ReadGPT {
    /// A valid GPT partition table as of revision 1 (2010-2017 and counting)
    RevisionOne,

    /// Require that there be an MBR partition present. The protective MBR will be read literally.
    Never,
}

pub enum SectorSize {
    /// Attempt to identify a valid GPT partition table at various locations, and use this
    /// information to derive the sector size. For MBR, it's very likely that 512 is a safe
    /// assumption.
    GuessOrAssume,

    /// Use a specific known sector size.
    Known(u16),
}

pub struct Options {
    pub mbr: ReadMBR,
    pub gpt: ReadGPT,
    pub sector_size: SectorSize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mbr: ReadMBR::Modern,
            gpt: ReadGPT::RevisionOne,
            sector_size: SectorSize::GuessOrAssume,
        }
    }
}

pub fn open<R>(mut reader: R, options: &Options) -> io::Result<Vec<Partition>>
where
    R: io::Read + io::Seek,
{
    let header_table = {
        reader.seek(io::SeekFrom::Start(0))?;

        let mut disc_header = [0u8; 512];
        reader.read_exact(&mut disc_header)?;

        if 0x55 != disc_header[510] || 0xAA != disc_header[511] {
            return Err(io::ErrorKind::NotFound.into());
        }

        mbr::parse_partition_table(&disc_header)?
    };

    match header_table.len() {
        1 if gpt::protective(&header_table[0]) => {}
        _ => return Ok(header_table),
    }

    match options.gpt {
        ReadGPT::Never => Ok(header_table),
        ReadGPT::RevisionOne => {
            let sector_size = match options.sector_size {
                SectorSize::Known(size) => size as usize,
                SectorSize::GuessOrAssume => header_table[0].first_byte as usize,
            };

            gpt::read(reader, sector_size)
        }
    }
}

#[cfg(test)]
mod tests {
    use open;
    use std::fs;
    use std::io::Read;
    use std::io::Seek;
    #[test]
    fn parse() {
        open(
            fs::File::open("src/test-data/4t-gpt.img").unwrap(),
            &::Options::default(),
        ).unwrap();
    }
}
