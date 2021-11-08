#[cfg(not(feature = "std"))]
pub(crate) use core::{u64, usize};
#[cfg(feature = "std")]
pub(crate) use std::{u64, usize};

pub(crate) mod convert {
    #[cfg(not(feature = "std"))]
    pub(crate) use core::convert::{TryFrom, TryInto};
    #[cfg(feature = "std")]
    pub(crate) use std::convert::{TryFrom, TryInto};
}
