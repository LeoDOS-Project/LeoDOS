use leodos_protocols::application::spacecomp::io::writer::BufWriter;
use leodos_protocols::application::spacecomp::packet::{
    AssignMapperMessage, OpCode, SpaceCompMessage,
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
    assign: AssignMapperMessage,
) -> Result<(), SpaceCompError> {
    let mut received = 0u8;
    {
        let mut writer = BufWriter::<WordCount, _>::new(
            tx,
            &mut bufs.msg,
            &mut bufs.payload,
            assign.reducer_addr,
            assign.job_id,
            OpCode::DataChunk,
        );

        loop {
            let Ok((_, len)) = rx.recv(&mut bufs.recv).await else {
                return Ok(());
            };
            let Ok(msg) = SpaceCompMessage::parse(&bufs.recv[..len]) else {
                continue;
            };
            if msg.op_code() != Ok(OpCode::DataChunk) {
                continue;
            }

            for word_bytes in msg.payload().split(|&b| b == b' ' || b == b'\n' || b == b'\t') {
                if word_bytes.is_empty() || word_bytes.len() > 16 {
                    continue;
                }
                let wc = WordCount::builder().word(word_bytes).count(1).build();
                writer.write(&wc).await?;
            }
            writer.flush().await?;

            received += 1;
            if received >= assign.collector_count {
                break;
            }
        }
    }

    let done = SpaceCompMessage::builder()
        .buffer(&mut bufs.msg)
        .op_code(OpCode::PhaseDone)
        .job_id(assign.job_id)
        .payload(&[])
        .build()?;
    tx.send(assign.reducer_addr, done).await.ok();
    Ok(())
}
