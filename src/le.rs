use core::convert::TryInto;

#[inline]
pub fn read_u16(slice: &[u8]) -> u16 {
    u16::from_le_bytes(slice[..2].try_into().expect("fixed size slice"))
}

#[inline]
pub fn read_u32(slice: &[u8]) -> u32 {
    u32::from_le_bytes(slice[..4].try_into().expect("fixed size slice"))
}

#[inline]
pub fn read_u64(slice: &[u8]) -> u64 {
    u64::from_le_bytes(slice[..8].try_into().expect("fixed size slice"))
}
