//! Inertial Measurement Unit (IMU).
//!
//! Combines a three-axis gyroscope and accelerometer to
//! measure angular rates and linear acceleration for attitude
//! propagation and manoeuvre detection. Uses a CAN bus.

use crate::ffi;
use crate::nos3::buses::can::{check, CanError};
use crate::nos3::buses::can::Can;

/// IMU housekeeping telemetry.
#[derive(Debug, Clone, Default)]
pub struct ImuHk {
    /// Device command counter.
    pub device_counter: u32,
    /// Device status word.
    pub device_status: u32,
}

/// Single-axis IMU data.
#[derive(Debug, Clone, Default)]
pub struct AxisData {
    /// Linear acceleration (m/s^2).
    pub linear_acc: f32,
    /// Angular rate (rad/s).
    pub angular_acc: f32,
}

/// Three-axis IMU data.
#[derive(Debug, Clone, Default)]
pub struct ImuData {
    /// X-axis data.
    pub x: AxisData,
    /// Y-axis data.
    pub y: AxisData,
    /// Z-axis data.
    pub z: AxisData,
}

/// Sends a command to the IMU.
pub fn command(
    device: &mut Can,
    cmd_code: u8,
) -> Result<(), CanError> {
    check(unsafe {
        ffi::GENERIC_IMU_CommandDevice(
            &mut device.inner,
            cmd_code,
        )
    })
}

/// Requests housekeeping telemetry from the IMU.
pub fn request_hk(
    device: &mut Can,
) -> Result<ImuHk, CanError> {
    let mut raw = ffi::GENERIC_IMU_Device_HK_tlm_t::default();
    check(unsafe {
        ffi::GENERIC_IMU_RequestHK(&mut device.inner, &mut raw)
    })?;
    Ok(ImuHk {
        device_counter: raw.DeviceCounter,
        device_status: raw.DeviceStatus,
    })
}

/// Requests single-axis data from the IMU.
pub fn request_axis(
    device: &mut Can,
    cmd_code: u8,
) -> Result<AxisData, CanError> {
    let mut raw = ffi::GENERIC_IMU_Device_Axis_Data_t::default();
    check(unsafe {
        ffi::GENERIC_IMU_RequestAxis(
            &mut device.inner,
            &mut raw,
            cmd_code,
        )
    })?;
    Ok(AxisData {
        linear_acc: raw.LinearAcc,
        angular_acc: raw.AngularAcc,
    })
}

/// Requests full three-axis data from the IMU.
pub fn request_data(
    device: &mut Can,
) -> Result<ImuData, CanError> {
    let mut raw = ffi::GENERIC_IMU_Device_Data_tlm_t::default();
    check(unsafe {
        ffi::GENERIC_IMU_RequestData(
            &mut device.inner,
            &mut raw,
        )
    })?;
    Ok(ImuData {
        x: AxisData {
            linear_acc: raw.X_Data.LinearAcc,
            angular_acc: raw.X_Data.AngularAcc,
        },
        y: AxisData {
            linear_acc: raw.Y_Data.LinearAcc,
            angular_acc: raw.Y_Data.AngularAcc,
        },
        z: AxisData {
            linear_acc: raw.Z_Data.LinearAcc,
            angular_acc: raw.Z_Data.AngularAcc,
        },
    })
}
