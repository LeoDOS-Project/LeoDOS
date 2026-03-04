//! Tokio-based async driver for srspp.
//!
//! Provides separate sender and receiver types for point-to-point communication.
//!
//! ## Sender Example
//!
//! ```ignore
//! let sender = SrsppSender::new(config, link);
//!
//! // Send messages
//! sender.send(&data).await?;
//! sender.send(&more_data).await?;
//!
//! // Wait for all to be acknowledged
//! sender.flush().await?;
//! ```
//!
//! ## Receiver Example
//!
//! ```ignore
//! let mut receiver = SrsppReceiver::new(config, link);
//!
//! // Receive messages
//! while let Some(message) = receiver.recv().await? {
//!     process(message);
//! }
//! ```

/// Async SRSPP receiver.
mod receiver;
/// Async SRSPP sender.
mod sender;
#[cfg(test)]
mod tests;

pub use receiver::{DeliveryToken, SrsppReceiver};
pub use sender::SrsppSender;

use crate::transport::srspp::machine::receiver::ReceiverError;
use crate::transport::srspp::machine::sender::SenderError;

use tokio::time::Duration;
use tokio::time::Instant;

/// Error type for srspp operations.
#[derive(Debug, thiserror::Error)]
pub enum SrsppError {
    /// Send buffer is full.
    #[error("send buffer full")]
    BufferFull,
    /// Window is full (too many unacked packets).
    #[error("window full")]
    WindowFull,
    /// Link error.
    #[error("link error: {0}")]
    LinkError(String),
    /// Packet error.
    #[error("packet error: {0}")]
    PacketError(String),
    /// Sender error.
    #[error(transparent)]
    SenderError(#[from] SenderError),
    /// Receiver error.
    #[error(transparent)]
    ReceiverError(#[from] ReceiverError),
}

/// Converts a tick count to a `Duration` given the tick rate.
fn ticks_to_duration(ticks: u32, ticks_per_sec: u32) -> Duration {
    Duration::from_millis((ticks as u64 * 1000) / ticks_per_sec as u64)
}

/// Sleeps until the given deadline, or forever if `None`.
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(d) => tokio::time::sleep_until(d.into()).await,
        None => std::future::pending().await,
    }
}
