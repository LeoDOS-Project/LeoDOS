//! Safe UART (serial port) wrapper.
//!
//! Wraps the hwlib `uart_*` functions with RAII lifetime
//! management. The port is closed automatically on drop.

use super::{check_uart, check_uart_count, UartError};
use crate::ffi;
use core::mem::MaybeUninit;

/// UART access mode.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Access {
    /// Read-only.
    ReadOnly,
    /// Write-only.
    WriteOnly,
    /// Read and write.
    ReadWrite,
}

/// An open UART port.
///
/// Created via [`Uart::open`]. Automatically closes the port
/// when dropped.
pub struct Uart {
    inner: ffi::uart_info_t,
}

impl Uart {
    /// Opens a UART port.
    ///
    /// `device` is the OS device path (e.g. `/dev/ttyS0`).
    /// `baud` is the baud rate. `access` selects read/write mode.
    pub fn open(
        device: &core::ffi::CStr,
        baud: u32,
        access: Access,
    ) -> Result<Self, UartError> {
        let mut info: ffi::uart_info_t = unsafe {
            MaybeUninit::zeroed().assume_init()
        };
        info.deviceString = device.as_ptr();
        info.baud = baud;
        info.isOpen = 0;
        info.access_option = match access {
            Access::ReadOnly => ffi::uart_access_flag_uart_access_flag_RDONLY,
            Access::WriteOnly => ffi::uart_access_flag_uart_access_flag_WRONLY,
            Access::ReadWrite => ffi::uart_access_flag_uart_access_flag_RDWR,
        };
        check_uart(unsafe { ffi::uart_init_port(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Reads up to `buf.len()` bytes from the port.
    ///
    /// Returns the number of bytes actually read.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, UartError> {
        let rc = unsafe {
            ffi::uart_read_port(
                &mut self.inner,
                buf.as_mut_ptr(),
                buf.len() as u32,
            )
        };
        check_uart_count(rc)
    }

    /// Writes bytes to the port.
    ///
    /// Returns the number of bytes actually written.
    pub fn write(&mut self, data: &[u8]) -> Result<usize, UartError> {
        let rc = unsafe {
            ffi::uart_write_port(
                &mut self.inner,
                data.as_ptr() as *mut u8,
                data.len() as u32,
            )
        };
        check_uart_count(rc)
    }

    /// Returns the number of bytes waiting to be read.
    pub fn bytes_available(&mut self) -> Result<usize, UartError> {
        let rc = unsafe {
            ffi::uart_bytes_available(&mut self.inner)
        };
        check_uart_count(rc)
    }

    /// Flushes the receive buffer.
    pub fn flush(&mut self) -> Result<(), UartError> {
        check_uart(unsafe { ffi::uart_flush(&mut self.inner) })
    }
}

impl Drop for Uart {
    fn drop(&mut self) {
        unsafe { ffi::uart_close_port(&mut self.inner); }
    }
}
