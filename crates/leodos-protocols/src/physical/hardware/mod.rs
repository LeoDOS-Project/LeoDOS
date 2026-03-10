//! Hardware-backed physical channel implementations.
//!
//! Implements [`PhysicalWriter`] and [`PhysicalReader`] directly
//! on the hwlib bus types from [`leodos_libcfs::nos3`], or via
//! thin wrappers that bind per-call parameters (address, ID).

/// [`PhysicalWriter`] / [`PhysicalReader`] for UART.
#[cfg(feature = "nos3")]
pub mod uart;
/// [`PhysicalWriter`] / [`PhysicalReader`] for SPI.
#[cfg(feature = "nos3")]
pub mod spi;
/// [`BoundSocket`] — socket with fixed remote address.
#[cfg(feature = "nos3")]
pub mod socket;
/// [`I2cChannel`] — I2C bus bound to a slave address.
#[cfg(feature = "nos3")]
pub mod i2c;
/// [`CanChannel`] — CAN bus bound to a transmit ID.
#[cfg(feature = "nos3")]
pub mod can;
