//! Safe GPIO pin wrapper.
//!
//! Wraps the hwlib `gpio_*` functions with RAII lifetime
//! management. The pin is closed automatically on drop.

use super::{check, HwError};
use crate::ffi;
use core::mem::MaybeUninit;

/// GPIO pin direction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Direction {
    /// Input pin.
    Input,
    /// Output pin.
    Output,
}

/// An open GPIO pin.
///
/// Created via [`Gpio::open`]. Automatically closes the pin
/// when dropped.
pub struct Gpio {
    inner: ffi::gpio_info_t,
}

impl Gpio {
    /// Opens and initialises a GPIO pin.
    ///
    /// `pin` is the hardware pin number. `direction` selects
    /// input or output mode.
    pub fn open(
        pin: u32,
        direction: Direction,
    ) -> Result<Self, HwError> {
        let mut info: ffi::gpio_info_t = unsafe {
            MaybeUninit::zeroed().assume_init()
        };
        info.pin = pin;
        info.direction = match direction {
            Direction::Input => 0,
            Direction::Output => 1,
        };
        info.isOpen = 0;
        check(unsafe { ffi::gpio_init(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Reads the current value of the pin.
    pub fn read(&mut self) -> Result<u8, HwError> {
        let mut value: u8 = 0;
        check(unsafe {
            ffi::gpio_read(&mut self.inner, &mut value)
        })?;
        Ok(value)
    }

    /// Writes a value (0 or 1) to the pin.
    pub fn write(&mut self, value: u8) -> Result<(), HwError> {
        check(unsafe {
            ffi::gpio_write(&mut self.inner, value)
        })
    }
}

impl Drop for Gpio {
    fn drop(&mut self) {
        unsafe { ffi::gpio_close(&mut self.inner); }
    }
}
