use leodos_libcfs::nos3::SpiError;
use leodos_libcfs::nos3::buses::spi::Spi;

use crate::physical::{PhysicalRead, PhysicalWrite};

impl PhysicalWrite for Spi {
    type Error = SpiError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        Spi::write(self, data)
    }
}

impl PhysicalRead for Spi {
    type Error = SpiError;

    /// SPI always transfers the exact number of bytes requested.
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        Spi::read(self, buffer)?;
        Ok(buffer.len())
    }
}
