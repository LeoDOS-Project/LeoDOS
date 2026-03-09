use leodos_libcfs::nos3::I2cError;
use leodos_libcfs::nos3::i2c::I2cBus;

use crate::physical::{PhysicalReader, PhysicalWriter};

/// An [`I2cBus`] bound to a fixed slave address and timeout.
///
/// I2C is a multi-device bus where each transaction specifies
/// a slave address. This wrapper fixes the address and timeout
/// so the bus can be used as a byte-stream physical channel to
/// a single peripheral.
pub struct I2cChannel {
    bus: I2cBus,
    addr: u8,
    timeout: u8,
}

impl I2cChannel {
    /// Binds an I2C bus to a specific slave address.
    pub fn new(bus: I2cBus, addr: u8, timeout: u8) -> Self {
        Self { bus, addr, timeout }
    }

    /// Returns a reference to the inner bus.
    pub fn bus(&self) -> &I2cBus {
        &self.bus
    }

    /// Returns a mutable reference to the inner bus.
    pub fn bus_mut(&mut self) -> &mut I2cBus {
        &mut self.bus
    }

    /// Consumes this channel, returning the inner bus.
    pub fn into_inner(self) -> I2cBus {
        self.bus
    }
}

impl PhysicalWriter for I2cChannel {
    type Error = I2cError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.bus.write(self.addr, data, self.timeout)
    }
}

impl PhysicalReader for I2cChannel {
    type Error = I2cError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.bus.read(self.addr, buffer, self.timeout)?;
        Ok(buffer.len())
    }
}
