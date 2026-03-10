use zerocopy::IntoBytes;

use crate::datalink::{DataLinkReader, DataLinkWriter};
use crate::application::colonies::messages::*;
use crate::network::spp::{Apid, SequenceCount};

/// Errors returned by the ColonyOS client.
#[derive(Debug)]
pub enum ClientError<E> {
    /// The transport layer reported an error.
    Transport(E),
    /// The packet could not be constructed or parsed.
    Packet(ColoniesMessageError),
    /// The provided buffer is too small for the operation.
    BufferTooSmall,
    /// The payload format is invalid (e.g., bad UTF-8 or missing fields).
    PayloadFormat,
}

/// A ColonyOS client that sends requests and receives responses over a data link.
pub struct ColoniesClient<T> {
    /// The underlying transport.
    pub transport: T,
    /// Application process identifier for packet routing.
    pub apid: Apid,
    /// Sequence counter for outgoing packets.
    pub seq_count: SequenceCount,
}

impl<T> ColoniesClient<T>
where
    T: DataLinkWriter + DataLinkReader<Error = <T as DataLinkWriter>::Error>,
{
    /// Creates a new client with the given transport and APID.
    pub fn new(transport: T, apid: u16) -> Self {
        Self {
            transport,
            apid: Apid::new(apid).expect("Invalid APID"),
            seq_count: SequenceCount::new(),
        }
    }

    /// Sends an assign request and returns the response length.
    pub async fn assign(
        &mut self,
        buffer: &mut [u8],
        msg_id: u32,
        executor_prv_key: &str,
    ) -> Result<usize, ClientError<<T as DataLinkWriter>::Error>> {
        self.rpc(buffer, ColoniesOpCode::AssignRequest, msg_id, |writer| {
            writer.write_str(executor_prv_key)
        })
        .await
    }

    /// Performs a request-response round trip with the given opcode and payload.
    pub async fn rpc<F>(
        &mut self,
        buffer: &mut [u8],
        op_code: ColoniesOpCode,
        msg_id: u32,
        payload_fn: F,
    ) -> Result<usize, ClientError<<T as DataLinkWriter>::Error>>
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
    ) -> Result<(), ClientError<<T as DataLinkWriter>::Error>>
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
            .write(packet.as_bytes())
            .await
            .map_err(ClientError::Transport)
    }

    /// Receives and validates a packet. Returns valid length on success.
    pub async fn receive(&mut self, buffer: &mut [u8]) -> Result<usize, ClientError<<T as DataLinkWriter>::Error>> {
        let len = self
            .transport
            .read(buffer)
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
