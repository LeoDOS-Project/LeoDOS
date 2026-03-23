//! CCSDS data compression algorithms.

/// CCSDS 121.0-B-3 Lossless Data Compression (Rice coding).
pub mod rice;
/// CCSDS 122.0-B-2 Image Data Compression (wavelet-based).
pub mod ccsds122;
/// CCSDS 123.0-B-2 Low-Complexity Lossless Multispectral &
/// Hyperspectral Image Compression.
pub mod ccsds123;
/// CCSDS 122.1-B-1 Spectral Preprocessing Transform for
/// Multispectral and Hyperspectral Image Compression.
pub mod spectral;

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
