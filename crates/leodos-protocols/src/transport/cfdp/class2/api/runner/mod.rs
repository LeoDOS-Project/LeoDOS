use core::future::Future;
use core::net::SocketAddr;

pub mod receiver_runner;
pub mod sender_runner;

/// Async UDP socket abstraction for platform-independent networking.
pub trait UdpSocket {
    /// The error type returned by socket operations.
    type Error;

    /// Receives a datagram and returns the number of bytes read and the source address.
    fn recv_from<'a>(
        &'a self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<(usize, SocketAddr), Self::Error>> + 'a;

    /// Sends a datagram to the specified address and returns the number of bytes sent.
    fn send_to<'a>(
        &'a self,
        buf: &'a [u8],
        target: SocketAddr,
    ) -> impl Future<Output = Result<usize, Self::Error>> + 'a;
}
