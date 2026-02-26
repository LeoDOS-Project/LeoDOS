use heapless::index_map::FnvIndexMap;
use leodos_libcfs::cfe::evs::event;
use leodos_protocols::mission::compute::packet::OpCode;
use leodos_protocols::network::isl::address::Address;
use zerocopy::IntoBytes;

use crate::data::{WordCount, WORD_COUNT_SIZE};
use crate::isl::{self, NodeHandle};

const BATCH_BUF_SIZE: usize = 256;

pub struct MapState {
    counts: FnvIndexMap<[u8; 16], u32, 64>,
}

impl MapState {
    pub fn new() -> Self {
        Self {
            counts: FnvIndexMap::new(),
        }
    }

    pub fn ingest_chunk(&mut self, chunk: &[u8]) {
        for word_bytes in chunk.split(|&b| b == b' ' || b == b'\n' || b == b'\t') {
            if word_bytes.is_empty() || word_bytes.len() > 16 {
                continue;
            }
            let mut key = [0u8; 16];
            key[..word_bytes.len()].copy_from_slice(word_bytes);

            if let Some(c) = self.counts.get_mut(&key) {
                *c += 1;
            } else {
                self.counts.insert(key, 1).ok();
            }
        }
    }

    pub async fn emit_results(
        &self,
        handle: &mut NodeHandle<'_>,
        reducer_addr: Address,
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
                isl::send(handle, reducer_addr, OpCode::DataChunk, job_id, &buf[..payload_len])
                    .await
                    .ok();
                idx = 0;
            }
        }

        if idx > 0 {
            let payload_len = idx * WORD_COUNT_SIZE;
            isl::send(handle, reducer_addr, OpCode::DataChunk, job_id, &buf[..payload_len])
                .await
                .ok();
        }

        event::info(0, "Mapper: results sent").ok();
    }
}
