pub mod randomizer;
pub mod crc;
pub mod cltu;
pub mod checksum;
pub mod cadu;

/// Interface for the Physical Layer (Radio/Serial).
pub trait PhysicalWriter {
    type Error;
    fn transmit(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}
