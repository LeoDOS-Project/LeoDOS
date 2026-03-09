//! I2C (Inter-Integrated Circuit) master bus.
//!
//! I2C is a two-wire serial bus used by spacecraft subsystems
//! such as power systems (EPS), sun sensors (CSS), and cameras.
//! The bus is closed on drop.

use crate::ffi;
use core::mem::MaybeUninit;

/// Errors from I2C operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum I2cError {
    /// Generic OS/driver error (`I2C_ERROR`).
    #[error("I2C: OS error")]
    OsError,
    /// File descriptor open error (`I2C_FD_OPEN_ERR`).
    #[error("I2C: file descriptor open error")]
    FdOpen,
    /// Unhandled error code.
    #[error("I2C: unhandled error ({0})")]
    Unhandled(i32),
}

pub(crate) fn check(rc: i32) -> Result<(), I2cError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::I2C_ERROR => Err(I2cError::OsError),
        _ if rc == ffi::I2C_FD_OPEN_ERR => Err(I2cError::FdOpen),
        other => Err(I2cError::Unhandled(other)),
    }
}

/// An open I2C master bus.
pub struct I2cBus {
    pub(crate) inner: ffi::i2c_bus_info_t,
}

impl I2cBus {
    /// Opens an I2C bus as master.
    ///
    /// `addr` is the default slave address. `speed` is the bus
    /// speed in kbps.
    pub fn open(addr: i32, speed: u32) -> Result<Self, I2cError> {
        let mut info: ffi::i2c_bus_info_t = unsafe {
            MaybeUninit::zeroed().assume_init()
        };
        info.addr = addr;
        info.speed = speed;
        info.isOpen = 0;
        check(unsafe { ffi::i2c_master_init(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Performs a write-then-read transaction.
    ///
    /// Writes `tx` bytes to `addr`, then reads into `rx`.
    /// `timeout` is in ticks.
    pub fn write_read(
        &mut self,
        addr: u8,
        tx: &[u8],
        rx: &mut [u8],
        timeout: u16,
    ) -> Result<(), I2cError> {
        check(unsafe {
            ffi::i2c_master_transaction(
                &mut self.inner,
                addr,
                tx.as_ptr() as *mut _,
                tx.len() as u8,
                rx.as_mut_ptr() as *mut _,
                rx.len() as u8,
                timeout,
            )
        })
    }

    /// Reads from a slave device.
    pub fn read(
        &mut self,
        addr: u8,
        rx: &mut [u8],
        timeout: u8,
    ) -> Result<(), I2cError> {
        check(unsafe {
            ffi::i2c_read_transaction(
                &mut self.inner,
                addr,
                rx.as_mut_ptr() as *mut _,
                rx.len() as u8,
                timeout,
            )
        })
    }

    /// Writes to a slave device.
    pub fn write(
        &mut self,
        addr: u8,
        tx: &[u8],
        timeout: u8,
    ) -> Result<(), I2cError> {
        check(unsafe {
            ffi::i2c_write_transaction(
                &mut self.inner,
                addr,
                tx.as_ptr() as *mut _,
                tx.len() as u8,
                timeout,
            )
        })
    }
}

impl Drop for I2cBus {
    fn drop(&mut self) {
        unsafe { ffi::i2c_master_close(&mut self.inner); }
    }
}
