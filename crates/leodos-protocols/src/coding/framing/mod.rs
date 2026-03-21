//! Frame synchronization and transmission unit encoding.

/// Channel Access Data Unit — ASM framing and frame sync
/// (CCSDS 131.0-B-5).
pub mod cadu;
/// Communications Link Transmission Unit (CLTU) encoding.
pub mod cltu;

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
