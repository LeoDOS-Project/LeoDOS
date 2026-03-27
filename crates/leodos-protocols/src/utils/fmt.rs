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

/// An owned, stack-allocated null-terminated C string.
pub struct CStrBuf<const N: usize> {
    #[doc(hidden)]
    pub buf: [u8; N],
    #[doc(hidden)]
    pub len: usize,
}

impl<const N: usize> CStrBuf<N> {
    /// Returns the contained C string.
    pub fn as_cstr(&self) -> &core::ffi::CStr {
        core::ffi::CStr::from_bytes_until_nul(&self.buf[..self.len + 1]).unwrap()
    }
}

impl<const N: usize> core::ops::Deref for CStrBuf<N> {
    type Target = core::ffi::CStr;
    fn deref(&self) -> &core::ffi::CStr {
        self.as_cstr()
    }
}

/// Formats a null-terminated C string.
///
/// Two forms:
/// - `fmt_cstr!(buf, "fmt", args)` — writes into `buf: &mut [u8]`, returns `Result<&CStr, _>`
/// - `fmt_cstr!(32, "fmt", args)` — returns `Result<CStrBuf<32>, _>` (owned, no separate buffer)
#[macro_export]
macro_rules! fmt_cstr {
    ($n:literal, $($arg:tt)*) => {{
        let mut buf = [0u8; $n];
        let mut w = $crate::utils::fmt::BufWriter { buf: &mut buf, pos: 0 };
        match core::fmt::Write::write_fmt(&mut w, format_args!($($arg)*)) {
            Ok(()) if w.pos < w.buf.len() => {
                w.buf[w.pos] = 0;
                let len = w.pos;
                Ok($crate::utils::fmt::CStrBuf { buf, len })
            }
            _ => Err(core::fmt::Error),
        }
    }};
    ($buf:expr, $($arg:tt)*) => {{
        let buf: &mut [u8] = &mut $buf;
        let mut w = $crate::utils::fmt::BufWriter { buf, pos: 0 };
        match core::fmt::Write::write_fmt(&mut w, format_args!($($arg)*)) {
            Ok(()) if w.pos < w.buf.len() => {
                w.buf[w.pos] = 0;
                Ok(core::ffi::CStr::from_bytes_until_nul(&w.buf[..w.pos + 1]).unwrap())
            }
            _ => Err(core::fmt::Error),
        }
    }};
}
