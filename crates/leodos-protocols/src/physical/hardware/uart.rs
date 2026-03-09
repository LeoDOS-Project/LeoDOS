use leodos_libcfs::nos3::uart::Uart;
use leodos_libcfs::nos3::UartError;

use crate::physical::{PhysicalReader, PhysicalWriter};

/// Physical channel backed by a hwlib UART port.
///
/// In NOS3 simulation, the UART is routed through NOS Engine
/// to the radio simulator. On real hardware, the same code
/// talks to the actual radio over a serial bus.
pub struct UartChannel {
    uart: Uart,
}

impl UartChannel {
    /// Wraps an already-opened UART port as a physical channel.
    pub fn new(uart: Uart) -> Self {
        Self { uart }
    }

    /// Returns a reference to the inner UART.
    pub fn uart(&self) -> &Uart {
        &self.uart
    }

    /// Returns a mutable reference to the inner UART.
    pub fn uart_mut(&mut self) -> &mut Uart {
        &mut self.uart
    }

    /// Consumes this channel, returning the inner UART.
    pub fn into_inner(self) -> Uart {
        self.uart
    }
}

impl PhysicalWriter for UartChannel {
    type Error = UartError;

    async fn write(
        &mut self,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        let mut offset = 0;
        while offset < data.len() {
            let n = self.uart.write(&data[offset..])?;
            offset += n;
        }
        Ok(())
    }
}

impl PhysicalReader for UartChannel {
    type Error = UartError;

    async fn read(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<usize, Self::Error> {
        self.uart.read(buffer)
    }
}
