use leodos_libcfs::nos3::SpiError;
use leodos_libcfs::nos3::spi::Spi;

use crate::physical::{PhysicalReader, PhysicalWriter};

impl PhysicalWriter for Spi {
    type Error = SpiError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        Spi::write(self, data)
    }
}

impl PhysicalReader for Spi {
    type Error = SpiError;

    /// SPI always transfers the exact number of bytes requested.
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        Spi::read(self, buffer)?;
        Ok(buffer.len())
    }
}
