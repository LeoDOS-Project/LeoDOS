use core::mem::size_of;
use heapless::index_map::FnvIndexMap;
use leodos_protocols::application::spacecomp::io::writer::BufWriter;
use leodos_protocols::application::spacecomp::packet::{
    AssignReducerMessage, OpCode, SpaceCompMessage,
};
use zerocopy::FromBytes;

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
        match msg.op_code() {
            Ok(OpCode::DataChunk) => ingest(&mut counts, msg.payload()),
            Ok(OpCode::PhaseDone) => {
                done_count += 1;
                if done_count >= assign.mapper_count {
                    emit(tx, bufs, &counts, assign).await?;
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}

fn ingest(counts: &mut FnvIndexMap<[u8; 16], u32, 64>, chunk: &[u8]) {
    let mut offset = 0;
    while offset + size_of::<WordCount>() <= chunk.len() {
        if let Ok(wc) = WordCount::read_from_bytes(&chunk[offset..offset + size_of::<WordCount>()]) {
            if let Some(c) = counts.get_mut(&wc.word) {
                *c += wc.count.get();
            } else {
                counts.insert(wc.word, wc.count.get()).ok();
            }
        }
        offset += size_of::<WordCount>();
    }
}

async fn emit(
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    counts: &FnvIndexMap<[u8; 16], u32, 64>,
    assign: AssignReducerMessage,
) -> Result<(), SpaceCompError> {
    let mut writer = BufWriter::<WordCount, _>::new(
        tx,
        &mut bufs.msg,
        &mut bufs.payload,
        assign.los_addr,
        assign.job_id,
        OpCode::JobResult,
    );

    for (word, &count) in counts.iter() {
        let wc = WordCount::builder().word(word).count(count).build();
        writer.write(&wc).await?;
    }
    writer.flush().await?;
    Ok(())
}
