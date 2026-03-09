//! CCSDS data compression algorithms.

/// CCSDS 121.0-B-3 Lossless Data Compression (Rice coding).
pub mod rice;
/// CCSDS 122.0-B-2 Image Data Compression (wavelet-based).
pub mod ccsds122;
/// CCSDS 123.0-B-2 Low-Complexity Lossless Multispectral &
/// Hyperspectral Image Compression.
pub mod ccsds123;
