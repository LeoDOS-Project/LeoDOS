use leodos_libcfs::cfe::evs::event;
use leodos_protocols::mission::compute::packet::{AssignCollectorPayload, OpCode};
use leodos_protocols::network::NetworkLayer;

use crate::isl;

pub async fn send_data<L: NetworkLayer>(
    link: &mut L,
    ctx: &isl::Context,
    payload: &AssignCollectorPayload,
    job_id: u16,
) {
    let mapper_addr = payload.mapper_addr.parse();

    event::info(0, "Collector: sending data").ok();

    let data = [0u8; 64];
    isl::send(link, ctx, mapper_addr, OpCode::DataChunk, job_id, &data)
        .await
        .ok();

    event::info(0, "Collector: done").ok();
}
