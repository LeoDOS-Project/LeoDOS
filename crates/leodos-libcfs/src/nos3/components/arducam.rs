//! ArduCam camera device driver wrapper.
//!
//! Wraps the `CAM_*` and `take_picture` device functions.
//! The camera uses global I2C/SPI bus state internally,
//! so these are free functions (no device parameter).

use crate::ffi;

/// Camera error.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CamError {
    /// Initialisation or operation failed.
    #[error("Camera: error ({0})")]
    Failed(i32),
}

fn check_cam(rc: i32) -> Result<(), CamError> {
    if rc == 0 { Ok(()) } else { Err(CamError::Failed(rc)) }
}

/// Image resolution preset.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Resolution {
    /// 160 x 120.
    R160x120 = 0,
    /// 320 x 240.
    R320x240 = 1,
    /// 800 x 600.
    R800x600 = 2,
    /// 1600 x 1200.
    R1600x1200 = 3,
    /// 2592 x 1944.
    R2592x1944 = 4,
}

/// Initialises the camera I2C bus.
pub fn init_i2c() -> Result<(), CamError> {
    check_cam(unsafe { ffi::CAM_init_i2c() })
}

/// Initialises the camera SPI bus.
pub fn init_spi() -> Result<(), CamError> {
    check_cam(unsafe { ffi::CAM_init_spi() })
}

/// Configures the camera sensor.
pub fn config() -> Result<(), CamError> {
    check_cam(unsafe { ffi::CAM_config() })
}

/// Prepares the camera for capture.
pub fn capture_prep() -> Result<(), CamError> {
    check_cam(unsafe { ffi::CAM_capture_prep() })
}

/// Triggers image capture.
pub fn capture() -> Result<(), CamError> {
    check_cam(unsafe { ffi::CAM_capture() })
}

/// Reads the FIFO buffer length (image size in bytes).
pub fn read_fifo_length() -> Result<u32, CamError> {
    let mut length: u32 = 0;
    check_cam(unsafe {
        ffi::CAM_read_fifo_length(&mut length)
    })?;
    Ok(length)
}

/// Takes a picture at the given resolution.
///
/// Returns the result code from the camera driver.
pub fn take_picture(size: Resolution) -> i32 {
    unsafe { ffi::take_picture(size as u8) }
}
