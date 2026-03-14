//! Internal utility functions for OSAL wrappers.
use crate::error::{CfsError, OsalError, Result};
use crate::ffi;
use heapless::CString;

pub(crate) fn c_name_from_str(
    name: &str,
) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
    crate::cstring(name)
}

pub(crate) fn c_path_from_str(
    path: &str,
) -> Result<CString<{ ffi::OS_MAX_PATH_LEN as usize }>> {
    crate::cstring(path).map_err(|_| CfsError::Osal(OsalError::FsPathTooLong))
}
