//! Safe wrappers for CFE Library query APIs.

use crate::cfe::es::app::AppInfo;
use crate::error::{CfsError, OsalError, Result};
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;
use heapless::{CString, String};

/// A type-safe, zero-cost wrapper for a cFE Library ID.
///
/// This is a lightweight, `Copy`-able handle that represents a unique loaded library.
/// It can be used to query information about that specific library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct LibId(pub ffi::CFE_ES_LibId_t);

impl LibId {
    /// Retrieves the cFE Library ID for a given library name.
    ///
    /// # Arguments
    /// * `name`: The registered name of the library to look up.
    ///
    /// # Errors
    ///
    /// Returns an error if no library with the given name is found.
    pub fn from_name(name: &str) -> Result<Self> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;

        let mut lib_id = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetLibIDByName(lib_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(Self(unsafe { lib_id.assume_init() }))
    }

    /// Retrieves the name for this library ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the Lib ID is not valid, the buffer is too small
    /// (unlikely with `heapless`), or the name is not valid UTF-8.
    pub fn name(&self) -> Result<String<{ ffi::OS_MAX_API_NAME as usize }>> {
        let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
        check(unsafe {
            ffi::CFE_ES_GetLibName(
                buffer.as_mut_ptr() as *mut libc::c_char,
                self.0,
                buffer.len(),
            )
        })?;
        let len = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
        let vec = heapless::Vec::from_slice(&buffer[..len]).map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
        String::from_utf8(vec).map_err(|_| CfsError::InvalidString)
    }

    /// Retrieves detailed information about the library with this ID.
    ///
    /// Note: This reuses the `AppInfo` struct, as the underlying FFI type is the same.
    /// Fields related to tasks (e.g., `MainTaskId`) will not be meaningful for a library.
    ///
    /// # Errors
    ///
    /// Returns an error if the Lib ID is not valid or if the underlying
    /// CFE call fails.
    pub fn info(&self) -> Result<AppInfo> {
        let mut lib_info_uninit = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetLibInfo(lib_info_uninit.as_mut_ptr(), self.0) })?;
        Ok(AppInfo {
            inner: unsafe { lib_info_uninit.assume_init() },
        })
    }

    /// Converts the Lib ID into a zero-based integer suitable for array indexing.
    ///
    /// # Errors
    ///
    /// Returns an error if the Lib ID is not valid or if the underlying
    /// CFE call fails.
    pub fn to_index(&self) -> Result<u32> {
        let mut index = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_LibID_ToIndex(self.0, index.as_mut_ptr()) })?;
        Ok(unsafe { index.assume_init() })
    }
}
