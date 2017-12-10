use std;

use std::io::Read;
use std::io::Result;
use std::io::Seek;
use std::io::SeekFrom;

/// Produced by `read_partition`.
pub struct RangeReader<R> {
    inner: R,
    first_byte: u64,
    len: u64,
}

impl<R: Seek> RangeReader<R> {
    pub fn new(mut inner: R, first_byte: u64, len: u64) -> Result<RangeReader<R>> {
        assert!(first_byte <= std::i64::MAX as u64);
        assert!(len <= std::i64::MAX as u64);

        assert_eq!(first_byte, inner.seek(SeekFrom::Start(first_byte))?);

        Ok(RangeReader {
            inner,
            first_byte,
            len,
        })
    }
}

impl<R> Read for RangeReader<R>
where
    R: Read + Seek,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let pos = self.inner.seek(SeekFrom::Current(0))? - self.first_byte;
        let remaining = self.len - pos;
        if remaining >= buf.len() as u64 {
            self.inner.read(buf)
        } else {
            self.inner.read(&mut buf[0..(remaining as usize)])
        }
    }
}

impl<R: Seek> Seek for RangeReader<R> {
    fn seek(&mut self, action: SeekFrom) -> Result<u64> {
        let new_pos = self.inner.seek(match action {
            SeekFrom::Start(dist) => {
                SeekFrom::Start(self.first_byte.checked_add(dist).expect("start overflow"))
            }
            SeekFrom::Current(dist) => SeekFrom::Current(dist),
            SeekFrom::End(dist) => {
                assert!(dist <= 0, "can't seek positively at end");
                // TODO: checked?
                SeekFrom::Start(self.first_byte + self.len - (-dist) as u64)
            }
        })?;

        assert!(
            new_pos >= self.first_byte && new_pos < self.first_byte + self.len,
            "out of bound seek: {:?} must leave us between {} and {}, but was {}",
            action,
            self.first_byte,
            self.len,
            new_pos
        );

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
