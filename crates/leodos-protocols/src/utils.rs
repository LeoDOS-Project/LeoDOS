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

/// Computes an XOR checksum over a byte slice.
pub fn checksum_u8(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |acc, &byte| acc ^ byte)
}

/// Returns true if the XOR checksum of the slice (including the checksum byte) is zero.
pub fn validate_checksum_u8(bytes: &[u8]) -> bool {
    checksum_u8(bytes) == 0
}

/// Trait for types that contain a protocol header of type `H`.
pub trait Header<H> {
    /// Returns a reference to the header.
    fn get(&self) -> &H;
    /// Returns a mutable reference to the header.
    fn get_mut(&mut self) -> &mut H;
}

/// A `no_std`-compatible [`core::fmt::Write`] adapter over a byte slice.
pub struct BufWriter<'a> {
    /// The output buffer.
    pub buf: &'a mut [u8],
    /// Current write position.
    pub pos: usize,
}

impl core::fmt::Write for BufWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let b = s.as_bytes();
        let end = self.pos + b.len();
        if end > self.buf.len() {
            return Err(core::fmt::Error);
        }
        self.buf[self.pos..end].copy_from_slice(b);
        self.pos = end;
        Ok(())
    }
}

/// Formats into a byte buffer using [`core::fmt::Write`], returning bytes written.
#[macro_export]
macro_rules! fmt {
    ($buf:expr, $($arg:tt)*) => {{
        let mut w = $crate::utils::BufWriter { buf: $buf, pos: 0 };
        match core::fmt::Write::write_fmt(&mut w, format_args!($($arg)*)) {
            Ok(()) => Ok(w.pos),
            Err(_) => Err(core::fmt::Error),
        }
    }};
}
