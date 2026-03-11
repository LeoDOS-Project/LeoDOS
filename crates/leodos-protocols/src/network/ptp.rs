use crate::datalink::{DatalinkRead, DatalinkWrite};
use crate::network::{NetworkRead, NetworkWrite};

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

impl<L: DatalinkWrite> NetworkWrite for PointToPoint<L> {
    type Error = L::Error;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        DatalinkWrite::write(&mut self.link, data).await
    }
}

impl<L: DatalinkRead> NetworkRead for PointToPoint<L> {
    type Error = L::Error;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        DatalinkRead::read(&mut self.link, buffer).await
    }
}
