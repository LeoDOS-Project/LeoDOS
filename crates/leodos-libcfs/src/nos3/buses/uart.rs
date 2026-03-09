//! UART (Universal Asynchronous Receiver-Transmitter) bus.
//!
//! UART is a serial communication interface used by spacecraft
//! subsystems such as star trackers, reaction wheels, GPS
//! receivers, and thrusters. The port is closed on drop.

use crate::ffi;
use core::mem::MaybeUninit;

/// Errors from UART operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum UartError {
    /// Generic OS/driver error (`UART_ERROR`).
    #[error("UART: OS error")]
    OsError,
    /// File descriptor open error (`UART_FD_OPEN`).
    #[error("UART: file descriptor open error")]
    FdOpen,
    /// Unhandled error code.
    #[error("UART: unhandled error ({0})")]
    Unhandled(i32),
}

pub(crate) fn check(rc: i32) -> Result<(), UartError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::UART_ERROR => Err(UartError::OsError),
        _ if rc == ffi::UART_FD_OPEN => Err(UartError::FdOpen),
        other => Err(UartError::Unhandled(other)),
    }
}

fn check_count(rc: i32) -> Result<usize, UartError> {
    if rc >= 0 {
        Ok(rc as usize)
    } else {
        Err(match rc {
            _ if rc == ffi::UART_ERROR => UartError::OsError,
            _ if rc == ffi::UART_FD_OPEN => UartError::FdOpen,
            other => UartError::Unhandled(other),
        })
    }
}

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
pub struct Uart {
    pub(crate) inner: ffi::uart_info_t,
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
        check(unsafe { ffi::uart_init_port(&mut info) })?;
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
        check_count(rc)
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
        check_count(rc)
    }

    /// Returns the number of bytes waiting to be read.
    pub fn bytes_available(&mut self) -> Result<usize, UartError> {
        let rc = unsafe {
            ffi::uart_bytes_available(&mut self.inner)
        };
        check_count(rc)
    }

    /// Flushes the receive buffer.
    pub fn flush(&mut self) -> Result<(), UartError> {
        check(unsafe { ffi::uart_flush(&mut self.inner) })
    }
}

impl Drop for Uart {
    fn drop(&mut self) {
        unsafe { ffi::uart_close_port(&mut self.inner); }
    }
}
