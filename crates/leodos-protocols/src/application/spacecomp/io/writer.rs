use core::future::Future;

use crate::network::isl::address::Address;

/// Addressed message sender for SpaceCoMP communication.
pub trait MessageSender {
    /// Error type returned by send operations.
    type Error;

    /// Sends a raw message to the given target address.
    fn send_message(
        &mut self,
        target: Address,
        data: &[u8],
    ) -> impl Future<Output = Result<(), Self::Error>>;
}
