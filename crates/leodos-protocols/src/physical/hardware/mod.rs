//! Hardware-backed physical channel implementations.
//!
//! Implements [`PhysicalWriter`] and [`PhysicalReader`] directly
//! on the hwlib bus types from [`leodos_libcfs::nos3`].

/// [`PhysicalWriter`] / [`PhysicalReader`] for UART.
#[cfg(feature = "cfs")]
pub mod uart;
/// [`PhysicalWriter`] / [`PhysicalReader`] for SPI.
#[cfg(feature = "cfs")]
pub mod spi;
