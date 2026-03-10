use crate::network::isl::address::Address;
use crate::network::spp::{SequenceCount, SequenceFlag};
use heapless::Vec;

use super::super::utils::GapTracker;
use super::super::base::ReceiverBase;
use super::super::{
    ReceiverAction, ReceiverActions, ReceiverConfig, ReceiverError,
    ReceiverEvent,
};

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
    /// Shared receiver state (sequence tracking, timers, ACK logic).
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

impl<const REASM: usize, const WIN: usize, const MTU: usize>
    LiteReceiver<REASM, WIN, MTU>
{
    /// Maximum number of segments that fit in the reassembly buffer.
    const MAX_SEGS: usize = REASM / MTU;

    /// Create a new receiver for a specific remote sender.
    pub fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self {
            base: ReceiverBase::new(config, remote_address),
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

    /// Get the remote address.
    pub fn remote_address(&self) -> Address {
        self.base.remote_address()
    }

    /// Process an event and produce actions.
    ///
    /// The pending buffer shift is deferred while a message awaits
    /// consumption.  This keeps the message data at the front of
    /// `message_buf` stable so that [`consume_message`] can hand
    /// out a valid `&[u8]` even when the driver keeps calling
    /// `handle` in between.
    pub fn handle(
        &mut self,
        event: ReceiverEvent<'_>,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        if self.complete_message_len.is_none() {
            self.apply_pending_shift();
        }
        actions.clear();
        match event {
            ReceiverEvent::DataReceived {
                seq,
                flags,
                payload,
            } => self.handle_data(seq, flags, payload, actions),
            ReceiverEvent::AckTimeout => {
                self.base.handle_ack_timeout(actions);
                Ok(())
            }
            ReceiverEvent::ProgressTimeout => {
                self.handle_progress_timeout(actions)
            }
        }
    }

    /// Take the complete message.
    pub fn take_message(&mut self) -> Option<&[u8]> {
        self.complete_message_len
            .take()
            .map(|len| &self.message_buf[..len])
    }

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
        self.base_seq = self
            .base_seq
            .wrapping_add(self.pending_segs)
            & SequenceCount::MAX as u16;
        self.pending_shift = 0;
        self.pending_segs = 0;
    }

    /// Returns a slice of the reassembly buffer.
    pub fn reassembly_data(&self, len: usize) -> &[u8] {
        &self.message_buf[..len]
    }

    /// Check if there's a complete message ready.
    pub fn has_message(&self) -> bool {
        self.complete_message_len.is_some()
    }

    /// Returns the length of the pending message, if any.
    pub fn message_len(&self) -> Option<usize> {
        self.complete_message_len
    }

    /// Pass the pending message to `f` and mark it consumed.
    ///
    /// The message occupies `&message_buf[..len]` and is stable
    /// as long as `handle` has not been called since the last
    /// `take_message` / `consume_message`.
    pub fn consume_message<F, Ret>(&mut self, f: F) -> Option<Ret>
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        let len = self.complete_message_len.take()?;
        Some(f(&self.message_buf[..len]))
    }

    /// Get the current expected sequence number.
    pub fn expected_seq(&self) -> SequenceCount {
        self.base.expected_seq()
    }

    /// Compute the segment distance from `base_seq` to `seq` with wrapping.
    fn seg_distance(&self, seq: SequenceCount) -> u16 {
        seq.value().wrapping_sub(self.base_seq) & SequenceCount::MAX
    }

    /// Process an incoming data segment, placing it and attempting delivery.
    fn handle_data(
        &mut self,
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &[u8],
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        let distance = self.base.distance(seq);
        let seq_before = self.base.expected_seq_raw();
        let max_dist = Self::MAX_SEGS as u16;

        if distance < max_dist {
            if distance > 0 && self.base.is_ooo_duplicate(distance)
            {
            } else {
                if distance > 0 {
                    self.base.record_ooo(distance);
                }
                self.place_segment(seq, flags, payload)?;
                self.try_deliver(actions)?;
            }
        }

        let progressed =
            self.base.expected_seq_raw() != seq_before;
        let has_gap =
            self.gaps.has_gaps() || !self.message_ends.is_empty();
        self.base
            .apply_post_data_logic(actions, progressed, has_gap);
        Ok(())
    }

    /// Handle a progress timeout by discarding partial reassembly and advancing.
    fn handle_progress_timeout(
        &mut self,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
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
        self.base_seq =
            self.base_seq.wrapping_add(1) & SequenceCount::MAX as u16;

        self.try_deliver(actions)?;

        let has_gap =
            self.gaps.has_gaps() || !self.message_ends.is_empty();
        self.base
            .apply_post_progress_logic(actions, has_gap);
        Ok(())
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
    fn try_deliver(
        &mut self,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
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
            actions.push(ReceiverAction::MessageReady);

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

    fn test_remote() -> Address {
        Address::satellite(1, 2)
    }

    fn cfg_immediate() -> ReceiverConfig {
        ReceiverConfig {
            local_address: Address::satellite(1, 1),
            apid: crate::network::spp::Apid::new(0x42).unwrap(),
            function_code: 0,
            immediate_ack: true,
            ack_delay_ticks: 20,
            progress_timeout_ticks: None,
        }
    }

    fn cfg_progress(ticks: u32) -> ReceiverConfig {
        ReceiverConfig {
            progress_timeout_ticks: Some(ticks),
            ..cfg_immediate()
        }
    }

    const MTU: usize = 64;

    type Rx = LiteReceiver<{ 8 * MTU }, 8, MTU>;

    fn make(cfg: ReceiverConfig) -> Rx {
        Rx::new(cfg, test_remote())
    }

    #[test]
    fn single_unsegmented() {
        let mut rx = make(cfg_immediate());
        let mut a = ReceiverActions::new();
        let payload = [42u8; 10];
        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(0),
                flags: SequenceFlag::Unsegmented,
                payload: &payload,
            },
            &mut a,
        )
        .unwrap();
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &payload);
    }

    #[test]
    fn segmented_mtu_aligned() {
        let mut rx = make(cfg_immediate());
        let mut a = ReceiverActions::new();

        let first = [1u8; MTU];
        let cont = [2u8; MTU];
        let last = [3u8; 10];

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(0),
                flags: SequenceFlag::First,
                payload: &first,
            },
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(1),
                flags: SequenceFlag::Continuation,
                payload: &cont,
            },
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(2),
                flags: SequenceFlag::Last,
                payload: &last,
            },
            &mut a,
        )
        .unwrap();
        assert!(rx.has_message());
        let msg = rx.take_message().unwrap();
        assert_eq!(&msg[..MTU], &first);
        assert_eq!(&msg[MTU..2 * MTU], &cont);
        assert_eq!(&msg[2 * MTU..2 * MTU + 10], &last);
    }

    #[test]
    fn ooo_delivery() {
        let mut rx = make(cfg_immediate());
        let mut a = ReceiverActions::new();

        let p0 = [0xAAu8; 10];
        let p1 = [0xBBu8; 10];

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(1),
                flags: SequenceFlag::Unsegmented,
                payload: &p1,
            },
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(0),
                flags: SequenceFlag::Unsegmented,
                payload: &p0,
            },
            &mut a,
        )
        .unwrap();

        let cnt = a
            .iter()
            .filter(|a| matches!(a, ReceiverAction::MessageReady))
            .count();
        assert_eq!(cnt, 2);
    }

    #[test]
    fn progress_timeout_skips_gap() {
        let mut rx = make(cfg_progress(50));
        let mut a = ReceiverActions::new();

        let p0 = [0xAA; 10];
        let p2 = [0xCC; 10];

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(0),
                flags: SequenceFlag::Unsegmented,
                payload: &p0,
            },
            &mut a,
        )
        .unwrap();
        rx.take_message();

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(2),
                flags: SequenceFlag::Unsegmented,
                payload: &p2,
            },
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());

        rx.handle(ReceiverEvent::ProgressTimeout, &mut a)
            .unwrap();

        assert_eq!(rx.expected_seq().value(), 3);
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &p2);
    }

    #[test]
    fn gap_merge_split() {
        let mut rx = make(cfg_immediate());
        let mut a = ReceiverActions::new();

        let p = [0u8; 10];

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(0),
                flags: SequenceFlag::Unsegmented,
                payload: &p,
            },
            &mut a,
        )
        .unwrap();
        rx.take_message();

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(3),
                flags: SequenceFlag::Unsegmented,
                payload: &p,
            },
            &mut a,
        )
        .unwrap();

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(1),
                flags: SequenceFlag::Unsegmented,
                payload: &p,
            },
            &mut a,
        )
        .unwrap();
        assert!(rx.has_message());
        rx.take_message();

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(2),
                flags: SequenceFlag::Unsegmented,
                payload: &p,
            },
            &mut a,
        )
        .unwrap();

        let cnt = a
            .iter()
            .filter(|a| matches!(a, ReceiverAction::MessageReady))
            .count();
        assert_eq!(cnt, 2);
    }
}
