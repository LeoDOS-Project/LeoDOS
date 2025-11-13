//! CFDP Timer types and functions.

use crate::ffi;

/// Timer state object for CFDP timing operations.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct Timer(pub(crate) ffi::CF_Timer_t);

impl Timer {
    /// Creates a new timer initialized with relative seconds.
    pub fn new(rel_sec: u32) -> Self {
        let mut timer = Self::default();
        timer.init_rel_sec(rel_sec);
        timer
    }

    /// Initializes the timer with relative seconds.
    pub fn init_rel_sec(&mut self, rel_sec: u32) {
        unsafe { ffi::CF_Timer_InitRelSec(&mut self.0, rel_sec) }
    }

    /// Returns true if the timer has expired.
    pub fn expired(&self) -> bool {
        unsafe { ffi::CF_Timer_Expired(&self.0) }
    }

    /// Advances the timer by one tick.
    pub fn tick(&mut self) {
        unsafe { ffi::CF_Timer_Tick(&mut self.0) }
    }

    /// Converts seconds to ticks.
    pub fn sec_to_ticks(sec: u32) -> u32 {
        unsafe { ffi::CF_Timer_Sec2Ticks(sec) }
    }
}
