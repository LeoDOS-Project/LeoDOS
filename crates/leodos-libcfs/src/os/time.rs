//! Safe, idiomatic wrappers for OSAL local time APIs.
//!
//! This module provides an `OsTime` struct and functions for interacting with
//! the host operating system's local time, as opposed to the cFE-managed
//! spacecraft time.

use crate::error::Result;
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;

/// A wrapper around `OS_time_t` representing a specific local time.
///
/// This time is based on the underlying OS epoch (e.g., UNIX epoch) and is
/// distinct from cFE's mission time (`libcfs::time::SysTime`).
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct OsTime(pub(crate) ffi::OS_time_t);

impl OsTime {
    /// Creates an `OsTime` instance from a relative duration in milliseconds.
    ///
    /// This is a safe wrapper for `OS_TimeFromRelativeMilliseconds`.
    #[cfg(not(nos3_cfe))]
    pub fn from_relative_millis(millis: i32) -> Self {
        Self(unsafe { ffi::OS_TimeFromRelativeMilliseconds(millis) })
    }

    /// Calculates the relative duration in milliseconds until this absolute time.
    ///
    /// This is a safe wrapper for `OS_TimeToRelativeMilliseconds`.
    /// Returns `OS_CHECK` (0) if the time is in the past, or `OS_PEND` (-1) if
    /// the time is too far in the future to be represented.
    #[cfg(not(nos3_cfe))]
    pub fn to_relative_millis(&self) -> i32 {
        unsafe { ffi::OS_TimeToRelativeMilliseconds(self.0) }
    }

    /// Gets the current local time from the underlying OS.
    ///
    /// This is a safe wrapper for `OS_GetLocalTime`.
    pub fn now() -> Result<Self> {
        let mut time_struct = MaybeUninit::uninit();
        check(unsafe { ffi::OS_GetLocalTime(time_struct.as_mut_ptr()) })?;
        Ok(Self(unsafe { time_struct.assume_init() }))
    }
}

/// Sets the local time on the underlying OS.
///
/// This is a safe wrapper for `OS_SetLocalTime`.
pub fn set_local_time(time: OsTime) -> Result<()> {
    check(unsafe { ffi::OS_SetLocalTime(&time.0) })?;
    Ok(())
}
