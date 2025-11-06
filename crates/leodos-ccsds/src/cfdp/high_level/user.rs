//! The CfdpUser callback trait.

use crate::cfdp::machine::receiver::ReceivedFileParams;
use crate::cfdp::machine::TransactionFinishedParams;

/// A trait the user implements to receive asynchronous notifications about CFDP events.
///
/// This is the primary callback mechanism for the high-level API.
pub trait CfdpUser: Send + Sync {
    /// Called when a transaction (sending or receiving) has finished, either
    /// successfully or with a fault.
    fn on_transaction_finished(
        &mut self,
        params: TransactionFinishedParams,
    ) -> impl core::future::Future<Output = ()> + Send;

    /// Called when a file has been successfully received and is available in the
    /// filestore.
    fn on_file_received(
        &mut self,
        params: ReceivedFileParams,
    ) -> impl core::future::Future<Output = ()> + Send;

    // Other indications from the spec, like suspend/resume, could be added here.
}

/// A no-op implementation of `CfdpUser` for users who don't need callbacks.
#[derive(Debug, Copy, Clone)]
pub struct NoopUser;

impl CfdpUser for NoopUser {
    async fn on_transaction_finished(&mut self, _params: TransactionFinishedParams) {}
    async fn on_file_received(&mut self, _params: ReceivedFileParams) {}
}
