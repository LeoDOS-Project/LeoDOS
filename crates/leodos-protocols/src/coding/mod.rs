/// Forward error-correction codes (Reed-Solomon, LDPC, convolutional).
pub mod fec;
/// Frame synchronization and transmission unit encoding (CADU, CLTU).
pub mod framing;
/// Re-export from application layer for backward compatibility.
pub use crate::application::compression;
/// CCSDS pseudo-randomization for TC and TM frames.
pub mod randomizer;
/// CRC-protected Space Packet wrapper.
pub mod crc;

use core::convert::Infallible;
use core::future::Future;

/// Coding pipeline that composes randomizer, FEC, and framer.
pub mod pipeline;
/// Proximity-1 coding pipeline (CCSDS 211.2-B-3).
pub mod proximity1;

// ── Layer boundary traits ──────────────────────────────────────

/// Accepts a transfer frame and writes it through the coding chain
/// to the physical layer.
pub trait CodingWrite {
    /// Error type for write operations.
    type Error: core::error::Error;
    /// Encodes and writes a transfer frame.
    fn write(&mut self, frame: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Reads coded bytes from the physical layer, decodes them, and
/// returns the transfer frame.
pub trait CodingRead {
    /// Error type for read operations.
    type Error: core::error::Error;
    /// Reads and decodes a transfer frame into `buffer`.
    fn read(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}

// ── Group traits (re-exported from their respective modules) ──

pub use fec::FecEncoder;
pub use fec::FecDecoder;
pub use framing::Framer;
pub use framing::Deframer;

// ── No-op types ──────────────────────────────────────────────

/// No-op randomizer that passes data through unchanged.
pub struct NoRandomizer;

impl randomizer::Randomizer for NoRandomizer {
    fn apply(&self, _buffer: &mut [u8]) {}
    fn table(&self) -> &[u8] {
        &[]
    }
}

/// No-op FEC that passes data through unchanged.
pub struct NoFec;

impl FecEncoder for NoFec {
    type Error = Infallible;
    fn encode(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error> {
        let len = data.len();
        output[..len].copy_from_slice(data);
        Ok(len)
    }
}

impl FecDecoder for NoFec {
    type Error = Infallible;
    fn decode(&self, data: &mut [u8]) -> Result<usize, Self::Error> {
        Ok(data.len())
    }
}

/// No-op framer that passes data through unchanged.
pub struct NoFramer;

impl Framer for NoFramer {
    type Error = Infallible;
    fn frame(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error> {
        let len = data.len();
        output[..len].copy_from_slice(data);
        Ok(len)
    }
}

impl Deframer for NoFramer {
    type Error = Infallible;
    fn deframe(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error> {
        let len = data.len();
        output[..len].copy_from_slice(data);
        Ok(len)
    }
}
