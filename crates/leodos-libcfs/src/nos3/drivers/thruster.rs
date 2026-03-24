//! Thruster (chemical or cold-gas).
//!
//! Commands individual thrusters to a duty-cycle percentage
//! for orbit maintenance, station-keeping, and coarse attitude
//! manoeuvres. Communicates over UART.

use crate::ffi;
use crate::nos3::buses::uart::check;
use crate::nos3::buses::uart::Uart;
use crate::nos3::buses::uart::UartError;

/// Commands a thruster to a given duty percentage.
pub fn set_percentage(
    device: &mut Uart,
    thruster_number: u8,
    percentage: u8,
    data_length: u8,
) -> Result<(), UartError> {
    check(unsafe {
        ffi::GENERIC_THRUSTER_SetPercentage(
            &mut device.inner,
            thruster_number,
            percentage,
            data_length,
        )
    })
}
