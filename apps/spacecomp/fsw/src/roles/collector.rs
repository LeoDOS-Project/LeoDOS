use leodos_protocols::mission::compute::packet::OpCode;
use leodos_protocols::network::isl::address::Address;

use crate::data;
use crate::isl::{self, NodeHandle};

const MAX_CHUNK: usize = 256;

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

    let mut offset = 0;
    while offset < chunk_len {
        let end = (offset + MAX_CHUNK).min(chunk_len);
        isl::send(handle, mapper_addr, OpCode::DataChunk, job_id, &chunk[offset..end])
            .await
            .ok();
        offset = end;
    }
}
