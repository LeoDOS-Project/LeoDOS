use heapless::index_map::FnvIndexMap;
use leodos_libcfs::cfe::evs::event;
use leodos_protocols::mission::compute::packet::OpCode;
use leodos_protocols::network::isl::address::Address;
use zerocopy::{FromBytes, IntoBytes};

use crate::data::{WordCount, WORD_COUNT_SIZE};
use crate::isl::{self, NodeHandle};

const BATCH_BUF_SIZE: usize = 256;

pub struct ReduceState {
    counts: FnvIndexMap<[u8; 16], u32, 64>,
}

impl ReduceState {
    pub fn new() -> Self {
        Self {
            counts: FnvIndexMap::new(),
        }
    }

    pub fn ingest_chunk(&mut self, chunk: &[u8]) {
        let mut offset = 0;
        while offset + WORD_COUNT_SIZE <= chunk.len() {
            if let Ok(wc) = WordCount::read_from_bytes(&chunk[offset..offset + WORD_COUNT_SIZE]) {
                let word = wc.word;
                let count = wc.count.get();
                if let Some(c) = self.counts.get_mut(&word) {
                    *c += count;
                } else {
                    self.counts.insert(word, count).ok();
                }
            }
            offset += WORD_COUNT_SIZE;
        }
    }

    pub async fn emit_results(
        &self,
        handle: &mut NodeHandle<'_>,
        los_addr: Address,
        job_id: u16,
    ) {
        let max_per_packet = BATCH_BUF_SIZE / WORD_COUNT_SIZE;
        let mut buf = [0u8; BATCH_BUF_SIZE];
        let mut idx = 0;

        for (word, &count) in self.counts.iter() {
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

        event::info(0, "Reducer: final results sent").ok();
    }
}
