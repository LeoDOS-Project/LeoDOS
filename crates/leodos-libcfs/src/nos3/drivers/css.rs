//! Coarse Sun Sensor (CSS).
//!
//! An array of six photodiodes that measure solar irradiance
//! from different directions, giving a coarse estimate of the
//! sun vector for safe-mode attitude determination. Uses I2C.

use crate::ffi;
use crate::nos3::buses::i2c::{check, I2cError};
use crate::nos3::buses::i2c::I2cBus;

/// CSS channel data (6 photodiode voltages).
#[derive(Debug, Clone, Default)]
pub struct CssData {
    /// Raw voltage readings from each channel.
    pub voltage: [u16; 6],
}

/// Requests sun sensor data from the CSS.
pub fn request_data(
    device: &mut I2cBus,
) -> Result<CssData, I2cError> {
    let mut raw = ffi::GENERIC_CSS_Device_Data_tlm_t::default();
    check(unsafe {
        ffi::GENERIC_CSS_RequestData(&mut device.inner, &mut raw)
    })?;
    Ok(CssData {
        voltage: raw.Voltage,
    })
}
