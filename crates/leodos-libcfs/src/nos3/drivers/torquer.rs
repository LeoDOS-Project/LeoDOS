//! Magnetorquer component driver.
//!
//! Higher-level interface to the magnetorquer hardware,
//! combining duty-cycle and direction into a single config
//! command and returning telemetry.

use crate::ffi;
use crate::nos3::buses::trq::check;
use crate::nos3::buses::trq::Torquer;
use crate::nos3::buses::trq::TrqError;

/// Torquer component telemetry.
#[derive(Debug, Clone, Default)]
pub struct TorquerTlm {
    /// Current direction (0 or 1).
    pub direction: u8,
    /// PWM duty cycle percentage.
    pub percent_on: u8,
}

/// Configures a torquer and returns updated telemetry.
pub fn config(torquer: &mut Torquer, percent: u8, direction: u8) -> Result<TorquerTlm, TrqError> {
    let mut raw = ffi::GENERIC_TORQUER_Device_tlm_t::default();
    check(unsafe {
        ffi::GENERIC_TORQUER_Config(&mut raw, &mut torquer.inner, percent, direction)
    })?;
    Ok(TorquerTlm {
        direction: raw.Direction,
        percent_on: raw.PercentOn,
    })
}
