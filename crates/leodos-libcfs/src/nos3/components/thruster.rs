//! Generic thruster device driver wrapper.
//!
//! Wraps the `GENERIC_THRUSTER_*` device function for
//! orbit/attitude maneuvers over a UART bus.

use crate::ffi;
use crate::nos3::{check_uart, UartError};
use crate::nos3::uart::Uart;

/// Commands a thruster to a given duty percentage.
pub fn set_percentage(
    device: &mut Uart,
    thruster_number: u8,
    percentage: u8,
    data_length: u8,
) -> Result<(), UartError> {
    check_uart(unsafe {
        ffi::GENERIC_THRUSTER_SetPercentage(
            &mut device.inner,
            thruster_number,
            percentage,
            data_length,
        )
    })
}
