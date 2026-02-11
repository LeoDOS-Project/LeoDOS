use crate::datalink::DataLink;
use crate::network::NetworkLayer;

pub struct PassThrough<L> {
    link: L,
}

impl<L> PassThrough<L> {
    pub fn new(link: L) -> Self {
        Self { link }
    }

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
