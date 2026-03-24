//! Fine Sun Sensor (FSS).
//!
//! Measures the sun direction with high angular precision
//! (alpha/beta angles) using a position-sensitive detector.
//! Used for fine attitude determination. Communicates via SPI.

use crate::ffi;
use crate::nos3::buses::spi::check;
use crate::nos3::buses::spi::Spi;
use crate::nos3::buses::spi::SpiError;

/// FSS measurement data.
#[derive(Debug, Clone, Default)]
pub struct FssData {
    /// Sun angle alpha (radians).
    pub alpha: f32,
    /// Sun angle beta (radians).
    pub beta: f32,
    /// Error code (0 = valid, 1 = invalid).
    pub error_code: u8,
}

/// Requests sun angle data from the FSS.
pub fn request_data(device: &mut Spi) -> Result<FssData, SpiError> {
    let mut raw = ffi::GENERIC_FSS_Device_Data_tlm_t::default();
    check(unsafe { ffi::GENERIC_FSS_RequestData(&mut device.inner, &mut raw) })?;
    Ok(FssData {
        alpha: raw.Alpha,
        beta: raw.Beta,
        error_code: raw.ErrorCode,
    })
}
