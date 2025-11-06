//! A simple duration struct similar to `core::time::Duration`.
//! Uses 32-bit unsigned integers for seconds and nanoseconds instead of 64-bit.

/// A simple duration struct similar to `std::time::Duration`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration {
    secs: u32,
    nanos: u32,
}

impl Duration {
    /// Creates a new `Duration` from seconds and nanoseconds.
    pub fn new(secs: u32, nanos: u32) -> Self {
        Self { secs, nanos }
    }

    /// Creates a new `Duration` from hours.
    pub fn from_hours(hours: u32) -> Self {
        Self {
            secs: hours * 60 * 60,
            nanos: 0,
        }
    }

    /// Creates a new `Duration` from minutes.
    pub fn from_mins(mins: u32) -> Self {
        Self {
            secs: mins * 60,
            nanos: 0,
        }
    }

    /// Creates a new `Duration` from seconds.
    pub fn from_secs(secs: u32) -> Self {
        Self { secs, nanos: 0 }
    }

    /// Creates a new `Duration` from milliseconds.
    pub fn from_millis(millis: u32) -> Self {
        Self {
            secs: millis / 1_000,
            nanos: (millis % 1_000) * 1_000_000,
        }
    }

    /// Creates a new `Duration` from microseconds.
    pub fn from_micros(micros: u32) -> Self {
        Self {
            secs: micros / 1_000_000,
            nanos: (micros % 1_000_000) * 1_000,
        }
    }

    /// Creates a new `Duration` from nanoseconds.
    pub fn from_nanos(nanos: u32) -> Self {
        Self { secs: 0, nanos }
    }

    /// Returns the seconds component of the duration.
    pub fn secs(&self) -> u32 {
        self.secs
    }

    /// Returns the nanoseconds component of the duration.
    pub fn nanos(&self) -> u32 {
        self.nanos
    }

    /// Returns the total duration in milliseconds.
    pub fn millis(&self) -> u32 {
        self.secs * 1_000 + self.nanos / 1_000_000
    }
}
