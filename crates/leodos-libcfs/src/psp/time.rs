//! Safe wrappers for PSP high-resolution time functions.
//!
//! This provides access to a raw, monotonic hardware clock, which is useful for
//! performance measurements. This time is distinct from the mission time provided
//! by `CFE_TIME`.

use crate::ffi;
use crate::os::time::OsTime;
use core::mem::MaybeUninit;

/// A 64-bit timestamp from a high-resolution, monotonic hardware clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timebase {
    /// The upper 32 bits of the 64-bit timebase value.
    pub upper_32: u32,
    /// The lower 32 bits of the 64-bit timebase value.
    pub lower_32: u32,
}

impl Timebase {
    /// Returns the full 64-bit value.
    pub fn as_u64(&self) -> u64 {
        ((self.upper_32 as u64) << 32) | (self.lower_32 as u64)
    }
}

/// Reads the raw, monotonic platform clock without normalization.
pub fn get_timebase() -> Timebase {
    let mut upper = MaybeUninit::uninit();
    let mut lower = MaybeUninit::uninit();
    unsafe { ffi::CFE_PSP_Get_Timebase(upper.as_mut_ptr(), lower.as_mut_ptr()) };
    Timebase {
        upper_32: unsafe { upper.assume_init() },
        lower_32: unsafe { lower.assume_init() },
    }
}

/// Reads the monotonic platform clock and normalizes it to an `OsTime` value.
pub fn get_time() -> OsTime {
    let mut time = MaybeUninit::uninit();
    unsafe { ffi::CFE_PSP_GetTime(time.as_mut_ptr()) };
    OsTime(unsafe { time.assume_init() })
}

/// Returns the resolution of the timebase clock in ticks per
/// second.
///
/// Guaranteed to be at least 1 MHz (1 µs per tick).
pub fn get_timer_ticks_per_second() -> u32 {
    unsafe { ffi::CFE_PSP_GetTimerTicksPerSecond() }
}

/// Returns the value at which the lower 32 bits of the timebase clock roll over.
///
/// A value of 0 indicates that it rolls over at the maximum `u32` value.
pub fn get_timer_low32_rollover() -> u32 {
    unsafe { ffi::CFE_PSP_GetTimerLow32Rollover() }
}
