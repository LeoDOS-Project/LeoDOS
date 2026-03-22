//! General utility functions.

use crate::error::{CfsError, OsalError, Result};
use core::ffi::CStr;
use heapless::CString;
use heapless::String;

/// Converts a `&str` to a null-terminated `CString<N>`.
pub fn cstring<const N: usize>(s: &str) -> Result<CString<N>> {
    let mut c = CString::new();
    c.extend_from_bytes(s.as_bytes())
        .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
    Ok(c)
}

/// Converts a NUL-terminated C `char` array to a `heapless::String<N>`.
pub fn string_from_c_buf<const N: usize>(buf: &[core::ffi::c_char]) -> Result<String<N>> {
    let c_str = unsafe { CStr::from_ptr(buf.as_ptr()) };
    let s = c_str.to_str().map_err(|_| CfsError::InvalidString)?;
    String::try_from(s).map_err(|_| CfsError::Osal(OsalError::NameTooLong))
}
