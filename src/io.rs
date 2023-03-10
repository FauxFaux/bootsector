use positioned_io2 as pio;
use snafu::prelude::*;

use crate::errors::IoSnafu;
use crate::Error;

pub trait ReadAt {
    fn read_exact_at(&self, pos: u64, buf: &mut [u8]) -> Result<(), Error>;
}

pub trait ReadSeek {
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error>;
    fn seek_abs(&mut self, pos: u64) -> Result<u64, Error>;
}

#[cfg(feature = "std")]
impl<R: std::io::Read + std::io::Seek> ReadSeek for R {
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        std::io::Read::read_exact(self, buf).context(IoSnafu {})
    }

    fn seek_abs(&mut self, pos: u64) -> Result<u64, Error> {
        std::io::Seek::seek(self, std::io::SeekFrom::Start(pos)).context(IoSnafu {})
    }
}

#[cfg(feature = "std")]
impl<R: pio::ReadAt> ReadAt for R {
    fn read_exact_at(&self, pos: u64, buf: &mut [u8]) -> Result<(), Error> {
        pio::ReadAt::read_exact_at(self, pos, buf).context(IoSnafu {})
    }
}

// #[cfg(feature = "std")]
// impl<R: std::io::Read + std::io::Seek> ReadAt for R {
//     fn read_exact_at(&self, pos: u64, buf: &mut [u8]) -> Result<(), Error> {
//         self.seek(std::io::SeekFrom::Start(pos)).context(IoSnafu {})?;
//         self.read_exact(buf).context(IoSnafu {})
//     }
// }
