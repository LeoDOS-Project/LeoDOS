use leodos_libcfs::os::net::UdpSocket;
use leodos_protocols::datalink::FrameSink;
use leodos_protocols::network::spp::SpacePacket;
use zerocopy::IntoBytes;

pub struct UdpLink<'a> {
    socket: &'a UdpSocket,
    target: leodos_libcfs::os::net::SocketAddr,
}

impl<'a> UdpLink<'a> {
    pub fn new(socket: &'a UdpSocket, target: leodos_libcfs::os::net::SocketAddr) -> Self {
        UdpLink { socket, target }
    }
}

impl<'a> FrameSink for UdpLink<'a> {
    type Error = leodos_libcfs::error::Error;

    async fn write(&mut self, packet: &SpacePacket) -> Result<(), Self::Error> {
        self.socket
            .send_to(packet.as_bytes(), &self.target)
            .map(|_| ())
    }
}
