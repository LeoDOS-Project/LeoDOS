//! Safe I2C master wrapper.
//!
//! Wraps the hwlib `i2c_*` functions with RAII lifetime
//! management. The bus is closed automatically on drop.

use super::{check_i2c, I2cError};
use crate::ffi;
use core::mem::MaybeUninit;

/// An open I2C master bus.
///
/// Created via [`I2cBus::open`]. Automatically closes the bus
/// when dropped.
pub struct I2cBus {
    inner: ffi::i2c_bus_info_t,
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
        check_i2c(unsafe { ffi::i2c_master_init(&mut info) })?;
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
        check_i2c(unsafe {
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
        check_i2c(unsafe {
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
        check_i2c(unsafe {
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
