//! CFE Platform Support Package (PSP) interface.
//!
//! The PSP provides the lowest-level abstraction layer, interacting directly with
//! the hardware and board support package (BSP). These wrappers expose some of
//! the more common and useful PSP functions to applications, but many are `unsafe`
//! due to their low-level nature.

use core::ffi::CStr;

use heapless::CString;

use crate::error::Error;
use crate::error::Result;
use crate::ffi;

pub mod cds;
pub mod eeprom;
pub mod exception;
pub mod mem;
pub mod restart;
pub mod time;
pub mod version;
pub mod watchdog;

/// Flushes processor data or instruction caches for a given memory range.
///
/// # Safety
///
/// Flushing caches can have significant system-wide effects. The address and
/// size must correspond to a valid memory region.
pub unsafe fn flush_caches(cache_type: u32, address: *mut (), size: u32) {
    ffi::CFE_PSP_FlushCaches(cache_type, address as *mut _, size);
}

/// Returns the PSP-defined processor name.
///
/// # C-API Mapping
/// This is a safe wrapper for `CFE_PSP_GetProcessorName`.
pub fn get_processor_name() -> &'static str {
    unsafe {
        CStr::from_ptr(ffi::CFE_PSP_GetProcessorName())
            .to_str()
            .unwrap_or("Invalid Processor Name")
    }
}

/// Converts a PSP status code to its symbolic name.
///
/// # C-API Mapping
/// This is a safe wrapper for `CFE_PSP_StatusToString`.
///
/// # Errors
/// Returns an error if the resulting string is too long for the internal buffer.
pub fn status_to_string(
    status: i32,
) -> Result<CString<{ ffi::CFE_PSP_STATUS_STRING_LENGTH as usize }>> {
    let mut buf = [0i8; ffi::CFE_PSP_STATUS_STRING_LENGTH as usize];
    unsafe { ffi::CFE_PSP_StatusToString(status, &mut buf) };
    let c_str = unsafe { CStr::from_ptr(buf.as_ptr()) };
    let mut s = CString::new();
    s.extend_from_bytes(c_str.to_bytes())
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(s)
}
