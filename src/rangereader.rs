use std::convert::TryFrom;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Result;
use std::io::Seek;
use std::io::SeekFrom;

/// Produced by `open_partition`.
pub struct RangeReader<R> {
    inner: R,
    first_byte: u64,
    end: u64,
}

impl<R: Seek> RangeReader<R> {
    pub fn new(mut inner: R, first_byte: u64, len: u64) -> Result<RangeReader<R>> {
        let end = first_byte
            .checked_add(len)
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "len implausibly past end"))?;

        let seeked = inner.seek(SeekFrom::Start(first_byte))?;
        if seeked != first_byte {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "start after end of file",
            ));
        }

        Ok(RangeReader {
            inner,
            first_byte,
            end,
        })
    }

    #[inline]
    fn check_valid_position(&self, pos: u64) -> Result<()> {
        if pos < self.first_byte || pos > self.end {
            Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "illegal cursor position: {} <= {} <= {}",
                    self.first_byte, pos, self.end,
                ),
            ))
        } else {
            Ok(())
        }
    }
}

impl<R> Read for RangeReader<R>
where
    R: Read + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let current_real = self.inner.seek(SeekFrom::Current(0))?;
        self.check_valid_position(current_real)?;

        let available = self.end - current_real;
        let available = usize::try_from(available).unwrap_or(std::usize::MAX);
        let available = available.min(buf.len());
        self.inner.read(&mut buf[..available])
    }
}

impl<R: Seek> Seek for RangeReader<R> {
    fn seek(&mut self, action: SeekFrom) -> Result<u64> {
        let new_pos =
            self.inner.seek(match action {
                SeekFrom::Start(dist) => {
                    SeekFrom::Start(self.first_byte.checked_add(dist).expect("start overflow"))
                }
                SeekFrom::Current(dist) => SeekFrom::Current(dist),
                SeekFrom::End(dist) => {
                    let dist = u64::try_from(-dist).map_err(|_| {
                        Error::new(ErrorKind::InvalidInput, "can't seek positively at end")
                    })?;
                    SeekFrom::Start(self.end.checked_sub(dist).ok_or_else(|| {
                        Error::new(ErrorKind::InvalidInput, "can't seek before zero")
                    })?)
                }
            })?;

        self.check_valid_position(new_pos)?;
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
