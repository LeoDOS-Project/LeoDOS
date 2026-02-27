use bon::bon;
use leodos_protocols::application::spacecomp::packet::{BuildError, OpCode, SpaceCompMessage};
use zerocopy::network_endian::U32;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct WordCount {
    pub word: [u8; 16],
    pub count: U32,
}

#[bon]
impl WordCount {
    #[builder]
    pub fn new(word: &[u8], count: u32) -> Self {
        let mut buf = [0u8; 16];
        let len = word.len().min(16);
        buf[..len].copy_from_slice(&word[..len]);
        Self {
            word: buf,
            count: U32::new(count),
        }
    }

    #[builder]
    pub fn message<'a>(
        buffer: &'a mut [u8],
        word: &[u8],
        count: u32,
        job_id: u16,
    ) -> Result<&'a SpaceCompMessage, BuildError> {
        let wc = Self::builder().word(word).count(count).build();
        SpaceCompMessage::builder()
            .buffer(buffer)
            .op_code(OpCode::DataChunk)
            .job_id(job_id)
            .payload(wc.as_bytes())
            .build()
    }
}

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

pub fn partition_text(partition_id: u8, total_partitions: u8) -> &'static [u8] {
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
