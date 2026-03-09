//! Hardware bus drivers (UART, I2C, SPI, CAN, GPIO, socket,
//! torquer, memory-mapped I/O).

/// UART (serial) bus.
pub mod uart;
/// I2C (two-wire) bus.
pub mod i2c;
/// SPI (synchronous serial) bus.
pub mod spi;
/// GPIO (general-purpose I/O) pins.
pub mod gpio;
/// CAN (controller area network) bus.
pub mod can;
/// Network sockets (TCP/UDP).
pub mod socket;
/// Magnetorquer PWM control.
pub mod trq;
/// Memory-mapped I/O.
pub mod mem;
