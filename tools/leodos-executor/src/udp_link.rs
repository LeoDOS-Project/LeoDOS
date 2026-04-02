use leodos_protocols::network::NetworkRead;
use leodos_protocols::network::NetworkWrite;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// SB header size (CCSDS primary + secondary command header).
const SB_HEADER_SIZE: usize = 8;

/// A UDP link that wraps payloads in cFS SB message headers.
///
/// ci_lab expects UDP packets to be complete cFS SB messages.
/// This link prepends an SB header (with the configured MsgId)
/// on write, and strips it on read, so the SRSPP layer above
/// sees only the payload.
pub struct UdpLink {
    socket: UdpSocket,
    remote: SocketAddr,
    /// StreamId for outgoing SB messages (router send topic).
    send_stream_id: u16,
    /// Sequence counter for outgoing messages.
    seq: u16,
}

impl UdpLink {
    pub async fn new(
        local_addr: &str,
        remote_addr: &str,
        router_send_topic: u16,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let socket = UdpSocket::bind(local_addr).await?;
        let remote: SocketAddr = remote_addr.parse()?;
        // cFS command MsgId: bit 12 set (command), bits 0-10 = topic
        let send_stream_id = 0x1800 | router_send_topic;
        Ok(Self {
            socket,
            remote,
            send_stream_id,
            seq: 0,
        })
    }
}

impl NetworkWrite for UdpLink {
    type Error = std::io::Error;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let total = SB_HEADER_SIZE + data.len();
        let mut buf = vec![0u8; total];

        // CCSDS primary header (6 bytes)
        let stream_id = self.send_stream_id;
        let seq = 0xC000 | (self.seq & 0x3FFF); // standalone packet
        let length = (total - 7) as u16; // CCSDS length = total - 7

        buf[0] = (stream_id >> 8) as u8;
        buf[1] = stream_id as u8;
        buf[2] = (seq >> 8) as u8;
        buf[3] = seq as u8;
        buf[4] = (length >> 8) as u8;
        buf[5] = length as u8;

        // Command secondary header (2 bytes): function code + checksum
        buf[6] = 0;
        buf[7] = 0;

        buf[SB_HEADER_SIZE..].copy_from_slice(data);

        self.seq = self.seq.wrapping_add(1);
        self.socket.send_to(&buf, self.remote).await?;
        Ok(())
    }
}

impl NetworkRead for UdpLink {
    type Error = std::io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut raw = vec![0u8; SB_HEADER_SIZE + buf.len()];
        let (len, _from) = self.socket.recv_from(&mut raw).await?;
        let payload_len = len.saturating_sub(SB_HEADER_SIZE);
        buf[..payload_len].copy_from_slice(&raw[SB_HEADER_SIZE..len]);
        Ok(payload_len)
    }
}
