use crate::datalink::DataLink;
use crate::network::NetworkLayer;

/// A network layer that forwards directly to a datalink with no framing.
pub struct PassThrough<L> {
    link: L,
}

impl<L> PassThrough<L> {
    /// Wraps a datalink in a passthrough network layer.
    pub fn new(link: L) -> Self {
        Self { link }
    }

    /// Consumes the wrapper and returns the inner datalink.
    pub fn into_inner(self) -> L {
        self.link
    }
}

impl<L: DataLink> NetworkLayer for PassThrough<L> {
    type Error = L::Error;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.link.send(data).await
    }

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.link.recv(buffer).await
    }
}
