use positioned_io2 as pio;
use snafu::prelude::*;

use crate::errors::IoSnafu;
use crate::Error;

pub trait Read {
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error>;
}
pub trait Seek {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Error>;
}

pub enum SeekFrom {
    Start(u64),
}

#[cfg(feature = "std")]
impl SeekFrom {
    fn to_std(self) -> std::io::SeekFrom {
        match self {
            SeekFrom::Start(p) => std::io::SeekFrom::Start(p),
        }
    }
}

#[cfg(feature = "std")]
impl<R: pio::ReadAt> Read for pio::Cursor<R> {
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        std::io::Read::read_exact(self, buf).context(IoSnafu {})
    }
}

#[cfg(feature = "std")]
impl<R: pio::ReadAt> Seek for pio::Cursor<R> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Error> {
        std::io::Seek::seek(self, pos.to_std()).context(IoSnafu {})
    }
}
