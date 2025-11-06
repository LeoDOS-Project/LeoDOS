//! CFE Time Services (TIME) interface.
//!
//! This module provides safe wrappers for the cFE Time Services API, which is
//! the primary source for mission-synchronized time in a cFS system. It handles
//! spacecraft time, Mission Elapsed Time (MET), and conversions between them.

use crate::cfe::duration::Duration;
use crate::error::Result;
use crate::ffi;
use crate::status::check;
use core::fmt;
use core::ops::{Add, Sub};
use core::str;

/// A wrapper around `CFE_TIME_SysTime_t` representing a specific time.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct SysTime(pub(crate) ffi::CFE_TIME_SysTime_t);

// Manual implementation of PartialEq because bindgen didn't derive it
impl PartialEq for SysTime {
    fn eq(&self, other: &Self) -> bool {
        self.0.Seconds == other.0.Seconds && self.0.Subseconds == other.0.Subseconds
    }
}

impl Eq for SysTime {}

impl PartialOrd for SysTime {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SysTime {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;
        match unsafe { ffi::CFE_TIME_Compare(self.0, other.0) } {
            ffi::CFE_TIME_Compare_CFE_TIME_A_LT_B => Ordering::Less,
            ffi::CFE_TIME_Compare_CFE_TIME_A_GT_B => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

impl fmt::Display for SysTime {
    /// Formats the time as a `yyyy-ddd-hh:mm:ss.xxxxx` string.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = [0u8; ffi::CFE_TIME_PRINTED_STRING_SIZE as usize];
        unsafe {
            ffi::CFE_TIME_Print(buffer.as_mut_ptr() as *mut libc::c_char, self.0);
        }
        // Find the null terminator to determine string length
        let len = buffer.iter().position(|&b| b == 0).unwrap_or(0);

        // CFE_TIME_Print is guaranteed to produce valid ASCII/UTF-8.
        let s = str::from_utf8(&buffer[..len]).map_err(|_| fmt::Error)?;
        f.write_str(s)
    }
}

impl From<Duration> for SysTime {
    fn from(duration: Duration) -> Self {
        let subseconds = microseconds_to_subseconds(duration.nanos() / 1000);
        SysTime(ffi::CFE_TIME_SysTime_t {
            Seconds: duration.secs(),
            Subseconds: subseconds,
        })
    }
}

impl SysTime {
    /// Returns the seconds component of the time.
    pub fn seconds(&self) -> u32 {
        self.0.Seconds
    }

    /// Returns the subseconds component of the time.
    /// The unit is 1/2^32 seconds.
    pub fn subseconds(&self) -> u32 {
        self.0.Subseconds
    }

    /// Returns the current spacecraft time in the mission-defined default format (TAI or UTC).
    /// This is the most common time function to use.
    pub fn now() -> Self {
        Self(unsafe { ffi::CFE_TIME_GetTime() })
    }

    /// Returns the current TAI (International Atomic Time).
    pub fn now_tai() -> Self {
        Self(unsafe { ffi::CFE_TIME_GetTAI() })
    }

    /// Returns the current UTC (Coordinated Universal Time).
    pub fn now_utc() -> Self {
        Self(unsafe { ffi::CFE_TIME_GetUTC() })
    }

    /// Returns the current Mission Elapsed Time (MET).
    pub fn now_met() -> Self {
        Self(unsafe { ffi::CFE_TIME_GetMET() })
    }

    /// Returns the current value of the spacecraft time correction factor (STCF).
    pub fn now_stcf() -> SysTime {
        SysTime(unsafe { ffi::CFE_TIME_GetSTCF() })
    }

    /// Converts a specified MET into Spacecraft Time (UTC or TAI).
    pub fn to_sc(&self) -> Self {
        Self(unsafe { ffi::CFE_TIME_MET2SCTime(self.0) })
    }

    /// Converts a CFE SysTime into the 6-byte CCSDS Day Segmented time format.
    ///
    /// This is a crucial utility function for creating telemetry.
    ///
    /// Format:
    /// - Bytes 0-3: Seconds since epoch (Big Endian)
    /// - Bytes 4-5: Subseconds, representing fractions of a second in 1/2^16 increments (Big Endian)
    pub fn to_ccsds(self) -> [u8; 6] {
        let mut ccsds_time = [0u8; 6];

        let seconds = self.seconds();

        // 2. Write the seconds into the first 4 bytes of the array in Big Endian format.
        ccsds_time[0..4].copy_from_slice(&seconds.to_be_bytes());

        // 3. Get the 32-bit subseconds field. The unit is 1/(2^32) seconds.
        let subseconds = self.subseconds();

        // 4. Convert the 32-bit subseconds into 16-bit subseconds. The CCSDS format
        //    divides a second into 2^16 parts. We can do this by right-shifting
        //    the 32-bit value by 16, effectively taking the most significant 16 bits.
        let subseconds_16bit = (subseconds >> 16) as u16;

        // 5. Write the 16-bit subseconds into the last 2 bytes of the array in Big Endian format.
        ccsds_time[4..6].copy_from_slice(&subseconds_16bit.to_be_bytes());

        ccsds_time
    }
}

impl Add for SysTime {
    type Output = Self;

    /// Adds two time values.
    fn add(self, other: Self) -> Self::Output {
        Self(unsafe { ffi::CFE_TIME_Add(self.0, other.0) })
    }
}

impl Sub for SysTime {
    type Output = Self;

    /// Subtracts `other` from `self`.
    fn sub(self, other: Self) -> Self::Output {
        Self(unsafe { ffi::CFE_TIME_Subtract(self.0, other.0) })
    }
}

/// Converts a subseconds value (1/2^32 seconds) to microseconds.
pub fn subseconds_to_microseconds(subseconds: u32) -> u32 {
    unsafe { ffi::CFE_TIME_Sub2MicroSecs(subseconds) }
}

/// Converts microseconds to a subseconds value.
pub fn microseconds_to_subseconds(microseconds: u32) -> u32 {
    unsafe { ffi::CFE_TIME_Micro2SubSecs(microseconds) }
}

/// A type alias for the callback function used for time synchronization events.
pub type SynchCallback = unsafe extern "C" fn() -> i32;

/// Registers a synchronization callback to be called on time synchronization events.
pub fn register_synch_callback(callback: SynchCallback) -> Result<()> {
    check(unsafe { ffi::CFE_TIME_RegisterSynchCallback(Some(callback)) })?;
    Ok(())
}

/// Unregisters a previously registered synchronization callback.
pub fn unregister_synch_callback(callback: SynchCallback) -> Result<()> {
    check(unsafe { ffi::CFE_TIME_UnregisterSynchCallback(Some(callback)) })?;
    Ok(())
}

/// Returns the current seconds count of the mission-elapsed time.
pub fn get_met_seconds() -> u32 {
    unsafe { ffi::CFE_TIME_GetMETseconds() }
}

/// Returns the current sub-seconds count of the mission-elapsed time.
pub fn get_met_subseconds() -> u32 {
    unsafe { ffi::CFE_TIME_GetMETsubsecs() }
}

/// Returns the current value of the leap seconds counter.
pub fn get_leap_seconds() -> i16 {
    unsafe { ffi::CFE_TIME_GetLeapSeconds() }
}

/// Returns the current state of the spacecraft clock.
pub fn get_clock_state() -> ffi::CFE_TIME_ClockState_Enum_t {
    unsafe { ffi::CFE_TIME_GetClockState() }
}

/// Provides information about the spacecraft clock as a bitmask.
pub fn get_clock_info() -> u16 {
    unsafe { ffi::CFE_TIME_GetClockInfo() }
}

/// Provides the 1 Hz signal from an external source.
///
/// This may be called from an interrupt handler context.
pub fn external_tone() {
    unsafe { ffi::CFE_TIME_ExternalTone() };
}

/// Provides the Mission Elapsed Time from an external source.
pub fn external_met(new_met: SysTime) {
    unsafe { ffi::CFE_TIME_ExternalMET(new_met.0) };
}

/// Provides time from an external source like a GPS receiver.
pub fn external_gps(new_time: SysTime, new_leaps: i16) {
    unsafe { ffi::CFE_TIME_ExternalGPS(new_time.0, new_leaps) };
}

/// Provides time from an external source relative to a known epoch.
pub fn external_time(new_time: SysTime) {
    unsafe { ffi::CFE_TIME_ExternalTime(new_time.0) };
}
