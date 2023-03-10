use alloc::string::String;

use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[cfg(feature = "std")]
    #[snafu(display("Underlying IO error at {pos}: {source}"))]
    Io {
        source: std::io::Error,
        pos: u64,
    },

    NotFound,

    Overflow,

    UnexpectedEof,

    BiggerThanMemory,

    InvalidStatic {
        message: &'static str,
    },

    InvalidData {
        message: String,
    },
}
