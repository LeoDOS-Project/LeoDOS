use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;
use heapless::Vec;

use super::super::DataOutcome;
use super::super::GapOutcome;
use super::super::ReceiverBackend;
use super::super::ReceiverError;
use super::super::base::ReceiverBase;
use super::super::utils::GapTracker;

/// Half-memory backend — reorder and reassembly share one buffer.
///
/// Use when memory is scarce: segments are placed directly at
/// their final byte offset in a single flat buffer, so there is
/// no separate reorder stage. Static memory is just `REASM`.
///
/// Trade-off: each delivery requires an O(REASM) byte shift to
/// reclaim the consumed prefix, and OOO insert is O(WIN) for
/// gap bookkeeping. Segments tile at MTU boundaries, so each
/// slot reserves a full MTU regardless of payload size.
///
/// * `REASM` — reassembly buffer size (the only buffer)
/// * `WIN` — maximum gap tracker intervals
/// * `MTU` — maximum segment payload size
pub struct LiteReceiver<const REASM: usize, const WIN: usize, const MTU: usize> {
    /// Sequence tracking state.
    base: ReceiverBase,
    /// Contiguous buffer where segments are placed at computed offsets.
    message_buf: [u8; REASM],
    /// Tracks unfilled byte ranges in the reassembly buffer.
    gaps: GapTracker<WIN>,
    /// Sorted list of byte offsets where complete messages end.
    message_ends: Vec<usize, WIN>,
    /// Sequence number corresponding to byte offset 0 in the buffer.
    base_seq: u16,
    /// Whether a multi-segment reassembly is in progress.
    reassembly_in_progress: bool,
    /// Length of a fully reassembled message, if one is ready.
    complete_message_len: Option<usize>,
    /// Deferred byte shift to apply before the next operation.
    pending_shift: usize,
    /// Number of segments consumed by the pending shift.
    pending_segs: u16,
}

impl<const REASM: usize, const WIN: usize, const MTU: usize> ReceiverBackend
    for LiteReceiver<REASM, WIN, MTU>
{
    fn new() -> Self {
        Self {
            base: ReceiverBase::new(),
            message_buf: [0u8; REASM],
            gaps: GapTracker::new(),
            message_ends: Vec::new(),
            base_seq: 0,
            reassembly_in_progress: false,
            complete_message_len: None,
            pending_shift: 0,
            pending_segs: 0,
        }
    }

    fn handle_data(
        &mut self,
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &[u8],
    ) -> Result<DataOutcome, ReceiverError> {
        if self.complete_message_len.is_none() {
            self.apply_pending_shift();
        }

        let distance = self.base.distance(seq);
        let seq_before = self.base.expected_seq_raw();

        let max_dist = Self::MAX_SEGS as u16;
        if distance == 0 || distance < max_dist && !self.base.is_ooo_duplicate(distance) {
            if distance > 0 {
                self.base.record_ooo(distance);
            }
            self.place_segment(seq, flags, payload)?;
            self.try_deliver()?;
        }

        let progressed = self.base.expected_seq_raw() != seq_before;
        let has_gap = self.gaps.has_gaps() || !self.message_ends.is_empty();
        Ok(DataOutcome { progressed, has_gap })
    }

    fn skip_gap(&mut self) -> Result<GapOutcome, ReceiverError> {
        if self.complete_message_len.is_none() {
            self.apply_pending_shift();
        }

        if self.reassembly_in_progress {
            self.gaps.reset();
            self.message_ends.clear();
            self.reassembly_in_progress = false;
        }

        let shift = MTU;
        if shift < REASM {
            self.message_buf.copy_within(shift.., 0);
        }
        self.gaps.shift(shift);
        self.message_ends.retain(|&e| e > shift);
        for e in self.message_ends.iter_mut() {
            *e -= shift;
        }

        self.base.advance();
        self.base_seq = self.base_seq.wrapping_add(1) & SequenceCount::MAX as u16;

        self.try_deliver()?;

        let has_gap = self.gaps.has_gaps() || !self.message_ends.is_empty();
        Ok(GapOutcome { has_gap })
    }

    fn take_message(&mut self) -> Option<&[u8]> {
        self.complete_message_len
            .take()
            .map(|len| &self.message_buf[..len])
    }

    fn reassembly_data(&self, len: usize) -> &[u8] {
        &self.message_buf[..len]
    }

    fn has_message(&self) -> bool {
        self.complete_message_len.is_some()
    }

    fn message_len(&self) -> Option<usize> {
        self.complete_message_len
    }

    fn consume_message<Ret>(&mut self, f: impl FnOnce(&[u8]) -> Ret) -> Option<Ret> {
        let len = self.complete_message_len.take()?;
        Some(f(&self.message_buf[..len]))
    }

    fn expected_seq(&self) -> SequenceCount {
        self.base.expected_seq()
    }

    fn recv_bitmap(&self) -> u16 {
        self.base.recv_bitmap()
    }
}

impl<const REASM: usize, const WIN: usize, const MTU: usize> LiteReceiver<REASM, WIN, MTU> {
    /// Maximum number of segments that fit in the reassembly buffer.
    const MAX_SEGS: usize = REASM / MTU;

    /// Shift the buffer and metadata to consume delivered messages.
    fn apply_pending_shift(&mut self) {
        let shift = self.pending_shift;
        if shift == 0 {
            return;
        }
        if shift < REASM {
            self.message_buf.copy_within(shift.., 0);
        }
        self.gaps.shift(shift);
        self.message_ends.retain(|&e| e > shift);
        for e in self.message_ends.iter_mut() {
            *e -= shift;
        }
        self.base_seq = self.base_seq.wrapping_add(self.pending_segs) & SequenceCount::MAX as u16;
        self.pending_shift = 0;
        self.pending_segs = 0;
    }

    /// Compute the segment distance from `base_seq` to `seq` with wrapping.
    fn seg_distance(&self, seq: SequenceCount) -> u16 {
        seq.value().wrapping_sub(self.base_seq) & SequenceCount::MAX
    }

    /// Copy a segment's payload into the reassembly buffer at its computed offset.
    fn place_segment(
        &mut self,
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &[u8],
    ) -> Result<(), ReceiverError> {
        let d = self.seg_distance(seq) as usize;
        if d >= Self::MAX_SEGS {
            return Ok(());
        }
        let start = d * MTU;
        let end = start + payload.len();
        if end > REASM {
            return Err(ReceiverError::MessageTooLarge);
        }
        self.message_buf[start..end].copy_from_slice(payload);

        let slot_end = (start + MTU).min(REASM);
        self.gaps.fill(start, slot_end);

        match flags {
            SequenceFlag::First => {
                self.reassembly_in_progress = true;
            }
            SequenceFlag::Last | SequenceFlag::Unsegmented => {
                let pos = self
                    .message_ends
                    .iter()
                    .position(|&e| e > end)
                    .unwrap_or(self.message_ends.len());
                let _ = self.message_ends.insert(pos, end);
            }
            SequenceFlag::Continuation => {}
        }

        Ok(())
    }

    /// Deliver complete messages whose gaps have all been filled.
    fn try_deliver(&mut self) -> Result<(), ReceiverError> {
        loop {
            let msg_end;

            if self.pending_shift > 0 {
                let Some(&pre) = self.message_ends.first() else {
                    break;
                };
                if !self.gaps.is_complete_to(pre) {
                    break;
                }
                self.apply_pending_shift();
                msg_end = *self.message_ends.first().unwrap();
            } else {
                let Some(&end) = self.message_ends.first() else {
                    break;
                };
                if !self.gaps.is_complete_to(end) {
                    break;
                }
                msg_end = end;
            }

            self.message_ends.remove(0);
            self.complete_message_len = Some(msg_end);
            self.reassembly_in_progress = false;

            let segs = (msg_end + MTU - 1) / MTU;
            for _ in 0..segs {
                self.base.advance();
            }
            self.pending_shift = segs * MTU;
            self.pending_segs = segs as u16;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_immediate() -> super::super::super::ReceiverConfig {
        super::super::super::ReceiverConfig {
            local_address: crate::network::isl::address::Address::satellite(1, 1),
            apid: crate::network::spp::Apid::new(0x42).unwrap(),
            function_code: 0,
            immediate_ack: true,
            ack_delay_ticks: 20,
            progress_timeout_ticks: None,
        }
    }

    const MTU: usize = 64;

    type Rx = LiteReceiver<{ 8 * MTU }, 8, MTU>;

    fn make() -> Rx {
        Rx::new()
    }

    #[test]
    fn segmented_mtu_aligned() {
        let mut rx = make();

        let first = [1u8; MTU];
        let cont = [2u8; MTU];
        let last = [3u8; 10];

        rx.handle_data(SequenceCount::from(0), SequenceFlag::First, &first)
            .unwrap();
        assert!(!rx.has_message());

        rx.handle_data(SequenceCount::from(1), SequenceFlag::Continuation, &cont)
            .unwrap();
        assert!(!rx.has_message());

        rx.handle_data(SequenceCount::from(2), SequenceFlag::Last, &last)
            .unwrap();
        assert!(rx.has_message());
        let msg = rx.take_message().unwrap();
        assert_eq!(&msg[..MTU], &first);
        assert_eq!(&msg[MTU..2 * MTU], &cont);
        assert_eq!(&msg[2 * MTU..2 * MTU + 10], &last);
    }

    #[test]
    fn gap_merge_split() {
        let mut rx = make();
        let p = [0u8; 10];

        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &p)
            .unwrap();
        rx.take_message();

        rx.handle_data(SequenceCount::from(3), SequenceFlag::Unsegmented, &p)
            .unwrap();

        rx.handle_data(SequenceCount::from(1), SequenceFlag::Unsegmented, &p)
            .unwrap();
        assert!(rx.has_message());
        rx.take_message();

        rx.handle_data(SequenceCount::from(2), SequenceFlag::Unsegmented, &p)
            .unwrap();
        // seq=2 fills the gap; both seq=2 and seq=3 pass through try_deliver,
        // with seq=3 landing in complete_message_len last.
        assert!(rx.has_message());
    }
}
