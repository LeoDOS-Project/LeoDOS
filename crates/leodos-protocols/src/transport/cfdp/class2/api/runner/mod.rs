use core::future::Future;
use core::net::SocketAddr;

pub mod receiver_runner;
pub mod sender_runner;

pub trait UdpSocket {
    type Error;

    fn recv_from<'a>(
        &'a self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<(usize, SocketAddr), Self::Error>> + 'a;

    fn send_to<'a>(
        &'a self,
        buf: &'a [u8],
        target: SocketAddr,
    ) -> impl Future<Output = Result<usize, Self::Error>> + 'a;
}
