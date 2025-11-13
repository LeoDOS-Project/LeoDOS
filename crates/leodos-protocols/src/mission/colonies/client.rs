use zerocopy::IntoBytes;

use crate::datalink::DataLink;
use crate::mission::colonies::messages::*;
use crate::network::spp::{Apid, SequenceCount};

#[derive(Debug)]
pub enum ClientError<E> {
    Transport(E),
    Packet(ColoniesMessageError),
    BufferTooSmall,
    PayloadFormat,
}

pub struct ColoniesClient<T> {
    pub transport: T,
    pub apid: Apid,
    pub seq_count: SequenceCount,
}

impl<T> ColoniesClient<T>
where
    T: DataLink,
{
    pub fn new(transport: T, apid: u16) -> Self {
        Self {
            transport,
            apid: Apid::new(apid).expect("Invalid APID"),
            seq_count: SequenceCount::new(),
        }
    }

    pub async fn assign(
        &mut self,
        buffer: &mut [u8],
        msg_id: u32,
        executor_prv_key: &str,
    ) -> Result<usize, ClientError<T::Error>> {
        self.rpc(buffer, ColoniesOpCode::AssignRequest, msg_id, |writer| {
            writer.write_str(executor_prv_key)
        })
        .await
    }

    pub async fn rpc<F>(
        &mut self,
        buffer: &mut [u8],
        op_code: ColoniesOpCode,
        msg_id: u32,
        payload_fn: F,
    ) -> Result<usize, ClientError<T::Error>>
    where
        F: FnOnce(&mut PayloadWriter) -> Result<(), ()>,
    {
        self.send(buffer, op_code, msg_id, payload_fn).await?;
        self.receive(buffer).await
    }

    /// Generic method to construct and send a packet with a closure-writer.
    pub async fn send<F>(
        &mut self,
        buffer: &mut [u8],
        op_code: ColoniesOpCode,
        msg_id: u32,
        payload_fn: F,
    ) -> Result<(), ClientError<T::Error>>
    where
        F: FnOnce(&mut PayloadWriter) -> Result<(), ()>,
    {
        let header_len = 16;
        if buffer.len() < header_len {
            return Err(ClientError::BufferTooSmall);
        }

        let mut writer = PayloadWriter::new(&mut buffer[header_len..]);
        payload_fn(&mut writer).map_err(|_| ClientError::BufferTooSmall)?;
        let payload_len = writer.len();

        let packet = ColoniesPacket::builder()
            .buffer(buffer)
            .apid(self.apid)
            .sequence_count(self.seq_count)
            .op_code(op_code)
            .msg_id(msg_id)
            .payload_len(payload_len)
            .build()
            .map_err(ClientError::Packet)?;

        packet.set_cfe_checksum();
        self.seq_count.increment();

        self.transport
            .send(packet.as_bytes())
            .await
            .map_err(ClientError::Transport)
    }

    /// Receives and validates a packet. Returns valid length on success.
    pub async fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, ClientError<T::Error>> {
        let len = self
            .transport
            .recv(buffer)
            .await
            .map_err(ClientError::Transport)?;

        // Basic validation
        let packet = ColoniesPacket::parse(&buffer[..len]).map_err(ClientError::Packet)?;

        if packet.primary.apid() != self.apid {
            // It's a valid packet but not for us.
            // We treat this as a PacketError::Parse to signal "ignore this" to the caller.
            return Err(ClientError::Packet(ColoniesMessageError::Parse));
        }

        Ok(len)
    }
}
