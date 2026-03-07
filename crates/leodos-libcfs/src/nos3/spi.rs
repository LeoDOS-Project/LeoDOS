//! Safe SPI device wrapper.
//!
//! Wraps the hwlib `spi_*` functions with RAII lifetime
//! management. The device is closed automatically on drop.

use super::{check_spi, SpiError};
use crate::ffi;
use core::mem::MaybeUninit;

/// An open SPI device.
///
/// Created via [`Spi::open`]. Automatically closes the device
/// when dropped.
pub struct Spi {
    inner: ffi::spi_info_t,
}

impl Spi {
    /// Opens an SPI device.
    ///
    /// - `device`: OS device path (e.g. `/dev/spidev0.0`)
    /// - `bus`: bus number (for mutex selection)
    /// - `cs`: chip-select line
    /// - `baudrate`: clock speed in Hz
    /// - `spi_mode`: SPI mode (0-3)
    /// - `bits_per_word`: typically 8
    pub fn open(
        device: &core::ffi::CStr,
        bus: u8,
        cs: u8,
        baudrate: u32,
        spi_mode: u8,
        bits_per_word: u8,
    ) -> Result<Self, SpiError> {
        let mut info: ffi::spi_info_t = unsafe {
            MaybeUninit::zeroed().assume_init()
        };
        info.deviceString = device.as_ptr();
        info.bus = bus;
        info.cs = cs;
        info.baudrate = baudrate;
        info.spi_mode = spi_mode;
        info.bits_per_word = bits_per_word;
        info.isOpen = 0;
        check_spi(unsafe { ffi::spi_init_dev(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Writes bytes to the SPI device.
    pub fn write(&mut self, data: &[u8]) -> Result<(), SpiError> {
        check_spi(unsafe {
            ffi::spi_write(
                &mut self.inner,
                data.as_ptr() as *mut u8,
                data.len() as u32,
            )
        })
    }

    /// Reads bytes from the SPI device.
    pub fn read(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(), SpiError> {
        check_spi(unsafe {
            ffi::spi_read(
                &mut self.inner,
                buf.as_mut_ptr(),
                buf.len() as u32,
            )
        })
    }

    /// Performs a full-duplex SPI transaction.
    ///
    /// - `tx`: data to transmit (or empty to shift out zeros)
    /// - `rx`: buffer for received data (or empty to discard)
    /// - `len`: number of bytes to transfer
    /// - `delay`: post-transfer delay (µs)
    /// - `bits`: bits per word for this transfer
    /// - `deselect`: if true, deassert CS after transfer
    pub fn transfer(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        len: u32,
        delay: u16,
        bits: u8,
        deselect: bool,
    ) -> Result<(), SpiError> {
        let tx_ptr = if tx.is_empty() {
            core::ptr::null_mut()
        } else {
            tx.as_ptr() as *mut u8
        };
        let rx_ptr = if rx.is_empty() {
            core::ptr::null_mut()
        } else {
            rx.as_mut_ptr()
        };
        check_spi(unsafe {
            ffi::spi_transaction(
                &mut self.inner,
                tx_ptr,
                rx_ptr,
                len,
                delay,
                bits,
                deselect as u8,
            )
        })
    }

    /// Manually asserts chip-select.
    pub fn select(&mut self) -> Result<(), SpiError> {
        check_spi(unsafe { ffi::spi_select_chip(&mut self.inner) })
    }

    /// Manually deasserts chip-select.
    pub fn deselect(&mut self) -> Result<(), SpiError> {
        check_spi(unsafe {
            ffi::spi_unselect_chip(&mut self.inner)
        })
    }
}

impl Drop for Spi {
    fn drop(&mut self) {
        unsafe { ffi::spi_close_device(&mut self.inner); }
    }
}
