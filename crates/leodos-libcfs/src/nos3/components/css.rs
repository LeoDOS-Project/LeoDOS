//! Generic CSS (Coarse Sun Sensor) device driver wrapper.
//!
//! Wraps the `GENERIC_CSS_*` device functions for
//! sun vector estimation over an I2C bus.

use crate::ffi;
use crate::nos3::{check_i2c, I2cError};
use crate::nos3::i2c::I2cBus;

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
    check_i2c(unsafe {
        ffi::GENERIC_CSS_RequestData(&mut device.inner, &mut raw)
    })?;
    Ok(CssData {
        voltage: raw.Voltage,
    })
}
