//! NOS3 hardware library (hwlib) safe wrappers.
//!
//! Provides RAII-based, safe Rust interfaces for the NOS3 hwlib
//! bus drivers: UART, I2C, SPI, and GPIO. Each device is opened
//! via a constructor and automatically closed on drop.

pub mod uart;
pub mod i2c;
pub mod spi;
pub mod gpio;

/// Raw FFI re-exports for advanced use.
pub mod ffi {
    pub use crate::ffi::{
        uart_access_flag, uart_info_t,
        i2c_bus_info_t,
        spi_info_t, spi_mutex_t,
        gpio_info_t,
        canid_t, can_info_t,
        can_init_dev, can_set_modes, can_write, can_read,
        can_close_device, can_master_transaction,
        socket_info_t, addr_fam_e, type_e, category_e,
        socket_create, socket_listen, socket_accept,
        socket_connect, socket_send, socket_recv, socket_close,
        trq_info_t, trq_init, trq_command, trq_close,
        trq_set_time_high, trq_set_period, trq_set_direction,
        devmem_write, devmem_read,
    };
}

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
