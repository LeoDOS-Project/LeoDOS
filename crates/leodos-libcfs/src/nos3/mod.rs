//! NOS3 (NASA Operational Simulator for Small Sats) interfaces.
//!
//! [`buses`] — hardware bus drivers (UART, I2C, SPI, CAN, GPIO,
//! socket, torquer, memory-mapped I/O).
//!
//! [`drivers`] — simulated spacecraft component drivers (sensors,
//! actuators, radio, GPS, camera).

/// Hardware bus drivers.
pub mod buses;
/// Simulated spacecraft component drivers.
pub mod drivers;

// Re-export error types at the `nos3` level for convenience.
pub use buses::uart::UartError;
pub use buses::i2c::I2cError;
pub use buses::spi::SpiError;
pub use buses::gpio::GpioError;
pub use buses::can::CanError;
pub use buses::socket::SocketError;
pub use buses::trq::TrqError;
pub use buses::mem::MemError;
