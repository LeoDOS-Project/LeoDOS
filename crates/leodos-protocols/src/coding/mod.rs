mod cadu;
mod checksum;
pub mod cltu;
pub mod crc;
pub mod physical;
pub mod randomizer;

/// Interface for the Physical Layer (Radio/Serial).
pub trait PhysicalWriter {
    type Error;
    fn transmit(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}
