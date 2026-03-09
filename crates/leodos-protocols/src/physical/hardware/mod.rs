//! Hardware-backed physical channel implementations.
//!
//! Implements [`PhysicalWriter`] and [`PhysicalReader`] directly
//! on the hwlib bus types from [`leodos_libcfs::nos3`], or via
//! thin wrappers that bind per-call parameters (address, ID).

/// [`PhysicalWriter`] / [`PhysicalReader`] for UART.
#[cfg(feature = "cfs")]
pub mod uart;
/// [`PhysicalWriter`] / [`PhysicalReader`] for SPI.
#[cfg(feature = "cfs")]
pub mod spi;
/// [`BoundSocket`] — socket with fixed remote address.
#[cfg(feature = "cfs")]
pub mod socket;
/// [`I2cChannel`] — I2C bus bound to a slave address.
#[cfg(feature = "cfs")]
pub mod i2c;
/// [`CanChannel`] — CAN bus bound to a transmit ID.
#[cfg(feature = "cfs")]
pub mod can;
