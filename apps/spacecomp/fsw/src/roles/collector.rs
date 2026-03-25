use leodos_protocols::application::spacecomp::packet::{
    AssignCollectorPayload, OpCode, SpaceCompMessage,
};

use crate::data;
use crate::Buffers;
use crate::TxHandle;
use crate::SpaceCompError;

const MAX_CHUNK: usize = 256;

pub async fn run(
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    job_id: u16,
    assign: AssignCollectorPayload,
) -> Result<(), SpaceCompError> {
    let partition = data::partition_text(assign.partition_id(), crate::NUM_SATS);

    for chunk in partition.chunks(MAX_CHUNK) {
        let m = SpaceCompMessage::builder()
            .buffer(&mut bufs.msg)
            .op_code(OpCode::DataChunk)
            .job_id(job_id)
            .payload_len(chunk.len())
            .build()?;
        m.payload_mut().copy_from_slice(chunk);
        tx.send(assign.mapper_addr(), m).await.ok();
    }

    Ok(())
}
