//! Star tracker.
//!
//! Captures star field images and matches them against a
//! catalogue to produce a high-accuracy attitude quaternion.
//! The primary attitude sensor for fine pointing. Uses UART.

use crate::ffi;
use crate::nos3::{check_uart, UartError};
use crate::nos3::buses::uart::Uart;

/// Star tracker housekeeping telemetry.
#[derive(Debug, Clone, Default)]
pub struct StarTrackerHk {
    /// Device command counter.
    pub device_counter: u32,
}

/// Star tracker quaternion data.
#[derive(Debug, Clone, Default)]
pub struct StarTrackerData {
    /// Quaternion component q0 (scalar).
    pub q0: f64,
    /// Quaternion component q1.
    pub q1: f64,
    /// Quaternion component q2.
    pub q2: f64,
    /// Quaternion component q3.
    pub q3: f64,
    /// Whether the measurement is valid.
    pub is_valid: bool,
}

/// Sends a command to the star tracker.
pub fn command(
    device: &mut Uart,
    cmd: u8,
    payload: u32,
) -> Result<(), UartError> {
    check_uart(unsafe {
        ffi::GENERIC_STAR_TRACKER_CommandDevice(
            &mut device.inner,
            cmd,
            payload,
        )
    })
}

/// Requests housekeeping telemetry from the star tracker.
pub fn request_hk(
    device: &mut Uart,
) -> Result<StarTrackerHk, UartError> {
    let mut raw =
        ffi::GENERIC_STAR_TRACKER_Device_HK_tlm_t::default();
    check_uart(unsafe {
        ffi::GENERIC_STAR_TRACKER_RequestHK(
            &mut device.inner,
            &mut raw,
        )
    })?;
    Ok(StarTrackerHk {
        device_counter: raw.DeviceCounter,
    })
}

/// Requests quaternion data from the star tracker.
pub fn request_data(
    device: &mut Uart,
) -> Result<StarTrackerData, UartError> {
    let mut raw =
        ffi::GENERIC_STAR_TRACKER_Device_Data_tlm_t::default();
    check_uart(unsafe {
        ffi::GENERIC_STAR_TRACKER_RequestData(
            &mut device.inner,
            &mut raw,
        )
    })?;
    Ok(StarTrackerData {
        q0: raw.Q0,
        q1: raw.Q1,
        q2: raw.Q2,
        q3: raw.Q3,
        is_valid: raw.IsValid != 0,
    })
}
