/// Channel Access Data Unit — ASM framing and frame sync (CCSDS 131.0-B-5).
pub mod cadu;
mod checksum;
/// CCSDS LDPC (AR4JA) forward error correction (CCSDS 131.0-B-5).
pub mod ldpc;
/// CCSDS Reed-Solomon (255,223) forward error correction (CCSDS 131.0-B-5).
pub mod reed_solomon;
/// Communications Link Transmission Unit (CLTU) encoding.
pub mod cltu;
/// CRC-protected Space Packet wrapper.
pub mod crc;
/// CCSDS pseudo-randomization for TC and TM frames.
pub mod randomizer;
/// CCSDS 121.0-B-3 Lossless Data Compression (Rice coding).
pub mod rice;
/// CCSDS 123.0-B-2 Low-Complexity Lossless Multispectral &
/// Hyperspectral Image Compression.
pub mod ccsds123;
/// CCSDS 122.0-B-2 Image Data Compression (wavelet-based).
pub mod ccsds122;
/// CCSDS convolutional code (rate 1/2, K=7) with Viterbi decoding
/// (CCSDS 131.0-B-5).
pub mod convolutional;
