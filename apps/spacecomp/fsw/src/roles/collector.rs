use leodos_libcfs::cfe::evs::event;
use leodos_protocols::mission::compute::packet::{AssignCollectorPayload, OpCode};
use leodos_protocols::network::NetworkLayer;

use crate::data;
use crate::isl;

pub async fn send_data<L: NetworkLayer>(
    link: &mut L,
    ctx: &isl::Context,
    payload: &AssignCollectorPayload,
    job_id: u16,
) {
    let mapper_addr = payload.mapper_addr.parse();
    let partition_id = payload.partition_id;

    event::info(0, "Collector: partitioning text").ok();

    let total_partitions = crate::NUM_SATS;
    let (chunk, chunk_len) = data::partition_text(partition_id, total_partitions);

    if chunk_len == 0 {
        event::info(0, "Collector: empty partition").ok();
        return;
    }

    let max_chunk = 256;
    let mut offset = 0;
    while offset < chunk_len {
        let end = (offset + max_chunk).min(chunk_len);
        let slice = &chunk[offset..end];
        isl::send(link, ctx, mapper_addr, OpCode::DataChunk, job_id, slice)
            .await
            .ok();
        offset = end;
    }

    event::info(0, "Collector: done").ok();
}
