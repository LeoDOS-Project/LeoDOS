use zerocopy::network_endian::U32;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct WordCount {
    pub word: [u8; 16],
    pub count: U32,
}

impl WordCount {
    pub fn new(word_bytes: &[u8], count: u32) -> Self {
        let mut w = [0u8; 16];
        let len = word_bytes.len().min(16);
        w[..len].copy_from_slice(&word_bytes[..len]);
        Self {
            word: w,
            count: U32::new(count),
        }
    }

    pub fn word_str(&self) -> &[u8] {
        let end = self.word.iter().position(|&b| b == 0).unwrap_or(16);
        &self.word[..end]
    }
}

pub const WORD_COUNT_SIZE: usize = core::mem::size_of::<WordCount>();

pub const SAMPLE_TEXT: &[u8] = b"\
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

pub fn partition_text(partition_id: u8, total_partitions: u8) -> (&'static [u8], usize) {
    let text = SAMPLE_TEXT;
    let chunk_size = text.len() / total_partitions as usize;
    let start = partition_id as usize * chunk_size;
    let end = if partition_id == total_partitions - 1 {
        text.len()
    } else {
        let mut e = start + chunk_size;
        while e < text.len() && text[e] != b' ' {
            e += 1;
        }
        e
    };
    if start >= text.len() {
        return (&[], 0);
    }
    let slice = &text[start..end.min(text.len())];
    (slice, slice.len())
}
