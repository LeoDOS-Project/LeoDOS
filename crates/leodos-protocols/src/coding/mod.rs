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

use core::future::Future;

// ── Layer boundary traits ──────────────────────────────────────

/// Accepts a transfer frame and writes it through the coding chain
/// to the physical layer.
pub trait CodingWriter {
    /// Error type for write operations.
    type Error;
    /// Encodes and writes a transfer frame.
    fn write(&mut self, frame: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Reads coded bytes from the physical layer, decodes them, and
/// returns the transfer frame.
pub trait CodingReader {
    /// Error type for read operations.
    type Error;
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
