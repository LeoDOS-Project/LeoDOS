use leodos_protocols::application::spacecomp::packet::{
    AssignCollectorMessage, OpCode, SpaceCompMessage,
};

use crate::data;
use crate::Buffers;
use crate::NodeHandle;
use crate::SpaceCompError;

const MAX_CHUNK: usize = 256;

pub async fn run(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    assign: AssignCollectorMessage,
) -> Result<(), SpaceCompError> {
    let partition = data::partition_text(assign.partition_id, crate::NUM_SATS);

    for chunk in partition.chunks(MAX_CHUNK) {
        let msg = SpaceCompMessage::builder()
            .buffer(&mut bufs.msg)
            .op_code(OpCode::DataChunk)
            .job_id(assign.job_id)
            .payload(chunk)
            .build()?;
        handle.send(assign.mapper_addr, msg).await.ok();
    }

    Ok(())
}
