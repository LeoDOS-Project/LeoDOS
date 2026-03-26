#![no_std]

use leodos_protocols::network::spp::Apid;
use leodos_spacecomp::bufwriter::BufWriter;
use leodos_spacecomp::node::SpaceComp;
use leodos_spacecomp::transport::Rx;
use leodos_spacecomp::transport::Tx;
use leodos_spacecomp::SpaceCompConfig;
use leodos_spacecomp::SpaceCompError;
use leodos_spacecomp::SpaceCompNode;

use heapless::index_map::FnvIndexMap;
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
        Self {
            word: buf,
            count: U32::new(count),
        }
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

// ── SpaceComp implementation ─────────────────────────────────

struct WordCount2;

impl SpaceComp for WordCount2 {
    async fn collect(&self, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let partition = partition_text(tx.partition_id());
        for chunk in partition.chunks(MAX_CHUNK) {
            tx.send(chunk).await?;
        }
        Ok(())
    }

    async fn map(&self, mut rx: impl Rx, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut writer = BufWriter::<WordCount, _>::new(&mut tx);
        let mut payload = [0u8; MAX_CHUNK];

        while let Some(Ok(len)) = rx.recv(&mut payload).await {
            for word in payload[..len].split(|&b| b == b' ' || b == b'\n' || b == b'\t') {
                if word.is_empty() || word.len() > 16 {
                    continue;
                }
                writer.write(&WordCount::new(word, 1)).await?;
            }
            writer.flush().await?;
        }

        tx.done().await?;
        Ok(())
    }

    async fn reduce(&self, mut rx: impl Rx, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut counts: FnvIndexMap<[u8; 16], u32, 64> = FnvIndexMap::new();
        let mut recv_buf = [0u8; 512];

        while let Some(Ok(len)) = rx.recv(&mut recv_buf).await {
            let rec_size = core::mem::size_of::<WordCount>();
            for wc_bytes in recv_buf[..len].chunks_exact(rec_size) {
                let Ok(wc) = WordCount::read_from_bytes(wc_bytes) else { continue };
                counts
                    .entry(wc.word)
                    .and_modify(|c| *c += wc.count.get())
                    .or_insert_with(|| wc.count.get())
                    .ok();
            }
        }

        let mut writer = BufWriter::<WordCount, _>::new(&mut tx);
        for (word, &count) in counts.iter() {
            writer.write(&WordCount::new(word, count)).await?;
        }
        writer.flush().await?;
        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn SPACECOMP_WORDCOUNT_AppMain() {
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

    let node: SpaceCompNode = SpaceCompNode::builder()
        .config(config)
        .store(leodos_protocols::transport::srspp::dtn::NoStore)
        .reachable(leodos_protocols::transport::srspp::dtn::AlwaysReachable)
        .build();
    node.start(&WordCount2);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
