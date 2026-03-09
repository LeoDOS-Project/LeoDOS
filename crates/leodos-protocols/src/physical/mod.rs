//! Physical layer: modulation, demodulation, and channel I/O.
//!
//! Defines the async traits for reading/writing raw bytes on a
//! physical channel. Higher layers (coding, datalink) depend on
//! these traits — this module has no upward dependencies.

use core::future::Future;

/// Modulation and demodulation schemes (BPSK, QPSK, OQPSK, 8PSK, GMSK).
pub mod modulator;
/// Hardware-backed physical channel implementations.
pub mod hardware;

/// Async trait for writing raw bytes to a physical channel.
pub trait PhysicalWriter {
    /// Error type for write operations.
    type Error;

    /// Writes the given data bytes to the physical channel.
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Async trait for reading raw bytes from a physical channel.
pub trait PhysicalReader {
    /// Error type for read operations.
    type Error;

    /// Reads bytes into the buffer, returning the number of bytes read.
    fn read(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
