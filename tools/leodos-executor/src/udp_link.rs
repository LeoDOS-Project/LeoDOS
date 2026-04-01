use leodos_protocols::network::NetworkRead;
use leodos_protocols::network::NetworkWrite;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// A UDP-based network link for SRSPP.
///
/// Sends packets to ci_lab (command uplink) and receives
/// from to_lab (telemetry downlink) via the cFS Software Bus.
pub struct UdpLink {
    socket: UdpSocket,
    remote: SocketAddr,
}

impl UdpLink {
    pub async fn new(
        local_addr: &str,
        remote_addr: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let socket = UdpSocket::bind(local_addr).await?;
        let remote: SocketAddr = remote_addr.parse()?;
        Ok(Self { socket, remote })
    }
}

impl NetworkWrite for UdpLink {
    type Error = std::io::Error;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.socket.send_to(data, self.remote).await?;
        Ok(())
    }
}

impl NetworkRead for UdpLink {
    type Error = std::io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let (len, _from) = self.socket.recv_from(buf).await?;
        Ok(len)
    }
}
