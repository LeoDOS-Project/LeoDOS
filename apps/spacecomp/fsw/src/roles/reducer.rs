use heapless::index_map::FnvIndexMap;
use leodos_spacecomp::bufwriter::BufWriter;
use leodos_spacecomp::packet::AssignReducerPayload;
use leodos_spacecomp::packet::OpCode;
use leodos_spacecomp::packet::SpaceCompMessage;

use crate::data::WordCount;
use crate::Buffers;
use crate::RxHandle;
use crate::SpaceCompError;
use crate::TxHandle;

pub async fn run(
    rx: &mut RxHandle<'_>,
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    job_id: u16,
    assign: AssignReducerPayload,
) -> Result<(), SpaceCompError> {
    let mut counts: FnvIndexMap<[u8; 16], u32, 64> = FnvIndexMap::new();
    let mut done_count = 0u8;

    loop {
        let Ok(op) = rx
            .recv_with(|data| {
                let Ok(msg) = SpaceCompMessage::parse(data) else {
                    return None;
                };
                match msg.op_code() {
                    Ok(OpCode::DataChunk) => {
                        for wc in msg.records::<WordCount>() {
                            counts
                                .entry(wc.word)
                                .and_modify(|c| *c += wc.count.get())
                                .or_insert_with(|| wc.count.get())
                                .ok();
                        }
                        None
                    }
                    Ok(op) => Some(op),
                    _ => None,
                }
            })
            .await
        else {
            return Ok(());
        };
        if op == Some(OpCode::PhaseDone) {
            done_count += 1;
            if done_count >= assign.mapper_count() {
                let mut writer = BufWriter::<WordCount>::new(
                    tx,
                    &mut bufs.msg,
                    assign.los_addr(),
                    job_id,
                    OpCode::JobResult,
                );

                for (word, &count) in counts.iter() {
                    let wc = WordCount::builder().word(word).count(count).build();
                    writer.write(&wc).await?;
                }
                writer.flush().await?;
                return Ok(());
            }
        }
    }
}
