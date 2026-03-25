#![no_std]

use leodos_libcfs::runtime::Runtime;
use leodos_spacecomp::Collector;
use leodos_spacecomp::Mapper;
use leodos_spacecomp::Reducer;
use leodos_spacecomp::Schema;
use leodos_spacecomp::Sink;
use leodos_spacecomp::SpaceCompConfig;
use leodos_spacecomp::SpaceCompJob;
use leodos_spacecomp::SpaceCompNode;

use heapless::index_map::FnvIndexMap;
use leodos_protocols::network::spp::Apid;
use zerocopy::network_endian::U32;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Ref;
use zerocopy::Unaligned;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

// ── Data types ──────────────────────────────────────────────

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

impl Schema for WordCount {
    type Key<'a> = [u8; 16];
    fn key<'a>(pkt: &Ref<&'a [u8], Self>) -> Self::Key<'a> {
        pkt.word
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

fn partition_text(partition_id: u8, total_partitions: u8) -> &'static [u8] {
    let chunk_size = SAMPLE_TEXT.len() / total_partitions as usize;
    let start = partition_id as usize * chunk_size;
    if start >= SAMPLE_TEXT.len() {
        return &[];
    }
    let end = if partition_id == total_partitions - 1 {
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

// ── SpaceCompJob implementation ─────────────────────────────

struct WordCountJob;

impl SpaceCompJob for WordCountJob {
    type Collected = WordCount;
    type Mapped = WordCount;
    type Result = WordCount;

    fn collector(&mut self) -> impl Collector<Input = WordCount, Output = WordCount> {
        TextCollector
    }

    fn mapper(&mut self) -> impl Mapper<Input = WordCount, Output = WordCount> {
        WordSplitter
    }

    fn reducer(&mut self) -> impl Reducer<Input = WordCount, Output = WordCount> {
        WordAggregator::new()
    }
}

// ── Collector: partitions text into WordCount records ────────

struct TextCollector;

impl Collector for TextCollector {
    type Input = WordCount;
    type Output = WordCount;

    async fn collect<S: Sink<Input = WordCount>>(
        &mut self,
        input: WordCount,
        sink: &mut S,
    ) -> Result<(), S::Error> {
        sink.write(&input).await
    }
}

// ── Mapper: identity (text already tokenized by collector) ──

struct WordSplitter;

impl Mapper for WordSplitter {
    type Input = WordCount;
    type Output = WordCount;

    async fn map<S: Sink<Input = WordCount>>(
        &mut self,
        input: WordCount,
        sink: &mut S,
    ) -> Result<(), S::Error> {
        sink.write(&input).await
    }
}

// ── Reducer: aggregate word counts ──────────────────────────

struct WordAggregator {
    counts: FnvIndexMap<[u8; 16], u32, 64>,
}

impl WordAggregator {
    fn new() -> Self {
        Self {
            counts: FnvIndexMap::new(),
        }
    }
}

impl Reducer for WordAggregator {
    type Input = WordCount;
    type Output = WordCount;

    fn reduce(&mut self, val: WordCount) -> impl Iterator<Item = WordCount> {
        self.counts
            .entry(val.word)
            .and_modify(|c| *c += val.count.get())
            .or_insert_with(|| val.count.get())
            .ok();
        core::iter::empty()
    }
}

// ── Entry point ─────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn SPACECOMP_WORDCOUNT_AppMain() {
    Runtime::new().run(async {
        let config = SpaceCompConfig {
            num_orbits: bindings::SPACECOMP_WORDCOUNT_NUM_ORBITS as u8,
            num_sats: bindings::SPACECOMP_WORDCOUNT_NUM_SATS as u8,
            altitude_m: 550_000.0,
            inclination_deg: 87.0,
            apid: Apid::new(bindings::SPACECOMP_WORDCOUNT_APID as u16).unwrap(),
            rto_ms: 1000,
            router_send_topic: 0,
            router_recv_topic: 0,
        };

        let mut node = SpaceCompNode::builder()
            .job(WordCountJob)
            .config(config)
            .build();

        node.run().await
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
