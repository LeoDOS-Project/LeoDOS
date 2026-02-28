use heapless::index_map::FnvIndexMap;
use leodos_protocols::application::spacecomp::io::writer::BufWriter;
use leodos_protocols::application::spacecomp::packet::{
    AssignReducerMessage, OpCode, SpaceCompMessage,
};

use crate::data::WordCount;
use crate::Buffers;
use crate::RxHandle;
use crate::SpaceCompError;
use crate::TxHandle;

pub async fn run(
    rx: &mut RxHandle<'_>,
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    assign: AssignReducerMessage,
) -> Result<(), SpaceCompError> {
    let mut counts: FnvIndexMap<[u8; 16], u32, 64> = FnvIndexMap::new();
    let mut done_count = 0u8;

    loop {
        let Ok((_, len)) = rx.recv(&mut bufs.recv).await else {
            return Ok(());
        };
        let Ok(msg) = SpaceCompMessage::parse(&bufs.recv[..len]) else {
            continue;
        };
        let Ok(op_code) = msg.op_code() else {
            continue;
        };
        match op_code {
            OpCode::DataChunk => {
                for wc in msg.records::<WordCount>() {
                    counts
                        .entry(wc.word)
                        .and_modify(|c| *c += wc.count.get())
                        .or_insert_with(|| wc.count.get())
                        .ok();
                }
            }
            OpCode::PhaseDone => {
                done_count += 1;
                if done_count >= assign.mapper_count {
                    let mut writer = BufWriter::<WordCount, _>::new(
                        tx,
                        &mut bufs.msg,
                        assign.los_addr,
                        assign.job_id,
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
            _ => {}
        }
    }
}
