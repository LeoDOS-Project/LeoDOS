use crate::datalink::{DatalinkRead, DatalinkWrite};

/// A data link composed of separate sender and receiver halves.
pub struct AsymmetricLink<S, R> {
    sender: S,
    receiver: R,
}

impl<S, R> AsymmetricLink<S, R> {
    /// Creates a new asymmetric link from separate sender and receiver.
    pub fn new(sender: S, receiver: R) -> Self {
        Self { sender, receiver }
    }
}

/// Errors from an asymmetric link, wrapping send or receive errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AsymmetricLinkError<SE, RE> {
    /// An error occurred during send.
    #[error("send error: {0}")]
    Send(SE),
    /// An error occurred during receive.
    #[error("recv error: {0}")]
    Recv(RE),
}

impl<S, R> DatalinkWrite for AsymmetricLink<S, R>
where
    S: DatalinkWrite,
    R: DatalinkRead,
{
    type Error = AsymmetricLinkError<S::Error, R::Error>;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.sender
            .write(data)
            .await
            .map_err(AsymmetricLinkError::Send)
    }
}

impl<S, R> DatalinkRead for AsymmetricLink<S, R>
where
    S: DatalinkWrite,
    R: DatalinkRead,
{
    type Error = AsymmetricLinkError<S::Error, R::Error>;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.receiver
            .read(buffer)
            .await
            .map_err(AsymmetricLinkError::Recv)
    }
}
