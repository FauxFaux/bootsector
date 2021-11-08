use crate::Error;

#[cfg(feature = "std")]
use std::io;

#[derive(Debug, Copy, Eq, PartialEq, Clone)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

pub trait ReaderError {
    type Error;
}

pub trait Read: ReaderError {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error<Self::Error>>;
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error<Self::Error>>;
}

pub trait Seek: ReaderError {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Error<Self::Error>>;
}

#[cfg(feature = "std")]
impl From<io::SeekFrom> for SeekFrom {
    fn from(e: io::SeekFrom) -> Self {
        match e {
            io::SeekFrom::Start(v) => SeekFrom::Start(v),
            io::SeekFrom::End(v) => SeekFrom::End(v),
            io::SeekFrom::Current(v) => SeekFrom::Current(v),
        }
    }
}

#[cfg(feature = "std")]
impl Into<io::SeekFrom> for SeekFrom {
    fn into(e: Self) -> io::SeekFrom {
        match e {
            SeekFrom::Start(v) => io::SeekFrom::Start(v),
            SeekFrom::End(v) => io::SeekFrom::End(v),
            SeekFrom::Current(v) => io::SeekFrom::Current(v),
        }
    }
}

#[cfg(feature = "std")]
impl<R> ReaderError for R
where
    R: io::Read,
{
    type Error = io::Error;
}

#[cfg(feature = "std")]
impl<R> Read for R
where
    R: io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error<Self::Error>> {
        (self as io::Read).read(buf).map_err(Error::Io)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error<Self::Error>> {
        (self as io::Read).read_exact(buf).map_err(Error::Io)
    }
}

#[cfg(feature = "std")]
impl<R> Seek for R
where
    R: io::Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> Result<(), Error<Self::Error>> {
        (self as io::Seek).seek(pos.into()).map_err(Error::Io)
    }
}
