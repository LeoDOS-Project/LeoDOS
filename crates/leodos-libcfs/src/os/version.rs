//! Safe, idiomatic wrappers for OSAL version query APIs.

use crate::ffi;
use core::ffi::CStr;

/// Retrieves the OSAL version identifier string (e.g., "equuleus-rc1").
pub fn get_version_string() -> &'static str {
    unsafe {
        CStr::from_ptr(ffi::OS_GetVersionString())
            .to_str()
            .unwrap_or("Invalid Version String")
    }
}

/// Retrieves the OSAL version code name (e.g., "Equuleus").
pub fn get_version_code_name() -> &'static str {
    unsafe {
        CStr::from_ptr(ffi::OS_GetVersionCodeName())
            .to_str()
            .unwrap_or("Invalid Code Name")
    }
}

/// Retrieves the numeric OSAL version identifier.
///
/// The returned array contains `[Major, Minor, Revision, MissionRevision]`.
pub fn get_version_number() -> [u8; 4] {
    let mut numbers: [u8; 4] = [0; 4];
    unsafe {
        ffi::OS_GetVersionNumber(numbers.as_mut_ptr());
    }
    numbers
}

/// Retrieves the OSAL library build number.
///
/// This is a monotonically increasing number that reflects the number of changes
/// since the epoch release.
pub fn get_build_number() -> u32 {
    unsafe { ffi::OS_GetBuildNumber() }
}
