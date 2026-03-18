use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;

use super::super::HandleResult;
use super::super::ReceiverBackend;
use super::super::ReceiverConfig;
use super::super::ReceiverError;
use super::super::base::ReceiverBase;
use super::super::utils::Bitset;
use super::super::utils::SlotMap;

/// Fastest backend — O(1) insert and O(1) delivery.
///
/// Use when CPU budget is tight and you can afford the extra
/// memory: both buffering an out-of-order segment and delivering
/// a run of consecutive segments are constant-time per packet.
///
/// Stores out-of-order segments in `WIN` fixed MTU-sized slots
/// indexed by `seq % WIN`. Each slot reserves a full MTU even
/// for shorter payloads.
///
/// Static memory: `WIN × MTU` (reorder) + `REASM` (reassembly).
///
/// * `WIN` — receive window (number of slots)
/// * `MTU` — maximum segment payload size
/// * `REASM` — reassembly buffer size
/// * `TOTAL` — total slot storage (`WIN * MTU`)
pub struct FastReceiver<const WIN: usize, const MTU: usize, const REASM: usize, const TOTAL: usize>
{
    /// Shared receiver state (sequence tracking, timers, ACK logic).
    base: ReceiverBase,
    /// Bitset tracking which window slots hold buffered segments.
    occupied: Bitset<WIN>,
    /// Fixed-size slot storage for out-of-order payloads.
    slots: SlotMap<TOTAL, WIN, MTU>,
    /// Per-slot sequence flags for buffered segments.
    flags: [SequenceFlag; WIN],
    /// Buffer for reassembling segmented messages.
    reassembly: [u8; REASM],
    /// Current write position in the reassembly buffer.
    reassembly_len: usize,
    /// Whether a multi-segment reassembly is in progress.
    reassembly_in_progress: bool,
    /// Length of a fully reassembled message, if one is ready.
    complete_message_len: Option<usize>,
}

impl<const WIN: usize, const MTU: usize, const REASM: usize, const TOTAL: usize> ReceiverBackend
    for FastReceiver<WIN, MTU, REASM, TOTAL>
{
    fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self {
            base: ReceiverBase::new(config, remote_address),
            occupied: Bitset::new(),
            slots: SlotMap::new(),
            flags: [SequenceFlag::default(); WIN],
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
    ) -> Result<HandleResult, ReceiverError> {
        let distance = self.base.distance(seq);
        let seq_before = self.base.expected_seq_raw();

        if distance == 0 {
            self.deliver_packet(flags, payload)?;
            self.base.advance();
            self.deliver_buffered()?;
        } else if distance < Self::MAX_AHEAD {
            if !self.base.is_ooo_duplicate(distance) {
                self.store_ooo(seq.value(), flags, payload);
                self.base.record_ooo(distance);
            }
        }

        let progressed = self.base.expected_seq_raw() != seq_before;
        let has_gap = self.occupied.any();
        Ok(self.base.apply_post_data_logic(progressed, has_gap))
    }

    fn handle_ack(&mut self) -> HandleResult {
        self.base.handle_ack_timeout()
    }

    fn handle_timeout(&mut self) -> Result<HandleResult, ReceiverError> {
        if self.reassembly_in_progress {
            self.reassembly_len = 0;
            self.reassembly_in_progress = false;
        }

        self.base.advance();
        self.deliver_buffered()?;

        let has_gap = self.occupied.any();
        Ok(self.base.apply_post_progress_logic(has_gap))
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

impl<const WIN: usize, const MTU: usize, const REASM: usize, const TOTAL: usize>
    FastReceiver<WIN, MTU, REASM, TOTAL>
{
    /// Maximum forward distance accepted for out-of-order packets.
    const MAX_AHEAD: u16 = WIN as u16;

    /// Map a raw sequence number to a window slot index.
    fn slot_idx(seq: u16) -> usize {
        seq as usize % WIN
    }

    /// Store an out-of-order segment into a fixed slot.
    fn store_ooo(&mut self, seq: u16, flags: SequenceFlag, payload: &[u8]) {
        let idx = Self::slot_idx(seq);
        if self.occupied.is_set(idx) {
            return;
        }
        self.slots.write(idx, payload);
        self.flags[idx] = flags;
        self.occupied.set(idx);
    }

    /// Deliver consecutive buffered segments starting from the expected sequence.
    fn deliver_buffered(&mut self) -> Result<(), ReceiverError> {
        let mut temp = [0u8; MTU];
        loop {
            let seq = self.base.expected_seq_raw();
            let idx = Self::slot_idx(seq);
            if !self.occupied.is_set(idx) {
                break;
            }

            let flags = self.flags[idx];
            self.occupied.clear(idx);

            let len = self.slots.read(idx, &mut temp);
            self.deliver_packet(flags, &temp[..len])?;
            self.base.advance();
        }
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
