//! GPIO (General-Purpose Input/Output) pin control.
//!
//! GPIO pins provide digital signal lines for discrete
//! spacecraft hardware control — enable/disable switches,
//! deployment indicators, and status lines. The pin is
//! closed on drop.

use crate::ffi;
use core::mem::MaybeUninit;

/// Errors from GPIO operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum GpioError {
    /// Generic OS/driver error (`GPIO_ERROR`).
    #[error("GPIO: OS error")]
    OsError,
    /// File descriptor open error (`GPIO_FD_OPEN_ERR`).
    #[error("GPIO: file descriptor open error")]
    FdOpen,
    /// Write error (`GPIO_WRITE_ERR`).
    #[error("GPIO: write error")]
    Write,
    /// Read error (`GPIO_READ_ERR`).
    #[error("GPIO: read error")]
    Read,
    /// Unhandled error code.
    #[error("GPIO: unhandled error ({0})")]
    Unhandled(i32),
}

fn check(rc: i32) -> Result<(), GpioError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::GPIO_ERROR => Err(GpioError::OsError),
        _ if rc == ffi::GPIO_FD_OPEN_ERR => Err(GpioError::FdOpen),
        _ if rc == ffi::GPIO_WRITE_ERR => Err(GpioError::Write),
        _ if rc == ffi::GPIO_READ_ERR => Err(GpioError::Read),
        other => Err(GpioError::Unhandled(other)),
    }
}

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
        check(unsafe { ffi::gpio_init(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Reads the current value of the pin.
    pub fn read(&mut self) -> Result<u8, GpioError> {
        let mut value: u8 = 0;
        check(unsafe { ffi::gpio_read(&mut self.inner, &mut value) })?;
        Ok(value)
    }

    /// Writes a value (0 or 1) to the pin.
    pub fn write(&mut self, value: u8) -> Result<(), GpioError> {
        check(unsafe { ffi::gpio_write(&mut self.inner, value) })
    }
}

impl Drop for Gpio {
    fn drop(&mut self) {
        unsafe {
            ffi::gpio_close(&mut self.inner);
        }
    }
}
