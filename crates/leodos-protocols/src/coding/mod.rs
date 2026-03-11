/// Forward error-correction codes (Reed-Solomon, LDPC, convolutional).
pub mod fec;
/// Frame synchronization and transmission unit encoding (CADU, CLTU).
pub mod framing;
/// CCSDS data compression algorithms (Rice, CCSDS 122, CCSDS 123).
pub mod compression;
/// CCSDS pseudo-randomization for TC and TM frames.
pub mod randomizer;
/// CRC-protected Space Packet wrapper.
pub mod crc;
mod checksum;

use core::convert::Infallible;
use core::future::Future;

/// Coding pipeline that composes randomizer, FEC, and framer.
pub mod pipeline;

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

// ── Group traits ───────────────────────────────────────────────

/// Forward error-correction encoder (Reed-Solomon, LDPC, convolutional).
pub trait FecEncoder {
    /// Error type for encoding operations.
    type Error;
    /// Encodes `data` with FEC parity into `output`.
    fn encode(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Forward error-correction decoder.
pub trait FecDecoder {
    /// Error type for decoding operations.
    type Error;
    /// Decodes and corrects `data` in-place.
    fn decode(&self, data: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Wraps coded data for transmission (ASM for TM, CLTU for TC).
pub trait Framer {
    /// Error type for framing operations.
    type Error;
    /// Frames `data` into `output` (e.g. prepends ASM).
    fn frame(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Extracts coded data from a framed transmission.
pub trait Deframer {
    /// Error type for deframing operations.
    type Error;
    /// Strips framing from `data` and writes the payload to `output`.
    fn deframe(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Lossless or lossy data compression (applied to payload, not frames).
pub trait Compressor {
    /// Error type for compression operations.
    type Error;
    /// Compresses `input` into `output`.
    fn compress(&self, input: &[u8], output: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Decompresses previously compressed data.
pub trait Decompressor {
    /// Error type for decompression operations.
    type Error;
    /// Decompresses `input` into `output`.
    fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<usize, Self::Error>;
}

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
