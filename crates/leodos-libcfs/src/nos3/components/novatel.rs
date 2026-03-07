//! NovAtel OEM615 GPS receiver device driver wrapper.
//!
//! Wraps the `NOVATEL_OEM615_*` device functions for
//! GPS position/velocity data over a UART bus.

use crate::ffi;
use crate::nos3::{check_uart, UartError};
use crate::nos3::uart::Uart;

/// GPS position and velocity data.
#[derive(Debug, Clone, Default)]
pub struct GpsData {
    /// GPS weeks since epoch.
    pub weeks: u16,
    /// Seconds into the current week.
    pub seconds_into_week: u32,
    /// Fractional seconds.
    pub fractions: f64,
    /// ECEF X position (m).
    pub ecef_x: f64,
    /// ECEF Y position (m).
    pub ecef_y: f64,
    /// ECEF Z position (m).
    pub ecef_z: f64,
    /// Velocity X (m/s).
    pub vel_x: f64,
    /// Velocity Y (m/s).
    pub vel_y: f64,
    /// Velocity Z (m/s).
    pub vel_z: f64,
    /// Latitude (degrees).
    pub lat: f32,
    /// Longitude (degrees).
    pub lon: f32,
    /// Altitude (m).
    pub alt: f32,
}

/// Sends a command to the GPS receiver.
pub fn command(
    device: &mut Uart,
    cmd_code: u8,
    log_type: i8,
    period_option: i8,
) -> Result<(), UartError> {
    check_uart(unsafe {
        ffi::NOVATEL_OEM615_CommandDevice(
            &mut device.inner,
            cmd_code,
            log_type,
            period_option,
        )
    })
}

/// Requests position/velocity data from the GPS receiver.
pub fn request_data(
    device: &mut Uart,
) -> Result<GpsData, UartError> {
    let mut raw =
        ffi::NOVATEL_OEM615_Device_Data_tlm_t::default();
    check_uart(unsafe {
        ffi::NOVATEL_OEM615_RequestData(
            &mut device.inner,
            &mut raw,
        )
    })?;
    Ok(gps_from_raw(&raw))
}

/// Reads GPS data via the child process interface.
pub fn child_read_data(
    device: &mut Uart,
) -> Result<GpsData, UartError> {
    let mut raw =
        ffi::NOVATEL_OEM615_Device_Data_tlm_t::default();
    check_uart(unsafe {
        ffi::NOVATEL_OEM615_ChildProcessReadData(
            &mut device.inner,
            &mut raw,
        )
    })?;
    Ok(gps_from_raw(&raw))
}

fn gps_from_raw(
    raw: &ffi::NOVATEL_OEM615_Device_Data_tlm_t,
) -> GpsData {
    GpsData {
        weeks: raw.Weeks,
        seconds_into_week: raw.SecondsIntoWeek,
        fractions: raw.Fractions,
        ecef_x: raw.ECEFX,
        ecef_y: raw.ECEFY,
        ecef_z: raw.ECEFZ,
        vel_x: raw.VelX,
        vel_y: raw.VelY,
        vel_z: raw.VelZ,
        lat: raw.lat,
        lon: raw.lon,
        alt: raw.alt,
    }
}
