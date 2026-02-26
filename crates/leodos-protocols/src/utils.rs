use zerocopy::network_endian::U16;

/// Returns the bits from `bitmap` specified by `mask`, right-aligned.
pub const fn get_bits_u16(bitmap: U16, mask: u16) -> u16 {
    (bitmap.get() & mask) >> mask.trailing_zeros()
}

/// Returns `bitmap` with the bits specified by `mask` set to `value`.
pub fn set_bits_u16(bitmap: &mut U16, mask: u16, value: u16) {
    bitmap.set((bitmap.get() & !mask) | (value << mask.trailing_zeros()));
}

/// Returns the bits from `bitmap` specified by `mask`, right-aligned.
pub const fn get_bits_u8(bitmap: u8, mask: u8) -> u8 {
    (bitmap & mask) >> mask.trailing_zeros()
}

/// Returns `bitmap` with the bits specified by `mask` set to `value`.
pub const fn set_bits_u8(bitmap: &mut u8, mask: u8, value: u8) {
    *bitmap = (*bitmap & !mask) | (value << mask.trailing_zeros())
}

/// Returns the minimum number of bytes required to represent the given u64 value.
pub fn min_len(v: u64) -> usize {
    for len in 1..=8 {
        if v < (1u64 << (len * 8)) {
            return len;
        }
    }
    8
}

pub fn checksum_u8(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |acc, &byte| acc ^ byte)
}

pub fn validate_checksum_u8(bytes: &[u8]) -> bool {
    checksum_u8(bytes) == 0
}

pub trait Header<H> {
    fn get(&self) -> &H;
    fn get_mut(&mut self) -> &mut H;
}
