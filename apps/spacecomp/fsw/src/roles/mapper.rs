use leodos_spacecomp::bufwriter::BufWriter;
use leodos_spacecomp::packet::{
    AssignMapperPayload, OpCode, SpaceCompMessage,
};

use crate::data::WordCount;
use crate::Buffers;
use crate::RxHandle;
use crate::SpaceCompError;
use crate::TxHandle;

const MAX_CHUNK: usize = 256;

pub async fn run(
    rx: &mut RxHandle<'_>,
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    job_id: u16,
    assign: AssignMapperPayload,
) -> Result<(), SpaceCompError> {
    let mut received = 0u8;
    {
        let mut writer = BufWriter::<WordCount>::new(
            tx,
            &mut bufs.msg,
            assign.reducer_addr(),
            job_id,
            OpCode::DataChunk,
        );

        loop {
            let mut payload = [0u8; MAX_CHUNK];
            let Ok(maybe_len) = rx
                .recv_with(|data| -> Option<usize> {
                    let msg = SpaceCompMessage::parse(data).ok()?;
                    if msg.op_code() != Ok(OpCode::DataChunk) {
                        return None;
                    }
                    let n = msg.payload().len().min(MAX_CHUNK);
                    payload[..n].copy_from_slice(&msg.payload()[..n]);
                    Some(n)
                })
                .await
            else {
                return Ok(());
            };
            let Some(len) = maybe_len else {
                continue;
            };

            for word_bytes in payload[..len].split(|&b| b == b' ' || b == b'\n' || b == b'\t') {
                if word_bytes.is_empty() || word_bytes.len() > 16 {
                    continue;
                }
                let wc = WordCount::builder().word(word_bytes).count(1).build();
                writer.write(&wc).await?;
            }
            writer.flush().await?;

            received += 1;
            if received >= assign.collector_count() {
                break;
            }
        }
    }

    let done = SpaceCompMessage::builder()
        .buffer(&mut bufs.msg)
        .op_code(OpCode::PhaseDone)
        .job_id(job_id)
        .payload_len(0)
        .build()?;
    tx.send(assign.reducer_addr(), done).await.ok();
    Ok(())
}
