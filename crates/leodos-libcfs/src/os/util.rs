//! Internal utility functions for OSAL wrappers.
use crate::error::{Error, Result};
use crate::ffi;
use heapless::CString;

pub(crate) fn c_name_from_str(name: &str) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
    let mut c_name = CString::new();
    c_name
        .extend_from_bytes(name.as_bytes())
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(c_name)
}

pub(crate) fn c_path_from_str(path: &str) -> Result<CString<{ ffi::OS_MAX_PATH_LEN as usize }>> {
    let mut c_path = CString::new();
    c_path
        .extend_from_bytes(path.as_bytes())
        .map_err(|_| Error::OsFsErrPathTooLong)?;
    Ok(c_path)
}
