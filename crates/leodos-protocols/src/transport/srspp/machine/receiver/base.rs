use crate::network::spp::SequenceCount;

/// Sequence tracking state shared by all receiver backends.
pub struct ReceiverBase {
    /// Next expected sequence number.
    expected_seq: u16,
    /// Bitmap of received out-of-order packets relative to `expected_seq`.
    recv_bitmap: u16,
}

impl ReceiverBase {
    /// Create a new sequence tracker starting at sequence 0.
    pub fn new() -> Self {
        Self {
            expected_seq: 0,
            recv_bitmap: 0,
        }
    }

    /// Returns the next expected sequence number.
    pub fn expected_seq(&self) -> SequenceCount {
        SequenceCount::from(self.expected_seq)
    }

    /// Returns the raw u16 expected sequence number.
    pub fn expected_seq_raw(&self) -> u16 {
        self.expected_seq
    }

    /// Returns the selective ACK bitmap.
    pub fn recv_bitmap(&self) -> u16 {
        self.recv_bitmap
    }

    /// Compute the forward distance from `expected_seq` to `seq`.
    pub fn distance(&self, seq: SequenceCount) -> u16 {
        seq.value().wrapping_sub(self.expected_seq) & SequenceCount::MAX
    }

    /// Check if an out-of-order packet at `distance` is a duplicate.
    pub fn is_ooo_duplicate(&self, distance: u16) -> bool {
        debug_assert!(distance > 0);
        let bit_pos = distance - 1;
        let mask = 1u16 << bit_pos;
        self.recv_bitmap & mask != 0
    }

    /// Record receipt of an out-of-order packet in the bitmap.
    pub fn record_ooo(&mut self, distance: u16) {
        debug_assert!(distance > 0);
        let bit_pos = distance - 1;
        self.recv_bitmap |= 1u16 << bit_pos;
    }

    /// Advance `expected_seq` by one and shift the bitmap.
    pub fn advance(&mut self) {
        self.expected_seq = (self.expected_seq + 1) & SequenceCount::MAX;
        self.recv_bitmap >>= 1;
    }
}
