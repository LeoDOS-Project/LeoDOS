//! Physical layer: modulation, demodulation, and channel I/O.
//!
//! Defines the async traits for reading/writing raw bytes on a
//! physical channel. Higher layers (coding, datalink) depend on
//! these traits — this module has no upward dependencies.

use core::future::Future;

/// BPSK and QPSK modulation/demodulation.
pub mod modulation;
/// Offset QPSK modulation/demodulation (Proximity-1).
pub mod oqpsk;
/// Gray-coded 8PSK modulation/demodulation.
pub mod eight_psk;
/// Gaussian Minimum Shift Keying (GMSK) modulation.
pub mod gmsk;

/// CFS/hwlib-backed physical channel (UART → NOS Engine).
#[cfg(feature = "cfs")]
pub mod cfs;

/// Async trait for writing raw bytes to a physical channel.
pub trait AsyncPhysicalWriter {
    /// Error type for write operations.
    type Error;

    /// Writes the given data bytes to the physical channel.
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Async trait for reading raw bytes from a physical channel.
pub trait AsyncPhysicalReader {
    /// Error type for read operations.
    type Error;

    /// Reads bytes into the buffer, returning the number of bytes read.
    fn read(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
