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
/// Async physical channel traits and CLTU writer.
pub mod physical;
/// CCSDS pseudo-randomization for TC and TM frames.
pub mod randomizer;

/// Synchronous trait for transmitting raw bytes on the physical layer.
pub trait PhysicalWriter {
    /// Error type for transmit operations.
    type Error;
    /// Transmits the given data bytes.
    fn transmit(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}
