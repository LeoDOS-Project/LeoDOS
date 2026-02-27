use super::{FrameReceiver, FrameSender};
use crate::datalink::DataLink;

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
#[derive(Debug, Clone)]
pub enum AsymmetricLinkError<SE, RE> {
    /// An error occurred during send.
    Send(SE),
    /// An error occurred during receive.
    Recv(RE),
}

impl<SE: core::fmt::Display, RE: core::fmt::Display> core::fmt::Display
    for AsymmetricLinkError<SE, RE>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Send(e) => write!(f, "send error: {e}"),
            Self::Recv(e) => write!(f, "recv error: {e}"),
        }
    }
}

impl<SE: core::error::Error, RE: core::error::Error> core::error::Error
    for AsymmetricLinkError<SE, RE>
{
}

impl<S, R> DataLink for AsymmetricLink<S, R>
where
    S: FrameSender,
    R: FrameReceiver,
{
    type Error = AsymmetricLinkError<S::Error, R::Error>;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.sender.send(data).await.map_err(AsymmetricLinkError::Send)
    }

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.receiver
            .recv(buffer)
            .await
            .map_err(AsymmetricLinkError::Recv)
    }
}
