//! Wrappers for PSP version query functions.

use crate::ffi;
use core::ffi::CStr;

/// Retrieves the PSP version identifier string (e.g., "equuleus-rc1").
pub fn get_version_string() -> &'static str {
    unsafe {
        CStr::from_ptr(ffi::CFE_PSP_GetVersionString())
            .to_str()
            .unwrap_or("Invalid Version String")
    }
}

/// Retrieves the PSP version code name (e.g., "Equuleus").
pub fn get_version_code_name() -> &'static str {
    unsafe {
        CStr::from_ptr(ffi::CFE_PSP_GetVersionCodeName())
            .to_str()
            .unwrap_or("Invalid Code Name")
    }
}

/// Retrieves the numeric PSP version identifier.
///
/// The returned array contains
/// `[Major, Minor, Revision, MissionRevision]`.
///
/// MissionRevision semantics: 0 = official release,
/// 1–254 = local patch (mission use), 255 = development build.
pub fn get_version_number() -> [u8; 4] {
    let mut numbers: [u8; 4] = [0; 4];
    unsafe {
        ffi::CFE_PSP_GetVersionNumber(numbers.as_mut_ptr());
    }
    numbers
}

/// Retrieves the PSP library build number.
///
/// Monotonically increasing number reflecting commits since the
/// epoch release. Fixed at compile time.
pub fn get_build_number() -> u32 {
    unsafe { ffi::CFE_PSP_GetBuildNumber() }
}
