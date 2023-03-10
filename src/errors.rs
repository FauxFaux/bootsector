use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[cfg(feature = "std")]
    #[snafu(display("I/O error: {}", source))]
    Io {
        source: std::io::Error,
    },

    NotFound,

    BiggerThanMemory,

    InvalidStatic {
        message: &'static str,
    },

    InvalidData {
        message: String,
    },
}
