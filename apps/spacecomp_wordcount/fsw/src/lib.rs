#![no_std]

use leodos_libcfs::runtime::Runtime;
use leodos_protocols::application::spacecomp::io::writer::BufWriter;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
use leodos_protocols::network::spp::Apid;
use leodos_spacecomp::SpaceCompConfig;
use leodos_spacecomp::SpaceCompError;
use leodos_spacecomp::SpaceCompNode;
use leodos_spacecomp::node::Buffers;
use leodos_spacecomp::node::RxHandle;
use leodos_spacecomp::node::TxHandle;

use heapless::index_map::FnvIndexMap;
use leodos_protocols::application::spacecomp::packet::AssignCollectorPayload;
use leodos_protocols::application::spacecomp::packet::AssignMapperPayload;
use leodos_protocols::application::spacecomp::packet::AssignReducerPayload;
use zerocopy::network_endian::U32;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

// ── Data ────────────────────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
struct WordCount {
    word: [u8; 16],
    count: U32,
}

impl WordCount {
    fn new(word: &[u8], count: u32) -> Self {
        let mut buf = [0u8; 16];
        let len = word.len().min(16);
        buf[..len].copy_from_slice(&word[..len]);
        Self { word: buf, count: U32::new(count) }
    }
}

const SAMPLE_TEXT: &[u8] = b"\
the quick brown fox jumps over the lazy dog \
the fox runs fast and the dog sleeps well \
a brown dog and a quick fox met in the park \
the lazy fox did not jump over the brown dog \
quick quick quick the fox is very quick \
the dog is not lazy the dog is resting \
over the hill and through the woods \
the brown fox and the brown dog are friends \
jump jump the fox can jump very high \
the quick dog chased the lazy fox home";

const NUM_SATS: u8 = bindings::SPACECOMP_WORDCOUNT_NUM_SATS as u8;
const MAX_CHUNK: usize = 256;

fn partition_text(partition_id: u8) -> &'static [u8] {
    let chunk_size = SAMPLE_TEXT.len() / NUM_SATS as usize;
    let start = partition_id as usize * chunk_size;
    if start >= SAMPLE_TEXT.len() {
        return &[];
    }
    let end = if partition_id == NUM_SATS - 1 {
        SAMPLE_TEXT.len()
    } else {
        let mut e = start + chunk_size;
        while e < SAMPLE_TEXT.len() && SAMPLE_TEXT[e] != b' ' {
            e += 1;
        }
        e.min(SAMPLE_TEXT.len())
    };
    &SAMPLE_TEXT[start..end]
}

// ── Role implementations ────────────────────────────────────

async fn collect(
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    job_id: u16,
    assign: AssignCollectorPayload,
) -> Result<(), SpaceCompError> {
    let partition = partition_text(assign.partition_id());
    for chunk in partition.chunks(MAX_CHUNK) {
        let m = SpaceCompMessage::builder()
            .buffer(&mut bufs.msg)
            .op_code(OpCode::DataChunk)
            .job_id(job_id)
            .payload_len(chunk.len())
            .build()?;
        m.payload_mut().copy_from_slice(chunk);
        tx.send(assign.mapper_addr(), m.as_bytes()).await.ok();
    }
    Ok(())
}

async fn map(
    rx: &mut RxHandle<'_>,
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    job_id: u16,
    assign: AssignMapperPayload,
) -> Result<(), SpaceCompError> {
    let mut received = 0u8;
    {
        let mut writer =
            BufWriter::<WordCount, _>::new(tx, &mut bufs.msg, assign.reducer_addr(), job_id, OpCode::DataChunk);

        loop {
            let mut payload = [0u8; MAX_CHUNK];
            let Ok(maybe_len) = rx
                .recv_with(|data| -> Option<usize> {
                    let msg = SpaceCompMessage::parse(data).ok()?;
                    if msg.op_code() != Ok(OpCode::DataChunk) {
                        return None;
                    }
                    let n = msg.payload().len().min(MAX_CHUNK);
                    payload[..n].copy_from_slice(&msg.payload()[..n]);
                    Some(n)
                })
                .await
            else {
                return Ok(());
            };
            let Some(len) = maybe_len else { continue };

            for word in payload[..len].split(|&b| b == b' ' || b == b'\n' || b == b'\t') {
                if word.is_empty() || word.len() > 16 {
                    continue;
                }
                let wc = WordCount::new(word, 1);
                writer.write(&wc).await?;
            }
            writer.flush().await?;

            received += 1;
            if received >= assign.collector_count() {
                break;
            }
        }
    }

    let done = SpaceCompMessage::builder()
        .buffer(&mut bufs.msg)
        .op_code(OpCode::PhaseDone)
        .job_id(job_id)
        .payload_len(0)
        .build()?;
    tx.send(assign.reducer_addr(), done.as_bytes()).await.ok();
    Ok(())
}

async fn reduce(
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
                let mut writer = BufWriter::<WordCount, _>::new(
                    tx,
                    &mut bufs.msg,
                    assign.los_addr(),
                    job_id,
                    OpCode::JobResult,
                );
                for (word, &count) in counts.iter() {
                    let wc = WordCount::new(word, count);
                    writer.write(&wc).await?;
                }
                writer.flush().await?;
                return Ok(());
            }
        }
    }
}

// ── Dispatch ─────────────────────────────────────────────────

async fn handle(
    rx: &mut RxHandle<'_>,
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    _point: leodos_protocols::network::isl::torus::Point,
    _source: leodos_protocols::network::isl::address::Address,
    len: usize,
) -> Result<(), SpaceCompError> {
    let msg = SpaceCompMessage::parse(&bufs.recv[..len])
        .map_err(SpaceCompError::Parse)?;
    let op = msg.op_code().map_err(SpaceCompError::Parse)?;
    let job_id = msg.job_id();

    match op {
        OpCode::AssignCollector => {
            let p = msg.parse_payload(leodos_protocols::application::spacecomp::packet::ParseError::AssignCollector)?;
            collect(tx, bufs, job_id, p).await
        }
        OpCode::AssignMapper => {
            let p = msg.parse_payload(leodos_protocols::application::spacecomp::packet::ParseError::AssignMapper)?;
            map(rx, tx, bufs, job_id, p).await
        }
        OpCode::AssignReducer => {
            let p = msg.parse_payload(leodos_protocols::application::spacecomp::packet::ParseError::AssignReducer)?;
            reduce(rx, tx, bufs, job_id, p).await
        }
        _ => Ok(()),
    }
}

// ── Entry point ─────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn SPACECOMP_WORDCOUNT_AppMain() {
    Runtime::new().run(async {
        let config = SpaceCompConfig {
            num_orbits: bindings::SPACECOMP_WORDCOUNT_NUM_ORBITS as u8,
            num_sats: NUM_SATS,
            altitude_m: 550_000.0,
            inclination_deg: 87.0,
            apid: Apid::new(bindings::SPACECOMP_WORDCOUNT_APID as u16).unwrap(),
            rto_ms: 1000,
            router_send_topic: 0,
            router_recv_topic: 0,
        };

        let node = SpaceCompNode::builder().config(config).build();
        node.run_with(handle).await
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
