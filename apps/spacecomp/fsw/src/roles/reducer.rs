use heapless::index_map::FnvIndexMap;
use leodos_protocols::mission::compute::packet::OpCode;
use leodos_protocols::network::isl::address::Address;
use zerocopy::{FromBytes, IntoBytes};

use crate::data::{WordCount, WORD_COUNT_SIZE};
use crate::isl::{self, NodeHandle};

const BATCH_BUF_SIZE: usize = 256;

pub async fn run(
    handle: &mut NodeHandle<'_>,
    los_addr: Address,
    job_id: u16,
    expected: u8,
) {
    let mut counts: FnvIndexMap<[u8; 16], u32, 64> = FnvIndexMap::new();
    let mut received = 0u8;
    let mut buf = [0u8; 512];

    loop {
        let Ok(msg) = handle.recv().await else { return };
        let Some(cmd) = isl::parse(&msg.data, &mut buf) else {
            continue;
        };
        if cmd.op_code != OpCode::DataChunk {
            continue;
        }

        ingest(&mut counts, &buf[..cmd.payload_len]);
        received += 1;

        if received >= expected {
            emit(handle, &counts, los_addr, job_id).await;
            return;
        }
    }
}

fn ingest(counts: &mut FnvIndexMap<[u8; 16], u32, 64>, chunk: &[u8]) {
    let mut offset = 0;
    while offset + WORD_COUNT_SIZE <= chunk.len() {
        if let Ok(wc) = WordCount::read_from_bytes(&chunk[offset..offset + WORD_COUNT_SIZE]) {
            let word = wc.word;
            let count = wc.count.get();
            if let Some(c) = counts.get_mut(&word) {
                *c += count;
            } else {
                counts.insert(word, count).ok();
            }
        }
        offset += WORD_COUNT_SIZE;
    }
}

async fn emit(
    handle: &mut NodeHandle<'_>,
    counts: &FnvIndexMap<[u8; 16], u32, 64>,
    los_addr: Address,
    job_id: u16,
) {
    let max_per_packet = BATCH_BUF_SIZE / WORD_COUNT_SIZE;
    let mut buf = [0u8; BATCH_BUF_SIZE];
    let mut idx = 0;

    for (word, &count) in counts.iter() {
        let wc = WordCount::new(word, count);
        let offset = idx * WORD_COUNT_SIZE;
        buf[offset..offset + WORD_COUNT_SIZE].copy_from_slice(wc.as_bytes());
        idx += 1;

        if idx >= max_per_packet {
            let payload_len = idx * WORD_COUNT_SIZE;
            isl::send(handle, los_addr, OpCode::JobResult, job_id, &buf[..payload_len])
                .await
                .ok();
            idx = 0;
        }
    }

    if idx > 0 {
        let payload_len = idx * WORD_COUNT_SIZE;
        isl::send(handle, los_addr, OpCode::JobResult, job_id, &buf[..payload_len])
            .await
            .ok();
    }
}
