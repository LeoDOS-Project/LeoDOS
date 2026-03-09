use leodos_libcfs::nos3::CanError;
use leodos_libcfs::nos3::buses::can::Can;

use crate::physical::{PhysicalReader, PhysicalWriter};

/// A [`Can`] bus bound to a fixed CAN ID for writing.
///
/// CAN is a message bus where each frame carries a CAN ID.
/// This wrapper fixes the transmit ID so the bus can be used
/// as a byte-stream physical channel. Received frames return
/// data regardless of their CAN ID.
pub struct CanChannel {
    can: Can,
    tx_id: u32,
}

impl CanChannel {
    /// Binds a CAN bus to a fixed transmit ID.
    pub fn new(can: Can, tx_id: u32) -> Self {
        Self { can, tx_id }
    }

    /// Returns a reference to the inner CAN device.
    pub fn can(&self) -> &Can {
        &self.can
    }

    /// Returns a mutable reference to the inner CAN device.
    pub fn can_mut(&mut self) -> &mut Can {
        &mut self.can
    }

    /// Consumes this channel, returning the inner CAN device.
    pub fn into_inner(self) -> Can {
        self.can
    }
}

impl PhysicalWriter for CanChannel {
    type Error = CanError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.can.write(self.tx_id, data)
    }
}

impl PhysicalReader for CanChannel {
    type Error = CanError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let (_id, len) = self.can.read(buffer)?;
        Ok(len)
    }
}
