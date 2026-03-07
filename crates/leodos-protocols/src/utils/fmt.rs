//! `no_std` formatting utilities.

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
        let mut w = $crate::utils::fmt::BufWriter { buf: $buf, pos: 0 };
        match core::fmt::Write::write_fmt(&mut w, format_args!($($arg)*)) {
            Ok(()) => Ok(w.pos),
            Err(_) => Err(core::fmt::Error),
        }
    }};
}
