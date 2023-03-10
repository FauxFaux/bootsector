use crate::Error;

pub trait ReadAt {
    fn read_exact_at(&self, pos: u64, buf: &mut [u8]) -> Result<(), Error>;
}

#[cfg(feature = "std")]
impl<R: positioned_io2::ReadAt> ReadAt for R {
    fn read_exact_at(&self, pos: u64, buf: &mut [u8]) -> Result<(), Error> {
        use crate::errors::IoSnafu;
        use snafu::prelude::*;
        positioned_io2::ReadAt::read_exact_at(self, pos, buf).context(IoSnafu { pos })
    }
}

#[cfg(not(feature = "std"))]
impl<'a> ReadAt for &'a [u8] {
    fn read_exact_at(&self, pos: u64, buf: &mut [u8]) -> Result<(), Error> {
        use core::convert::TryFrom;
        let read_len = u64::try_from(buf.len()).map_err(|_| Error::BiggerThanMemory)?;
        let self_len = u64::try_from(self.len()).map_err(|_| Error::BiggerThanMemory)?;
        if pos + read_len > self_len {
            return Err(Error::UnexpectedEof);
        }
        let start = usize::try_from(pos).map_err(|_| Error::BiggerThanMemory)?;
        let end = start
            .checked_add(buf.len())
            .ok_or(Error::BiggerThanMemory)?;

        buf.copy_from_slice(&self[start..end]);
        Ok(())
    }
}
