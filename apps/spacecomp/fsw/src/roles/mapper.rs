use core::mem::size_of;
use leodos_protocols::application::spacecomp::packet::{
    AssignMapperMessage, OpCode, SpaceCompMessage,
};
use zerocopy::IntoBytes;

use crate::data::WordCount;
use crate::Buffers;
use crate::NodeHandle;
use crate::SpaceCompError;

const MAX_PER_PACKET: usize = 256 / size_of::<WordCount>();

pub async fn run(
    handle: &mut NodeHandle<'_>,
    bufs: &mut Buffers,
    assign: AssignMapperMessage,
) -> Result<(), SpaceCompError> {
    let mut received = 0u8;

    loop {
        let Ok((_, len)) = handle.recv(&mut bufs.recv).await else {
            return Ok(());
        };
        let Ok(msg) = SpaceCompMessage::parse(&bufs.recv[..len]) else {
            continue;
        };
        if msg.op_code() != Ok(OpCode::DataChunk) {
            continue;
        }

        let payload_start = SpaceCompMessage::HEADER_SIZE;
        let mut idx = 0;

        for word_bytes in bufs.recv[payload_start..len]
            .split(|&b| b == b' ' || b == b'\n' || b == b'\t')
        {
            if word_bytes.is_empty() || word_bytes.len() > 16 {
                continue;
            }
            let wc = WordCount::builder().word(word_bytes).count(1).build();
            let offset = idx * size_of::<WordCount>();
            bufs.payload[offset..offset + size_of::<WordCount>()]
                .copy_from_slice(wc.as_bytes());
            idx += 1;

            if idx >= MAX_PER_PACKET {
                let msg = SpaceCompMessage::builder()
                    .buffer(&mut bufs.msg)
                    .op_code(OpCode::DataChunk)
                    .job_id(assign.job_id)
                    .payload(&bufs.payload[..idx * size_of::<WordCount>()])
                    .build()?;
                handle.send(assign.reducer_addr, msg).await.ok();
                idx = 0;
            }
        }

        if idx > 0 {
            let msg = SpaceCompMessage::builder()
                .buffer(&mut bufs.msg)
                .op_code(OpCode::DataChunk)
                .job_id(assign.job_id)
                .payload(&bufs.payload[..idx * size_of::<WordCount>()])
                .build()?;
            handle.send(assign.reducer_addr, msg).await.ok();
        }

        received += 1;
        if received >= assign.collector_count {
            let done = SpaceCompMessage::builder()
                .buffer(&mut bufs.msg)
                .op_code(OpCode::PhaseDone)
                .job_id(assign.job_id)
                .payload(&[])
                .build()?;
            handle.send(assign.reducer_addr, done).await.ok();
            return Ok(());
        }
    }
}
