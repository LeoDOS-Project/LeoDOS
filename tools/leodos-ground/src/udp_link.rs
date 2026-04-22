use leodos_protocols::network::NetworkRead;
use leodos_protocols::network::NetworkWrite;
use leodos_protocols::transport::srspp::packet::SrsppPacket;
use leodos_protocols::transport::srspp::packet::SrsppType;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

/// Bound UDP socket plus a dispatcher task that splits incoming
/// packets by SRSPP type.
///
/// Tokio `SrsppSender` drops any non-ACK packet it reads; the
/// tokio `SrsppReceiver` similarly ignores ACKs. If both share a
/// socket without coordination, each one steals the other's
/// packets. This wrapper runs a background task that reads from
/// the socket once and forwards ACK bytes to one channel and DATA
/// bytes to another. Each SRSPP end then gets its own reader that
/// pulls from the right channel, while sharing the underlying
/// socket for outbound writes.
pub struct GroundSocket {
    socket: Arc<UdpSocket>,
    remote: SocketAddr,
}

impl GroundSocket {
    pub async fn bind(local: SocketAddr, remote: SocketAddr) -> std::io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind(local).await?);
        Ok(Self { socket, remote })
    }

    /// Returns (sender-link, receiver-link, dispatcher-task).
    /// The caller must drive the dispatcher (e.g. tokio::spawn).
    pub fn split(
        self,
    ) -> (
        ChannelLink,
        ChannelLink,
        impl std::future::Future<Output = ()> + Send + 'static,
    ) {
        let (ack_tx, ack_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (data_tx, data_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        let socket = self.socket.clone();
        let dispatcher = async move {
            let mut buf = vec![0u8; 2048];
            loop {
                let (len, _from) = match socket.recv_from(&mut buf).await {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let packet = &buf[..len];
                let stype = SrsppPacket::parse(packet).and_then(|p| p.srspp_type());
                let owned = packet.to_vec();
                match stype {
                    Ok(SrsppType::Ack) => {
                        if ack_tx.send(owned).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {
                        if data_tx.send(owned).is_err() {
                            break;
                        }
                    }
                    Err(_) => {}
                }
            }
        };

        let sender_link = ChannelLink {
            socket: self.socket.clone(),
            remote: self.remote,
            rx: ack_rx,
        };
        let receiver_link = ChannelLink {
            socket: self.socket,
            remote: self.remote,
            rx: data_rx,
        };
        (sender_link, receiver_link, dispatcher)
    }
}

/// A link backed by a shared UDP socket for writes and an mpsc
/// channel for reads. The channel is fed by the dispatcher task
/// that routes SRSPP ACK vs DATA packets.
pub struct ChannelLink {
    socket: Arc<UdpSocket>,
    remote: SocketAddr,
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl NetworkWrite for ChannelLink {
    type Error = std::io::Error;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.socket.send_to(data, self.remote).await?;
        Ok(())
    }
}

impl NetworkRead for ChannelLink {
    type Error = std::io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let pkt = self
            .rx
            .recv()
            .await
            .ok_or_else(|| std::io::Error::other("dispatcher closed"))?;
        let len = pkt.len().min(buf.len());
        buf[..len].copy_from_slice(&pkt[..len]);
        Ok(len)
    }
}
