//! Monotonic clock abstraction for time-dependent routing
//! and protocol decisions.

use core::cell::Cell;

/// Provides the current time in seconds.
pub trait Clock {
    /// Returns the current time in seconds since an
    /// arbitrary epoch.
    fn now(&self) -> u32;
}

/// A fixed clock that always returns the same value.
///
/// Useful for testing or when time-dependent routing is
/// not needed.
pub struct FixedClock(Cell<u32>);

impl FixedClock {
    /// Creates a fixed clock at the given time.
    pub fn new(time_s: u32) -> Self {
        Self(Cell::new(time_s))
    }

    /// Updates the fixed time value.
    pub fn set(&self, time_s: u32) {
        self.0.set(time_s);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> u32 {
        self.0.get()
    }
}

/// Clock backed by cFS Mission Elapsed Time (counts from 0).
#[cfg(feature = "cfs")]
pub struct MetClock;

#[cfg(feature = "cfs")]
impl MetClock {
    /// Creates a new MET clock.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "cfs")]
impl Clock for MetClock {
    fn now(&self) -> u32 {
        leodos_libcfs::cfe::time::SysTime::now_met().seconds()
    }
}

/// Clock backed by cFS spacecraft time (TAI or UTC,
/// depending on mission config).
#[cfg(feature = "cfs")]
pub struct SysTimeClock;

#[cfg(feature = "cfs")]
impl SysTimeClock {
    /// Creates a new spacecraft time clock.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "cfs")]
impl Clock for SysTimeClock {
    fn now(&self) -> u32 {
        leodos_libcfs::cfe::time::SysTime::now().seconds()
    }
}

/// Clock backed by `std::time::Instant`.
#[cfg(feature = "std")]
pub struct StdClock {
    epoch: std::time::Instant,
}

#[cfg(feature = "std")]
impl StdClock {
    /// Creates a clock starting from now.
    pub fn new() -> Self {
        Self {
            epoch: std::time::Instant::now(),
        }
    }
}

#[cfg(feature = "std")]
impl Clock for StdClock {
    fn now(&self) -> u32 {
        self.epoch.elapsed().as_secs() as u32
    }
}
