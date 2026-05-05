//! Process environment variables.
//!
//! cFS doesn't ship its own env-var abstraction; this is a thin
//! safe wrapper around `libc::getenv` for the pc-linux PSP.
//! Returns the value as a fixed-capacity [`heapless::String`] so
//! consumers can stay `no_std`.

use core::ffi::c_char;
use heapless::String;

/// Returns the value of `name` from the process environment, or
/// `None` if the variable is unset, the value doesn't fit in `N`
/// bytes, or it isn't valid UTF-8.
///
/// `N` is the maximum value length (excluding the trailing NUL).
pub fn var<const N: usize>(name: &str) -> Option<String<N>> {
    let mut name_buf = [0u8; 256];
    if name.len() >= name_buf.len() {
        return None;
    }
    name_buf[..name.len()].copy_from_slice(name.as_bytes());
    name_buf[name.len()] = 0;

    // SAFETY: name_buf is a properly NUL-terminated C string;
    // libc::getenv reads until NUL.
    let ptr = unsafe { libc::getenv(name_buf.as_ptr() as *const c_char) };
    if ptr.is_null() {
        return None;
    }

    let mut bytes = [0u8; 1024];
    let mut len = 0;
    // SAFETY: getenv returns a NUL-terminated C string belonging
    // to the environment; we copy bytes until we hit NUL or fill
    // our local buffer.
    unsafe {
        while len < bytes.len() {
            let b = *ptr.add(len);
            if b == 0 {
                break;
            }
            bytes[len] = b as u8;
            len += 1;
        }
    }
    if len == 0 || len == bytes.len() {
        return None;
    }

    let s = core::str::from_utf8(&bytes[..len]).ok()?;
    String::try_from(s).ok()
}
