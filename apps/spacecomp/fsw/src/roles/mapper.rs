use core::mem::size_of;
use heapless::index_map::FnvIndexMap;
use leodos_protocols::application::spacecomp::packet::{
    AssignMapperMessage, OpCode, SpaceCompMessage,
};
use leodos_protocols::network::isl::address::Address;
use zerocopy::network_endian::U32;
use zerocopy::IntoBytes;

use crate::data::WordCount;
use crate::Buffers;
use crate::SpaceCompError;
use crate::NodeHandle;

pub async fn run(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    assign: AssignMapperMessage,
) -> Result<(), SpaceCompError> {
    let mut counts: FnvIndexMap<[u8; 16], u32, 64> = FnvIndexMap::new();
    let mut received = 0u8;

    loop {
        let Ok((_, len)) = handle.recv(&mut bufs.recv).await else {
            return Ok(());
        };
        let Some(msg) = SpaceCompMessage::parse(&bufs.recv[..len]) else {
            continue;
        };
        if msg.op_code() != Ok(OpCode::DataChunk) {
            continue;
        }

        ingest(&mut counts, msg.payload());
        received += 1;

        if received >= assign.collector_count {
            emit(handle, bufs, &counts, assign.reducer_addr, assign.job_id).await;
            return Ok(());
        }
    }
}

fn ingest(counts: &mut FnvIndexMap<[u8; 16], u32, 64>, chunk: &[u8]) {
    for word_bytes in chunk.split(|&b| b == b' ' || b == b'\n' || b == b'\t') {
        if word_bytes.is_empty() || word_bytes.len() > 16 {
            continue;
        }
        let mut key = [0u8; 16];
        key[..word_bytes.len()].copy_from_slice(word_bytes);

        if let Some(c) = counts.get_mut(&key) {
            *c += 1;
        } else {
            counts.insert(key, 1).ok();
        }
    }
}

async fn emit(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    counts: &FnvIndexMap<[u8; 16], u32, 64>,
    reducer_addr: Address,
    job_id: u16,
) {
    let max_per_packet = bufs.payload.len() / size_of::<WordCount>();
    let mut idx = 0;

    for (&word, &count) in counts.iter() {
        let wc = WordCount {
            word,
            count: U32::new(count),
        };
        let offset = idx * size_of::<WordCount>();
        bufs.payload[offset..offset + size_of::<WordCount>()].copy_from_slice(wc.as_bytes());
        idx += 1;

        if idx >= max_per_packet {
            let payload_len = idx * size_of::<WordCount>();
            if let Some(msg) = SpaceCompMessage::builder()
                .buffer(&mut bufs.msg)
                .op_code(OpCode::DataChunk)
                .job_id(job_id)
                .payload(&bufs.payload[..payload_len])
                .build()
            {
                handle.send(reducer_addr, msg.as_bytes()).await.ok();
            }
            idx = 0;
        }
    }

    if idx > 0 {
        let payload_len = idx * size_of::<WordCount>();
        if let Some(msg) = SpaceCompMessage::builder()
            .buffer(&mut bufs.msg)
            .op_code(OpCode::DataChunk)
            .job_id(job_id)
            .payload(&bufs.payload[..payload_len])
            .build()
        {
            handle.send(reducer_addr, msg.as_bytes()).await.ok();
        }
    }
}
