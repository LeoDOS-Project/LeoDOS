//! GPIO (General-Purpose Input/Output) pin control.
//!
//! GPIO pins provide digital signal lines for discrete
//! spacecraft hardware control — enable/disable switches,
//! deployment indicators, and status lines. The pin is
//! closed on drop.

use super::{check_gpio, GpioError};
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
pub struct Gpio {
    inner: ffi::gpio_info_t,
}

impl Gpio {
    /// Opens and initialises a GPIO pin.
    ///
    /// `pin` is the hardware pin number. `direction` selects
    /// input or output mode.
    pub fn open(pin: u32, direction: Direction) -> Result<Self, GpioError> {
        let mut info: ffi::gpio_info_t = unsafe { MaybeUninit::zeroed().assume_init() };
        info.pin = pin;
        info.direction = match direction {
            Direction::Input => 0,
            Direction::Output => 1,
        };
        info.isOpen = 0;
        check_gpio(unsafe { ffi::gpio_init(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Reads the current value of the pin.
    pub fn read(&mut self) -> Result<u8, GpioError> {
        let mut value: u8 = 0;
        check_gpio(unsafe { ffi::gpio_read(&mut self.inner, &mut value) })?;
        Ok(value)
    }

    /// Writes a value (0 or 1) to the pin.
    pub fn write(&mut self, value: u8) -> Result<(), GpioError> {
        check_gpio(unsafe { ffi::gpio_write(&mut self.inner, value) })
    }
}

impl Drop for Gpio {
    fn drop(&mut self) {
        unsafe {
            ffi::gpio_close(&mut self.inner);
        }
    }
}
