use alloc::string::String;

use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[cfg(feature = "std")]
    #[snafu(display("I/O error: {}", source))]
    Io {
        source: std::io::Error,
    },
    #[cfg(not(feature = "std"))]
    Io {},

    NotFound,

    BiggerThanMemory,

    InvalidStatic {
        message: &'static str,
    },

    InvalidData {
        message: String,
    },
}
