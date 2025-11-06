use crate::spp::SequenceCount;
use crate::spp::SequenceFlag;

/// An iterator that breaks a large data slice into chunks suitable for
/// creating a sequence of segmented `SpacePacket`s.
pub struct Segmenter<'a> {
    data: &'a [u8],
    segment_size: usize,
    sequence_count: SequenceCount,
    position: usize,
    is_first: bool,
}

/// Contains the necessary data to construct one `SpacePacket` in a segmented sequence.
#[derive(Debug, PartialEq, Eq)]
pub struct SegmentedPacketData<'a> {
    pub sequence_flag: SequenceFlag,
    pub sequence_count: SequenceCount,
    pub payload: &'a [u8],
}

impl<'a> Segmenter<'a> {
    /// Creates a new `Segmenter` iterator.
    pub fn new(data: &'a [u8], segment_size: usize, start_count: SequenceCount) -> Self {
        Self {
            data,
            segment_size,
            sequence_count: start_count,
            position: 0,
            is_first: true,
        }
    }
}

impl<'a> Iterator for Segmenter<'a> {
    type Item = SegmentedPacketData<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.data.len() {
            return None;
        }

        let sequence_flag = if self.is_first {
            self.is_first = false;
            SequenceFlag::First
        } else if self.position + self.segment_size >= self.data.len() {
            SequenceFlag::Last
        } else {
            SequenceFlag::Continuation
        };

        let end = (self.position + self.segment_size).min(self.data.len());
        let payload = &self.data[self.position..end];
        self.position = end;

        let item = SegmentedPacketData {
            sequence_flag,
            sequence_count: self.sequence_count,
            payload,
        };

        self.sequence_count.increment();
        Some(item)
    }
}
