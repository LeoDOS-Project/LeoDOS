use heapless::index_map::FnvIndexMap;
use leodos_protocols::application::spacecomp::packet::{
    AssignReducerMessage, OpCode, SpaceCompMessage,
};
use leodos_protocols::network::isl::address::Address;
use core::mem::size_of;
use zerocopy::{FromBytes, IntoBytes};
use crate::data::WordCount;
use crate::Buffers;
use crate::SpaceCompError;
use crate::NodeHandle;

pub async fn run(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    assign: AssignReducerMessage,
) -> Result<(), SpaceCompError> {
    let mut counts: FnvIndexMap<[u8; 16], u32, 64> = FnvIndexMap::new();
    let mut done_count = 0u8;

    loop {
        let Ok((_, len)) = handle.recv(&mut bufs.recv).await else {
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
                    emit(handle, bufs, &counts, assign.los_addr, assign.job_id).await?;
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
            let word = wc.word;
            let count = wc.count.get();
            if let Some(c) = counts.get_mut(&word) {
                *c += count;
            } else {
                counts.insert(word, count).ok();
            }
        }
        offset += size_of::<WordCount>();
    }
}

async fn emit(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    counts: &FnvIndexMap<[u8; 16], u32, 64>,
    los_addr: Address,
    job_id: u16,
) -> Result<(), SpaceCompError> {
    let max_per_packet = bufs.payload.len() / size_of::<WordCount>();
    let mut idx = 0;

    for (word, &count) in counts.iter() {
        let wc = WordCount::builder().word(word).count(count).build();
        let offset = idx * size_of::<WordCount>();
        bufs.payload[offset..offset + size_of::<WordCount>()].copy_from_slice(wc.as_bytes());
        idx += 1;

        if idx >= max_per_packet {
            let payload_len = idx * size_of::<WordCount>();
            let msg = SpaceCompMessage::builder()
                .buffer(&mut bufs.msg)
                .op_code(OpCode::JobResult)
                .job_id(job_id)
                .payload(&bufs.payload[..payload_len])
                .build()?;
            handle.send(los_addr, msg).await.ok();
            idx = 0;
        }
    }

    if idx > 0 {
        let payload_len = idx * size_of::<WordCount>();
        let msg = SpaceCompMessage::builder()
            .buffer(&mut bufs.msg)
            .op_code(OpCode::JobResult)
            .job_id(job_id)
            .payload(&bufs.payload[..payload_len])
            .build()?;
        handle.send(los_addr, msg).await.ok();
    }

    Ok(())
}
