use core::future::Future;

/// A trait for sending application-level messages.
///
/// Implementations of this trait handle the wrapping of data into
/// Space Packets (L3) and Transfer Frames (L2).
pub trait PacketTransport {
    /// Error type returned by send operations.
    type Error;

    /// Sends a payload to the configured destination.
    ///
    /// The implementation is responsible for:
    /// 1. Adding the Space Packet Primary Header (APID, Seq Count).
    /// 2. Adding any Secondary Headers (e.g., CFE Function Code).
    /// 3. Passing the packet to the Datalink layer.
    fn send(&mut self, function_code: u8, payload: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}
