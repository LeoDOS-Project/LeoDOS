//! Reaction wheel.
//!
//! A flywheel spun by a brushless motor to exchange angular
//! momentum with the spacecraft, providing precise three-axis
//! attitude control. Communicates over UART.

use crate::ffi;
use crate::nos3::{check_uart, UartError};
use crate::nos3::buses::uart::Uart;

/// Reads the current angular momentum from a reaction wheel.
pub fn get_momentum(
    wheel: &mut Uart,
) -> Result<f64, UartError> {
    let mut momentum: f64 = 0.0;
    check_uart(unsafe {
        ffi::GetCurrentMomentum(
            &mut wheel.inner,
            &mut momentum,
        )
    })?;
    Ok(momentum)
}

/// Commands a torque value to a reaction wheel.
pub fn set_torque(
    wheel: &mut Uart,
    torque: f64,
) -> Result<(), UartError> {
    check_uart(unsafe {
        ffi::SetRWTorque(&mut wheel.inner, torque)
    })
}
