//! NOS3 hardware library (hwlib) safe wrappers.
//!
//! Provides RAII-based, safe Rust interfaces for the NOS3 hwlib
//! bus drivers: UART, I2C, SPI, and GPIO. Each device is opened
//! via a constructor and automatically closed on drop.

pub mod uart;
pub mod i2c;
pub mod spi;
pub mod gpio;
pub mod can;
pub mod socket;
pub mod trq;
pub mod mem;

/// Errors from hwlib operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum HwError {
    /// Generic OS/driver error (OS_ERROR = -1).
    OsError,
    /// File descriptor / device open error (OS_ERR_FILE = -2).
    FileError,
    /// Device-specific error code.
    DeviceError(i32),
}

impl core::fmt::Display for HwError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OsError => write!(f, "hwlib: OS error"),
            Self::FileError => write!(f, "hwlib: file/device error"),
            Self::DeviceError(c) => write!(f, "hwlib: error {c}"),
        }
    }
}

/// Converts a hwlib i32 return code to a Result.
fn check(rc: i32) -> Result<(), HwError> {
    match rc {
        0 => Ok(()),       // OS_SUCCESS
        -1 => Err(HwError::OsError),
        -2 => Err(HwError::FileError),
        other => Err(HwError::DeviceError(other)),
    }
}

/// Converts a hwlib i32 return code that returns a byte count.
/// Positive = success (byte count), negative = error.
fn check_count(rc: i32) -> Result<usize, HwError> {
    if rc >= 0 {
        Ok(rc as usize)
    } else {
        match rc {
            -1 => Err(HwError::OsError),
            -2 => Err(HwError::FileError),
            other => Err(HwError::DeviceError(other)),
        }
    }
}
