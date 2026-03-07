//! Safe torquer (magnetorquer) wrapper.
//!
//! Wraps the hwlib `trq_*` functions with RAII lifetime
//! management. The torquer is closed automatically on drop.

use super::{check, HwError};
use crate::ffi;
use core::mem::MaybeUninit;

/// Torquer direction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TrqDirection {
    /// Positive magnetic moment.
    Positive,
    /// Negative magnetic moment.
    Negative,
}

/// An initialised magnetorquer.
///
/// Created via [`Torquer::open`]. Automatically closed on drop.
pub struct Torquer {
    inner: ffi::trq_info_t,
}

impl Torquer {
    /// Initialises a torquer.
    ///
    /// - `num`: torquer number (0, 1, or 2)
    /// - `period_ns`: PWM timer period in nanoseconds
    pub fn open(
        num: u8,
        period_ns: u32,
    ) -> Result<Self, HwError> {
        let mut info: ffi::trq_info_t = unsafe {
            MaybeUninit::zeroed().assume_init()
        };
        info.trq_num = num;
        info.timer_period_ns = period_ns;
        check(unsafe { ffi::trq_init(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Commands the torquer to a duty cycle and direction.
    ///
    /// `percent_high` is the PWM duty cycle (0–100).
    pub fn command(
        &mut self,
        percent_high: u8,
        direction: TrqDirection,
    ) -> Result<(), HwError> {
        let pos = matches!(direction, TrqDirection::Positive);
        check(unsafe {
            ffi::trq_command(&mut self.inner, percent_high, pos)
        })
    }

    /// Sets the PWM high time directly (in nanoseconds).
    pub fn set_time_high(
        &mut self,
        ns: u32,
    ) -> Result<(), HwError> {
        check(unsafe {
            ffi::trq_set_time_high(&mut self.inner, ns)
        })
    }

    /// Applies the configured timer period.
    pub fn set_period(&mut self) -> Result<(), HwError> {
        check(unsafe {
            ffi::trq_set_period(&mut self.inner)
        })
    }

    /// Sets the torquer direction.
    pub fn set_direction(
        &mut self,
        direction: TrqDirection,
    ) -> Result<(), HwError> {
        let pos = matches!(direction, TrqDirection::Positive);
        check(unsafe {
            ffi::trq_set_direction(&mut self.inner, pos)
        })
    }
}

impl Drop for Torquer {
    fn drop(&mut self) {
        unsafe { ffi::trq_close(&mut self.inner); }
    }
}
