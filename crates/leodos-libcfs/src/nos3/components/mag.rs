//! Generic MAG (Magnetometer) device driver wrapper.
//!
//! Wraps the `GENERIC_MAG_*` device functions for
//! magnetic field measurement over an SPI bus.

use crate::ffi;
use crate::nos3::{check_spi, SpiError};
use crate::nos3::spi::Spi;

/// Magnetometer measurement data.
#[derive(Debug, Clone, Default)]
pub struct MagData {
    /// Magnetic field intensity along X axis (raw).
    pub x: i32,
    /// Magnetic field intensity along Y axis (raw).
    pub y: i32,
    /// Magnetic field intensity along Z axis (raw).
    pub z: i32,
}

/// Requests magnetic field data from the magnetometer.
pub fn request_data(
    device: &mut Spi,
) -> Result<MagData, SpiError> {
    let mut raw = ffi::GENERIC_MAG_Device_Data_tlm_t::default();
    check_spi(unsafe {
        ffi::GENERIC_MAG_RequestData(&mut device.inner, &mut raw)
    })?;
    Ok(MagData {
        x: raw.MagneticIntensityX,
        y: raw.MagneticIntensityY,
        z: raw.MagneticIntensityZ,
    })
}
