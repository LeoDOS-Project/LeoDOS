//! Forward error-correction codes.

/// CCSDS Reed-Solomon (255,223) (CCSDS 131.0-B-5).
pub mod reed_solomon;
/// CCSDS LDPC (AR4JA) codes (CCSDS 131.0-B-5).
pub mod ldpc;
/// Convolutional code (rate 1/2, K=7) with Viterbi decoding
/// (CCSDS 131.0-B-5).
pub mod convolutional;

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
