use leodos_protocols::mission::spacecomp::packet::{OpCode, SpaceCompMessage};
use leodos_protocols::network::isl::address::Address;

use crate::data;
use crate::NodeHandle;

const MAX_CHUNK: usize = 256;
const MSG_BUF_SIZE: usize = 512;

pub async fn run(
    handle: &mut NodeHandle<'_>,
    mapper_addr: Address,
    partition_id: u8,
    job_id: u16,
) {
    let total_partitions = crate::NUM_SATS;
    let (chunk, chunk_len) = data::partition_text(partition_id, total_partitions);

    if chunk_len == 0 {
        return;
    }

    let mut msg_buf = [0u8; MSG_BUF_SIZE];
    let mut offset = 0;
    while offset < chunk_len {
        let end = (offset + MAX_CHUNK).min(chunk_len);
        if let Some(msg) = SpaceCompMessage::builder()
            .buffer(&mut msg_buf)
            .op_code(OpCode::DataChunk)
            .job_id(job_id)
            .payload(&chunk[offset..end])
            .build()
        {
            handle.send(mapper_addr, msg.as_bytes()).await.ok();
        }
        offset = end;
    }
}
