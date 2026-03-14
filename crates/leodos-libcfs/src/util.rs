//! General utility functions.

use crate::error::{CfsError, OsalError, Result};
use heapless::CString;

/// Converts a `&str` to a null-terminated `CString<N>`.
pub fn cstring<const N: usize>(s: &str) -> Result<CString<N>> {
    let mut c = CString::new();
    c.extend_from_bytes(s.as_bytes())
        .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
    Ok(c)
}
