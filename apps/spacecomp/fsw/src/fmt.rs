pub struct BufWriter<'a> {
    pub buf: &'a mut [u8],
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

macro_rules! fmt {
    ($buf:expr, $($arg:tt)*) => {{
        let mut w = $crate::fmt::BufWriter { buf: $buf, pos: 0 };
        match core::fmt::Write::write_fmt(&mut w, format_args!($($arg)*)) {
            Ok(()) => Ok(w.pos),
            Err(_) => Err(core::fmt::Error),
        }
    }};
}
