use crate::datalink::{DataLinkReader, DataLinkWriter};
use crate::network::{NetworkReader, NetworkWriter};

/// A point-to-point network layer that forwards directly to a datalink.
pub struct PointToPoint<L> {
    link: L,
}

impl<L> PointToPoint<L> {
    /// Wraps a datalink in a point-to-point network layer.
    pub fn new(link: L) -> Self {
        Self { link }
    }

    /// Consumes the wrapper and returns the inner datalink.
    pub fn into_inner(self) -> L {
        self.link
    }
}

impl<L: DataLinkWriter> NetworkWriter for PointToPoint<L> {
    type Error = L::Error;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        DataLinkWriter::send(&mut self.link, data).await
    }
}

impl<L: DataLinkReader> NetworkReader for PointToPoint<L> {
    type Error = L::Error;

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        DataLinkReader::recv(&mut self.link, buffer).await
    }
}
