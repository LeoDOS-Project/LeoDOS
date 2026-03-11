use leodos_libcfs::nos3::UartError;
use leodos_libcfs::nos3::buses::uart::Uart;

use crate::physical::{PhysicalRead, PhysicalWrite};

impl PhysicalWrite for Uart {
    type Error = UartError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let mut offset = 0;
        while offset < data.len() {
            let n = Uart::write(self, &data[offset..])?;
            offset += n;
        }
        Ok(())
    }
}

impl PhysicalRead for Uart {
    type Error = UartError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        Uart::read(self, buffer)
    }
}
