use crate::network::isl::address::Address;
use crate::network::spp::{SequenceCount, SequenceFlag};

use super::super::base::ReceiverBase;
use super::super::utils::{Bitset, BumpSlab};
use super::super::{
    ReceiverAction, ReceiverActions, ReceiverBackend, ReceiverConfig, ReceiverError,
};

/// Metadata for a single buffered out-of-order segment.
#[derive(Clone, Copy, Default)]
struct SlotMeta {
    /// Byte offset into the bump slab.
    offset: usize,
    /// Length of the stored payload in bytes.
    len: usize,
    /// SPP sequence flag for this segment.
    flags: SequenceFlag,
}

/// Packed backend — efficient when payloads are small.
///
/// Use when segments are typically much smaller than the MTU:
/// out-of-order payloads are bump-allocated at their actual
/// size, so `BUF` can be much smaller than `WIN × MTU`.
///
/// OOO insert is O(1) (bump append). Delivery copies each
/// buffered segment into the reassembly buffer — O(MSG) total.
///
/// Static memory: `BUF` (reorder slab) + `REASM` (reassembly).
///
/// * `WIN` — receive window (number of indexed slots)
/// * `BUF` — bump slab capacity in bytes
/// * `REASM` — reassembly buffer size
pub struct PackedReceiver<const WIN: usize, const BUF: usize, const REASM: usize> {
    /// Shared receiver state (sequence tracking, timers, ACK logic).
    base: ReceiverBase,
    /// Bitset tracking which window slots hold buffered segments.
    occupied: Bitset<WIN>,
    /// Per-slot metadata (offset, length, flags) for buffered segments.
    slot_meta: [SlotMeta; WIN],
    /// Append-only bump allocator storing out-of-order payloads.
    slab: BumpSlab<BUF>,
    /// Buffer for reassembling segmented messages.
    reassembly: [u8; REASM],
    /// Current write position in the reassembly buffer.
    reassembly_len: usize,
    /// Whether a multi-segment reassembly is in progress.
    reassembly_in_progress: bool,
    /// Length of a fully reassembled message, if one is ready.
    complete_message_len: Option<usize>,
}

impl<const WIN: usize, const BUF: usize, const REASM: usize> ReceiverBackend
    for PackedReceiver<WIN, BUF, REASM>
{
    fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self {
            base: ReceiverBase::new(config, remote_address),
            occupied: Bitset::new(),
            slot_meta: [SlotMeta::default(); WIN],
            slab: BumpSlab::new(),
            reassembly: [0u8; REASM],
            reassembly_len: 0,
            reassembly_in_progress: false,
            complete_message_len: None,
        }
    }

    fn remote_address(&self) -> Address {
        self.base.remote_address()
    }

    fn handle_data(
        &mut self,
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &[u8],
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {

        let distance = self.base.distance(seq);
        let seq_before = self.base.expected_seq_raw();

        if distance == 0 {
            self.deliver_packet(flags, payload)?;
            self.base.advance();
            self.deliver_buffered(actions)?;
            if self.complete_message_len.is_some() {
                actions.push(ReceiverAction::MessageReady);
            }
        } else if distance < Self::MAX_AHEAD {
            if !self.base.is_ooo_duplicate(distance) {
                self.store_ooo(seq.value(), flags, payload)?;
                self.base.record_ooo(distance);
            }
        }

        let progressed = self.base.expected_seq_raw() != seq_before;
        let has_gap = self.occupied.any();
        self.base
            .apply_post_data_logic(actions, progressed, has_gap);
        Ok(())
    }

    fn handle_ack(&mut self, actions: &mut ReceiverActions) {

        self.base.handle_ack_timeout(actions);
    }

    fn handle_timeout(&mut self, actions: &mut ReceiverActions) -> Result<(), ReceiverError> {


        if self.reassembly_in_progress {
            self.reassembly_len = 0;
            self.reassembly_in_progress = false;
        }

        self.base.advance();
        self.deliver_buffered(actions)?;

        if self.complete_message_len.is_some() {
            actions.push(ReceiverAction::MessageReady);
        }

        let has_gap = self.occupied.any();
        self.base.apply_post_progress_logic(actions, has_gap);
        Ok(())
    }

    fn take_message(&mut self) -> Option<&[u8]> {
        self.complete_message_len
            .take()
            .map(|len| &self.reassembly[..len])
    }

    fn reassembly_data(&self, len: usize) -> &[u8] {
        &self.reassembly[..len]
    }

    fn has_message(&self) -> bool {
        self.complete_message_len.is_some()
    }

    fn message_len(&self) -> Option<usize> {
        self.complete_message_len
    }

    fn consume_message<Ret>(&mut self, f: impl FnOnce(&[u8]) -> Ret) -> Option<Ret> {
        let len = self.complete_message_len.take()?;
        Some(f(&self.reassembly[..len]))
    }

    fn expected_seq(&self) -> SequenceCount {
        self.base.expected_seq()
    }
}

impl<const WIN: usize, const BUF: usize, const REASM: usize> PackedReceiver<WIN, BUF, REASM> {
    /// Maximum forward distance accepted for out-of-order packets.
    const MAX_AHEAD: u16 = WIN as u16;

    /// Map a raw sequence number to a window slot index.
    fn slot_idx(seq: u16) -> usize {
        seq as usize % WIN
    }

    /// Store an out-of-order segment in the slab and record its metadata.
    fn store_ooo(
        &mut self,
        seq: u16,
        flags: SequenceFlag,
        payload: &[u8],
    ) -> Result<(), ReceiverError> {
        let idx = Self::slot_idx(seq);
        if self.occupied.is_set(idx) {
            return Ok(());
        }
        let (offset, len) = self.slab.alloc(payload).ok_or(ReceiverError::BufferFull)?;
        self.slot_meta[idx] = SlotMeta { offset, len, flags };
        self.occupied.set(idx);
        Ok(())
    }

    /// Deliver consecutive buffered segments starting from the expected sequence.
    fn deliver_buffered(&mut self, actions: &mut ReceiverActions) -> Result<(), ReceiverError> {
        loop {
            let seq = self.base.expected_seq_raw();
            let idx = Self::slot_idx(seq);
            if !self.occupied.is_set(idx) {
                break;
            }

            let meta = self.slot_meta[idx];
            self.occupied.clear(idx);

            let mut temp = [0u8; REASM];
            let len = meta.len.min(REASM);
            temp[..len].copy_from_slice(self.slab.get(meta.offset, meta.len));
            self.deliver_packet(meta.flags, &temp[..len])?;
            self.base.advance();

            if self.complete_message_len.is_some() {
                actions.push(ReceiverAction::MessageReady);
            }
        }
        self.slab.clear();
        Ok(())
    }

    /// Append or complete a packet into the reassembly buffer based on its flags.
    fn deliver_packet(&mut self, flags: SequenceFlag, payload: &[u8]) -> Result<(), ReceiverError> {
        match flags {
            SequenceFlag::Unsegmented => {
                if payload.len() > REASM {
                    return Err(ReceiverError::MessageTooLarge);
                }
                self.reassembly[..payload.len()].copy_from_slice(payload);
                self.complete_message_len = Some(payload.len());
                self.reassembly_in_progress = false;
            }
            SequenceFlag::First => {
                if payload.len() > REASM {
                    return Err(ReceiverError::MessageTooLarge);
                }
                self.reassembly[..payload.len()].copy_from_slice(payload);
                self.reassembly_len = payload.len();
                self.reassembly_in_progress = true;
                self.complete_message_len = None;
            }
            SequenceFlag::Continuation | SequenceFlag::Last => {
                if !self.reassembly_in_progress {
                    return Err(ReceiverError::ReassemblyError);
                }
                let new_len = self.reassembly_len + payload.len();
                if new_len > REASM {
                    return Err(ReceiverError::MessageTooLarge);
                }
                self.reassembly[self.reassembly_len..new_len].copy_from_slice(payload);
                self.reassembly_len = new_len;

                if flags == SequenceFlag::Last {
                    self.complete_message_len = Some(self.reassembly_len);
                    self.reassembly_in_progress = false;
                }
            }
        }
        Ok(())
    }
}
