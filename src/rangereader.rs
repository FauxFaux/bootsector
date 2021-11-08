use crate::no_std::convert::TryFrom;
#[cfg(feature = "std")]
use std::io::{self, ErrorKind, SeekFrom};

use crate::read::{Read, ReaderError, Seek, SeekFrom};
use crate::Error;

#[cfg_attr(feature = "std", derive(Error))]
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum RangeError<E> {
    #[cfg_attr(feature = "std", error("len implausibly past end"))]
    LenImplausiblePastEnd,
    #[cfg_attr(feature = "std", error("start after end of file"))]
    StartAfterEndOfFile,
    #[cfg_attr(
        feature = "std",
        error("illegal cursor position: {first_byte} <= {pos} <= {end}")
    )]
    IllegalCursorPosition { first_byte: u64, pos: u64, end: u64 },
    #[cfg_attr(feature = "std", error("can't seek positively at end"))]
    CantSeekPastEnd,
    #[cfg_attr(feature = "std", error("can't seek before zero"))]
    CantSeekBeforeStart,
    #[cfg_attr(feature = "std", error("Inner io error: {}"))]
    Inner(E),
}

#[cfg(feature = "std")]
impl<E> Into<io::Error> for RangeError<E>
where
    E: Into<io::Error>,
{
    fn into(self) -> io::Error {
        use RangeError::*;
        match self {
            Inner(E) => E,
            e => io::Error::new(ErrorKind::InvalidInput, format!("{}", e)),
        }
    }
}

/// Produced by `open_partition`.
pub struct RangeReader<R> {
    inner: R,
    first_byte: u64,
    end: u64,
}

impl<R: Seek> RangeReader<R> {
    pub fn new(
        mut inner: R,
        first_byte: u64,
        len: u64,
    ) -> Result<RangeReader<R>, RangeError<Error<R::Error>>> {
        let end = first_byte
            .checked_add(len)
            .ok_or_else(|| RangeError::LenImplausiblePastEnd)?;

        let seeked = inner
            .seek(SeekFrom::Start(first_byte))
            .map_err(RangeError::Inner)?;
        if seeked != first_byte {
            return Err(RangeError::StartAfterEndOfFile);
        }

        Ok(RangeReader {
            inner,
            first_byte,
            end,
        })
    }

    #[inline]
    fn check_valid_position(&self, pos: u64) -> Result<(), RangeError<Error<R::Error>>> {
        if pos < self.first_byte || pos > self.end {
            Err(RangeError::IllegalCursorPosition {
                first_byte: self.first_byte,
                pos,
                end: self.end,
            })
        } else {
            Ok(())
        }
    }
}

impl<R, E> ReaderError for RangeReader<R>
where
    R: Read<Error = E> + Seek<Error = E>,
{
    // We carry over the error from the inner reader
    type Error = RangeError<Error<E>>;
}

impl<R, E> Read for RangeReader<R>
where
    R: Read<Error = E> + Seek<Error = E>,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error<Self::Error>> {
        let current_real = self
            .inner
            .seek(SeekFrom::Current(0))
            .map_err(RangeError::Inner)
            .map_err(Error::Io)?;
        self.check_valid_position(current_real).map_err(Error::Io)?;

        let available = self.end - current_real;
        let available = usize::try_from(available).unwrap_or(crate::no_std::usize::MAX);
        let available = available.min(buf.len());
        self.inner
            .read(&mut buf[..available])
            .map_err(RangeError::Inner)
            .map_err(Error::Io)
    }

    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<(), Error<Self::Error>> {
        let current_real = self
            .inner
            .seek(SeekFrom::Current(0))
            .map_err(RangeError::Inner)
            .map_err(Error::Io)?;
        self.check_valid_position(current_real).map_err(Error::Io)?;

        let end_real = current_real + buf.len() as u64;
        self.check_valid_position(end_real).map_err(Error::Io)?;

        self.inner
            .read_exact(buf)
            .map_err(RangeError::Inner)
            .map_err(Error::Io)
    }
}

impl<R> Seek for RangeReader<R>
where
    R: Read + Seek,
{
    fn seek(&mut self, action: SeekFrom) -> Result<u64, Error<Self::Error>> {
        let new_pos = self
            .inner
            .seek(match action {
                SeekFrom::Start(dist) => {
                    SeekFrom::Start(self.first_byte.checked_add(dist).expect("start overflow"))
                }
                SeekFrom::Current(dist) => SeekFrom::Current(dist),
                SeekFrom::End(dist) => {
                    let dist = u64::try_from(-dist)
                        .map_err(|_| RangeError::CantSeekPastEnd)
                        .map_err(Error::Io)?;
                    SeekFrom::Start(
                        self.end
                            .checked_sub(dist)
                            .ok_or_else(|| RangeError::CantSeekBeforeStart)
                            .map_err(Error::Io)?,
                    )
                }
            })
            .map_err(RangeError::Inner)
            .map_err(Error::Io)?;

        self.check_valid_position(new_pos).map_err(Error::Io)?;
        Ok(new_pos - self.first_byte)
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::io::Read;
    use std::io::Seek;
    use std::io::SeekFrom;

    use super::RangeReader;

    #[test]
    fn reader() {
        let data = io::Cursor::new([0u8, 1, 2, 3, 4, 5, 6, 7]);
        let mut reader = RangeReader::new(data, 2, 5).expect("setup");
        let mut buf = [0u8, 2];
        reader.read_exact(&mut buf).expect("read");
        assert_eq!(2, buf[0]);
        assert_eq!(3, buf[1]);
        reader.read_exact(&mut buf).expect("read");
        assert_eq!(4, buf[0]);
        assert_eq!(5, buf[1]);
        assert_eq!(1, reader.read(&mut buf).expect("read"));
        assert_eq!(6, buf[0]);
        assert_eq!(0, reader.read(&mut buf).expect("read"));

        reader.seek(SeekFrom::Start(0)).expect("seek");
        reader.read_exact(&mut buf).expect("read");
        assert_eq!(2, buf[0]);
        assert_eq!(3, buf[1]);

        reader.seek(SeekFrom::End(-2)).expect("seek");
        reader.read_exact(&mut buf).expect("read");
        assert_eq!(5, buf[0]);
        assert_eq!(6, buf[1]);

        reader.seek(SeekFrom::Start(2)).expect("seek");
        reader.seek(SeekFrom::Current(-1)).expect("seek");
        reader.read_exact(&mut buf).expect("read");
        assert_eq!(3, buf[0]);
        assert_eq!(4, buf[1]);
    }
}
