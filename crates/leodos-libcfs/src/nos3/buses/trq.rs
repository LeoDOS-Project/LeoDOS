//! Magnetorquer (torque rod) PWM driver.
//!
//! Magnetorquers generate a magnetic dipole to interact with
//! Earth's magnetic field, providing low-power attitude control
//! and reaction wheel desaturation. Closed on drop.

use crate::ffi;
use core::mem::MaybeUninit;

/// Errors from torquer operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum TrqError {
    /// Initialisation error (`TRQ_INIT_ERR`).
    #[error("Torquer: init error")]
    Init,
    /// Self-test error (`TRQ_SELFTEST_ERR`).
    #[error("Torquer: self-test error")]
    SelfTest,
    /// Connect error (`TRQ_CONNECT_ERR`).
    #[error("Torquer: connect error")]
    Connect,
    /// Invalid torquer number (`TRQ_NUM_ERR`).
    #[error("Torquer: invalid torquer number")]
    NumError,
    /// Time high value out of range (`TRQ_TIME_HIGH_VAL_ERR`).
    #[error("Torquer: time high value error")]
    TimeHighVal,
    /// Unhandled error code.
    #[error("Torquer: unhandled error ({0})")]
    Unhandled(i32),
}

pub(crate) fn check(rc: i32) -> Result<(), TrqError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::TRQ_INIT_ERR => Err(TrqError::Init),
        _ if rc == ffi::TRQ_SELFTEST_ERR => Err(TrqError::SelfTest),
        _ if rc == ffi::TRQ_CONNECT_ERR => Err(TrqError::Connect),
        _ if rc == ffi::TRQ_NUM_ERR => Err(TrqError::NumError),
        _ if rc == ffi::TRQ_TIME_HIGH_VAL_ERR => Err(TrqError::TimeHighVal),
        other => Err(TrqError::Unhandled(other)),
    }
}

/// Torquer direction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TrqDirection {
    /// Positive magnetic moment.
    Positive,
    /// Negative magnetic moment.
    Negative,
}

/// An initialised magnetorquer.
pub struct Torquer {
    pub(crate) inner: ffi::trq_info_t,
}

impl Torquer {
    /// Initialises a torquer.
    ///
    /// - `num`: torquer number (0, 1, or 2)
    /// - `period_ns`: PWM timer period in nanoseconds
    pub fn open(
        num: u8,
        period_ns: u32,
    ) -> Result<Self, TrqError> {
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
    ) -> Result<(), TrqError> {
        let pos = matches!(direction, TrqDirection::Positive);
        check(unsafe {
            ffi::trq_command(&mut self.inner, percent_high, pos)
        })
    }

    /// Sets the PWM high time directly (in nanoseconds).
    pub fn set_time_high(
        &mut self,
        ns: u32,
    ) -> Result<(), TrqError> {
        check(unsafe {
            ffi::trq_set_time_high(&mut self.inner, ns)
        })
    }

    /// Applies the configured timer period.
    pub fn set_period(&mut self) -> Result<(), TrqError> {
        check(unsafe {
            ffi::trq_set_period(&mut self.inner)
        })
    }

    /// Sets the torquer direction.
    pub fn set_direction(
        &mut self,
        direction: TrqDirection,
    ) -> Result<(), TrqError> {
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
