//! Hardware-backed physical channel implementations.
//!
//! Implements [`PhysicalWrite`] and [`PhysicalRead`] directly
//! on the hwlib bus types from [`leodos_libcfs::nos3`], or via
//! thin wrappers that bind per-call parameters (address, ID).

/// [`PhysicalWrite`] / [`PhysicalRead`] for UART.
pub mod uart;
/// [`PhysicalWrite`] / [`PhysicalRead`] for SPI.
pub mod spi;
/// [`BoundSocket`] — socket with fixed remote address.
pub mod socket;
/// [`I2cChannel`] — I2C bus bound to a slave address.
pub mod i2c;
/// [`CanChannel`] — CAN bus bound to a transmit ID.
pub mod can;
