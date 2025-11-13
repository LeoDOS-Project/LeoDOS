use zerocopy::IntoBytes as _;

use crate::datalink::DataLink;
use crate::mission::colonies::client::{ClientError, ColoniesClient};
use crate::mission::colonies::messages::*;
use core::future::Future;
use core::str;

pub trait ColoniesHandler {
    fn handle(
        &mut self,
        func_name: &str,
        args: ArgIterator<'_>,
        writer: &mut PayloadWriter<'_>,
    ) -> impl Future<Output = Result<(), &'static str>>;
}

pub struct ColoniesExecutor<L2, H> {
    client: ColoniesClient<L2>,
    handler: H,
    executor_prv_key: heapless::String<64>,
    msg_id: u32,
}

impl<L2, H> ColoniesExecutor<L2, H>
where
    L2: DataLink,
    H: ColoniesHandler,
{
    pub fn new(link: L2, handler: H, apid: u16, executor_prv_key: &str) -> Self {
        Self {
            client: ColoniesClient::new(link, apid),
            handler,
            executor_prv_key: heapless::String::try_from(executor_prv_key).unwrap_or_default(),
            msg_id: 0,
        }
    }

    pub async fn assign(&mut self, buffer: &mut [u8]) -> Result<(), ClientError<L2::Error>> {
        self.client
            .assign(buffer, self.msg_id, &self.executor_prv_key)
            .await
            .map(|_| ())
    }

    pub async fn run(&mut self, buffer: &mut [u8]) -> ! {
        loop {
            self.msg_id = self.msg_id.wrapping_add(1);
            let Ok(len) = self
                .client
                .assign(buffer, self.msg_id, &self.executor_prv_key)
                .await
            else {
                continue;
            };

            let (rx_slice, tx_slice) = buffer.split_at_mut(len);
            let _ = self.process_packet(rx_slice, tx_slice).await;
        }
    }

    async fn process_packet(
        &mut self,
        rx_data: &[u8],
        tx_buffer: &mut [u8],
    ) -> Result<(), ClientError<L2::Error>> {
        let packet = ColoniesPacket::parse(rx_data).map_err(ClientError::Packet)?;

        // Correlation Check
        if packet.colonies.op_code() != Ok(ColoniesOpCode::AssignResponse) {
            return Ok(());
        }
        if packet.colonies.msg_id() != self.msg_id {
            return Ok(());
        }

        // --- Parsing ---
        let mut iter = ArgIterator::new(&packet.payload);

        let process_id_bytes = iter.next().ok_or(ClientError::PayloadFormat)?;

        // Capture ProcessID on stack
        let mut pid_storage = [0u8; 64];
        let pid_len = process_id_bytes.len().min(64);
        pid_storage[..pid_len].copy_from_slice(&process_id_bytes[..pid_len]);
        let process_id = &pid_storage[..pid_len];

        let func_name_bytes = iter.next().ok_or(ClientError::PayloadFormat)?;
        let func_name = str::from_utf8(func_name_bytes).map_err(|_| ClientError::PayloadFormat)?;

        // --- Execution ---
        // Manually write output to TX buffer so we can await the async handler
        let header_len = 16;
        if tx_buffer.len() < header_len {
            return Err(ClientError::BufferTooSmall);
        }

        let mut writer = PayloadWriter::new(&mut tx_buffer[header_len..]);
        writer
            .write_bytes(process_id)
            .map_err(|_| ClientError::BufferTooSmall)?;

        let exec_result = self.handler.handle(func_name, iter, &mut writer).await;

        let (op_code, final_len) = match exec_result {
            Ok(_) => (ColoniesOpCode::Close, writer.len()),
            Err(err_msg) => {
                let mut err_writer = PayloadWriter::new(&mut tx_buffer[header_len..]);
                let _ = err_writer.write_bytes(process_id);
                let _ = err_writer.write_str(err_msg);
                (ColoniesOpCode::Fail, err_writer.len())
            }
        };

        // Manual packet build since we already wrote the payload
        let packet = ColoniesPacket::builder()
            .buffer(tx_buffer)
            .apid(self.client.apid)
            .sequence_count(self.client.seq_count)
            .op_code(op_code)
            .msg_id(self.msg_id)
            .payload_len(final_len)
            .build()
            .map_err(ClientError::Packet)?;

        packet.set_cfe_checksum();
        self.client.seq_count.increment();

        self.client
            .transport
            .send(packet.as_bytes())
            .await
            .map_err(ClientError::Transport)
    }
}
