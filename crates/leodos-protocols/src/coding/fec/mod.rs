//! Forward error-correction codes.

/// CCSDS Reed-Solomon (255,223) (CCSDS 131.0-B-5).
pub mod reed_solomon;
/// CCSDS LDPC (AR4JA) codes (CCSDS 131.0-B-5).
pub mod ldpc;
/// Convolutional code (rate 1/2, K=7) with Viterbi decoding
/// (CCSDS 131.0-B-5).
pub mod convolutional;
