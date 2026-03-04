use crate::network::isl::address::Address;
use crate::network::spp::{SequenceCount, SequenceFlag};

use super::primitives::{Bitset, SlotMap};
use super::shell::ReceiverShell;
use super::{
    ReceiverAction, ReceiverActions, ReceiverConfig, ReceiverError,
    ReceiverEvent,
};

/// Backend A: indexed slots with fixed MTU-sized payloads.
///
/// * `WIN` — receive window (number of slots)
/// * `MTU` — maximum segment payload size
/// * `REASM` — reassembly buffer size
/// * `TOTAL` — total slot storage (`WIN * MTU`)
pub struct ReceiverA<
    const WIN: usize,
    const MTU: usize,
    const REASM: usize,
    const TOTAL: usize,
> {
    shell: ReceiverShell,
    occupied: Bitset<WIN>,
    slots: SlotMap<TOTAL, WIN, MTU>,
    flags: [SequenceFlag; WIN],
    reassembly: [u8; REASM],
    reassembly_len: usize,
    reassembly_in_progress: bool,
    complete_message_len: Option<usize>,
}

impl<const WIN: usize, const MTU: usize, const REASM: usize, const TOTAL: usize>
    ReceiverA<WIN, MTU, REASM, TOTAL>
{
    const MAX_AHEAD: u16 = WIN as u16;

    /// Create a new receiver for a specific remote sender.
    pub fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self {
            shell: ReceiverShell::new(config, remote_address),
            occupied: Bitset::new(),
            slots: SlotMap::new(),
            flags: [SequenceFlag::default(); WIN],
            reassembly: [0u8; REASM],
            reassembly_len: 0,
            reassembly_in_progress: false,
            complete_message_len: None,
        }
    }

    /// Get the remote address.
    pub fn remote_address(&self) -> Address {
        self.shell.remote_address()
    }

    /// Process an event and produce actions.
    pub fn handle(
        &mut self,
        event: ReceiverEvent<'_>,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        actions.clear();
        match event {
            ReceiverEvent::DataReceived {
                seq,
                flags,
                payload,
            } => self.handle_data(seq, flags, payload, actions),
            ReceiverEvent::AckTimeout => {
                self.shell.handle_ack_timeout(actions);
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
            .map(|len| &self.reassembly[..len])
    }

    /// Returns a slice of the reassembly buffer.
    pub fn reassembly_data(&self, len: usize) -> &[u8] {
        &self.reassembly[..len]
    }

    /// Check if there.s a complete message ready.
    pub fn has_message(&self) -> bool {
        self.complete_message_len.is_some()
    }

    /// Get the current expected sequence number.
    pub fn expected_seq(&self) -> SequenceCount {
        self.shell.expected_seq()
    }

    fn slot_idx(seq: u16) -> usize {
        seq as usize % WIN
    }

    fn handle_data(
        &mut self,
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &[u8],
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        let distance = self.shell.distance(seq);
        let seq_before = self.shell.expected_seq_raw();

        if distance == 0 {
            self.deliver_packet(flags, payload)?;
            self.shell.advance();
            self.deliver_buffered(actions)?;
            if self.complete_message_len.is_some() {
                actions.push(ReceiverAction::MessageReady);
            }
        } else if distance < Self::MAX_AHEAD {
            if !self.shell.is_ooo_duplicate(distance) {
                self.store_ooo(seq.value(), flags, payload);
                self.shell.record_ooo(distance);
            }
        }

        let progressed =
            self.shell.expected_seq_raw() != seq_before;
        let has_gap = self.occupied.any();
        self.shell
            .apply_post_data_logic(actions, progressed, has_gap);
        Ok(())
    }

    fn handle_progress_timeout(
        &mut self,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        if self.reassembly_in_progress {
            self.reassembly_len = 0;
            self.reassembly_in_progress = false;
        }

        self.shell.advance();
        self.deliver_buffered(actions)?;

        if self.complete_message_len.is_some() {
            actions.push(ReceiverAction::MessageReady);
        }

        let has_gap = self.occupied.any();
        self.shell
            .apply_post_progress_logic(actions, has_gap);
        Ok(())
    }

    fn store_ooo(
        &mut self,
        seq: u16,
        flags: SequenceFlag,
        payload: &[u8],
    ) {
        let idx = Self::slot_idx(seq);
        if self.occupied.is_set(idx) {
            return;
        }
        self.slots.write(idx, payload);
        self.flags[idx] = flags;
        self.occupied.set(idx);
    }

    fn deliver_buffered(
        &mut self,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        let mut temp = [0u8; MTU];
        loop {
            let seq = self.shell.expected_seq_raw();
            let idx = Self::slot_idx(seq);
            if !self.occupied.is_set(idx) {
                break;
            }

            let flags = self.flags[idx];
            self.occupied.clear(idx);

            let len = self.slots.read(idx, &mut temp);
            self.deliver_packet(flags, &temp[..len])?;
            self.shell.advance();

            if self.complete_message_len.is_some() {
                actions.push(ReceiverAction::MessageReady);
            }
        }
        Ok(())
    }

    fn deliver_packet(
        &mut self,
        flags: SequenceFlag,
        payload: &[u8],
    ) -> Result<(), ReceiverError> {
        match flags {
            SequenceFlag::Unsegmented => {
                if payload.len() > REASM {
                    return Err(ReceiverError::MessageTooLarge);
                }
                self.reassembly[..payload.len()]
                    .copy_from_slice(payload);
                self.complete_message_len = Some(payload.len());
                self.reassembly_in_progress = false;
            }
            SequenceFlag::First => {
                if payload.len() > REASM {
                    return Err(ReceiverError::MessageTooLarge);
                }
                self.reassembly[..payload.len()]
                    .copy_from_slice(payload);
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
                self.reassembly[self.reassembly_len..new_len]
                    .copy_from_slice(payload);
                self.reassembly_len = new_len;

                if flags == SequenceFlag::Last {
                    self.complete_message_len =
                        Some(self.reassembly_len);
                    self.reassembly_in_progress = false;
                }
            }
        }
        Ok(())
    }
}
