//! Receiver state machine for SRSP.
//!
//! Handles reordering, reassembly, and ACK generation.
//! Completely synchronous - no I/O, no async.

use crate::network::isl::address::Address;
use crate::network::spp::{Apid, SequenceCount, SequenceFlag};
use heapless::Vec;

/// Maximum number of actions that can be emitted per event.
const MAX_ACTIONS: usize = 32;

/// Events that drive the receiver state machine.
#[derive(Debug)]
pub enum ReceiverEvent<'a> {
    /// A data packet was received.
    DataReceived {
        /// Sequence number of the received packet.
        seq: SequenceCount,
        /// Segmentation flags of the received packet.
        flags: SequenceFlag,
        /// Payload data of the received packet.
        payload: &'a [u8],
    },

    /// ACK delay timer expired.
    AckTimeout,

    /// Progress timer expired — `expected_seq` hasn't advanced.
    ProgressTimeout,
}

/// Actions the receiver machine wants the driver to perform.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ReceiverAction {
    /// Send an ACK packet to a specific destination.
    SendAck {
        /// Address to send the ACK to.
        destination: Address,
        /// Highest contiguously received sequence number.
        cumulative_ack: SequenceCount,
        /// Bitmap of selectively acknowledged packets beyond the cumulative ACK.
        selective_bitmap: u16,
    },

    /// Start ACK delay timer.
    StartAckTimer {
        /// Timer duration in ticks.
        ticks: u32,
    },

    /// Stop ACK delay timer.
    StopAckTimer,

    /// A complete message is ready. Call `take_message()` to retrieve it.
    MessageReady,

    /// Start progress timer (gap detected).
    StartProgressTimer {
        /// Timer duration in ticks.
        ticks: u32,
    },

    /// Stop progress timer (progress made).
    StopProgressTimer,
}

/// Collection of actions emitted by the receiver.
#[derive(Debug)]
pub struct ReceiverActions {
    inner: Vec<ReceiverAction, MAX_ACTIONS>,
}

impl ReceiverActions {
    /// Create a new empty actions collection.
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Add an action to the collection.
    pub fn push(&mut self, action: ReceiverAction) {
        let _ = self.inner.push(action);
    }

    /// Clear all actions.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterate over the actions.
    pub fn iter(&self) -> impl Iterator<Item = &ReceiverAction> {
        self.inner.iter()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Number of actions.
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Default for ReceiverActions {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a ReceiverActions {
    type Item = &'a ReceiverAction;
    type IntoIter = core::slice::Iter<'a, ReceiverAction>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

/// Error from receiver operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum ReceiverError {
    /// Reorder buffer full.
    #[error("Reorder buffer full")]
    BufferFull,
    /// Message too large for reassembly buffer.
    #[error("Message too large for reassembly buffer")]
    MessageTooLarge,
    /// Reassembly error (e.g., continuation without first).
    #[error("Reassembly error")]
    ReassemblyError,
}

/// Configuration for the receiver.
#[derive(Debug, Clone)]
#[derive(bon::Builder)]
pub struct ReceiverConfig {
    /// Local address of this receiver.
    pub local_address: Address,
    /// APID filter for incoming packets.
    pub apid: Apid,
    /// cFE function code for outgoing ACK packets.
    pub function_code: u8,
    /// ISL routing message ID for outgoing ACK packets.
    pub message_id: u8,
    /// ISL routing action code for outgoing ACK packets.
    pub action_code: u8,
    /// If true, send ACKs immediately; otherwise use delayed ACKs.
    pub immediate_ack: bool,
    /// Delayed ACK timer duration in ticks.
    pub ack_delay_ticks: u32,
    /// Progress timeout in ticks; `None` disables gap-skipping.
    pub progress_timeout_ticks: Option<u32>,
}

/// Metadata for a packet in the reorder buffer.
#[derive(Clone, Copy, Default)]
struct ReorderMeta {
    occupied: bool,
    seq: u16,
    flags: SequenceFlag,
    offset: usize,
    len: usize,
}

/// Receiver state machine.
///
/// Handles reordering, reassembly, and ACK generation.
/// Completely synchronous - no I/O, no async.
///
/// # Type Parameters
///
/// * `WIN` - Maximum number of out-of-order packets to buffer
/// * `BUF` - Total reorder buffer size in bytes
/// * `REASM` - Maximum reassembled message size
pub struct ReceiverMachine<const WIN: usize, const BUF: usize, const REASM: usize> {
    config: ReceiverConfig,
    remote_address: Address,

    // Sequence tracking
    expected_seq: u16,
    recv_bitmap: u16,

    // Reorder buffer
    reorder_meta: [ReorderMeta; WIN],
    reorder_data: [u8; BUF],
    reorder_write_pos: usize,

    // Reassembly buffer
    reassembly: [u8; REASM],
    reassembly_len: usize,
    reassembly_in_progress: bool,

    // Complete message
    complete_message_len: Option<usize>,

    // ACK state
    ack_pending: bool,
    ack_timer_running: bool,
}

impl<const WIN: usize, const BUF: usize, const REASM: usize> ReceiverMachine<WIN, BUF, REASM> {
    /// Maximum window ahead distance.
    const MAX_AHEAD: u16 = 16;

    /// Create a new receiver state machine for a specific remote sender.
    pub fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self {
            config,
            remote_address,
            expected_seq: 0,
            recv_bitmap: 0,
            reorder_meta: [ReorderMeta::default(); WIN],
            reorder_data: [0u8; BUF],
            reorder_write_pos: 0,
            reassembly: [0u8; REASM],
            reassembly_len: 0,
            reassembly_in_progress: false,
            complete_message_len: None,
            ack_pending: false,
            ack_timer_running: false,
        }
    }

    /// Get the remote address this machine is receiving from.
    pub fn remote_address(&self) -> Address {
        self.remote_address
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
            } => {
                self.handle_data(seq, flags, payload, actions)?;
            }
            ReceiverEvent::AckTimeout => {
                self.handle_ack_timeout(actions);
            }
            ReceiverEvent::ProgressTimeout => {
                self.handle_progress_timeout(actions)?;
            }
        }
        Ok(())
    }

    /// Take the complete message.
    ///
    /// Call this after receiving a `MessageReady` action.
    /// Returns `None` if no complete message is available.
    pub fn take_message(&mut self) -> Option<&[u8]> {
        self.complete_message_len
            .take()
            .map(|len| &self.reassembly[..len])
    }

    /// Check if there's a complete message ready.
    pub fn has_message(&self) -> bool {
        self.complete_message_len.is_some()
    }

    /// Get the current expected sequence number.
    pub fn expected_seq(&self) -> SequenceCount {
        SequenceCount::from(self.expected_seq)
    }

    fn handle_data(
        &mut self,
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &[u8],
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        let seq_val = seq.value();
        let distance = seq_val.wrapping_sub(self.expected_seq) & SequenceCount::MAX;
        let seq_before = self.expected_seq;

        if distance == 0 {
            self.deliver_packet(flags, payload)?;
            self.advance();

            self.deliver_buffered(actions)?;

            if self.complete_message_len.is_some() {
                actions.push(ReceiverAction::MessageReady);
            }
        } else if distance < Self::MAX_AHEAD {
            let bit_pos = distance - 1;
            let mask = 1u16 << bit_pos;

            if self.recv_bitmap & mask == 0 {
                self.store_out_of_order(seq_val, flags, payload)?;
                self.recv_bitmap |= mask;
            }
        } else if distance > SequenceCount::MAX - Self::MAX_AHEAD {
        }

        let progressed = self.expected_seq != seq_before;

        if let Some(ticks) = self.config.progress_timeout_ticks {
            if progressed {
                actions.push(ReceiverAction::StopProgressTimer);
                let has_gap = self.reorder_meta.iter().any(|m| m.occupied);
                if has_gap {
                    actions.push(ReceiverAction::StartProgressTimer { ticks });
                }
            } else {
                let has_gap = self.reorder_meta.iter().any(|m| m.occupied);
                if has_gap {
                    actions.push(ReceiverAction::StartProgressTimer { ticks });
                }
            }
        }

        self.ack_pending = true;

        if self.config.immediate_ack {
            self.emit_ack(actions);
        } else if !self.ack_timer_running {
            actions.push(ReceiverAction::StartAckTimer {
                ticks: self.config.ack_delay_ticks,
            });
            self.ack_timer_running = true;
        }

        Ok(())
    }

    fn handle_ack_timeout(&mut self, actions: &mut ReceiverActions) {
        self.ack_timer_running = false;
        if self.ack_pending {
            self.emit_ack(actions);
        }
    }

    fn handle_progress_timeout(
        &mut self,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        if self.reassembly_in_progress {
            self.reassembly_len = 0;
            self.reassembly_in_progress = false;
        }

        self.advance();

        self.deliver_buffered(actions)?;

        if self.complete_message_len.is_some() {
            actions.push(ReceiverAction::MessageReady);
        }

        let has_gap = self
            .reorder_meta
            .iter()
            .any(|m| m.occupied);
        if has_gap {
            if let Some(ticks) = self.config.progress_timeout_ticks {
                actions.push(ReceiverAction::StartProgressTimer { ticks });
            }
        } else {
            actions.push(ReceiverAction::StopProgressTimer);
        }

        Ok(())
    }

    fn emit_ack(&mut self, actions: &mut ReceiverActions) {
        let cumulative = self.expected_seq.wrapping_sub(1) & SequenceCount::MAX;

        // Stop timer if running
        if self.ack_timer_running {
            actions.push(ReceiverAction::StopAckTimer);
            self.ack_timer_running = false;
        }

        actions.push(ReceiverAction::SendAck {
            destination: self.remote_address,
            cumulative_ack: SequenceCount::from(cumulative),
            selective_bitmap: self.recv_bitmap,
        });
        self.ack_pending = false;
    }

    fn advance(&mut self) {
        self.expected_seq = (self.expected_seq + 1) & SequenceCount::MAX;
        self.recv_bitmap >>= 1;
    }

    fn store_out_of_order(
        &mut self,
        seq: u16,
        flags: SequenceFlag,
        payload: &[u8],
    ) -> Result<(), ReceiverError> {
        // Find free slot
        let slot_idx = self
            .reorder_meta
            .iter()
            .position(|m| !m.occupied)
            .ok_or(ReceiverError::BufferFull)?;

        // Check space
        if payload.len() > BUF - self.reorder_write_pos {
            self.compact_reorder();
            if payload.len() > BUF - self.reorder_write_pos {
                return Err(ReceiverError::BufferFull);
            }
        }

        let offset = self.reorder_write_pos;
        self.reorder_data[offset..offset + payload.len()].copy_from_slice(payload);
        self.reorder_write_pos += payload.len();

        self.reorder_meta[slot_idx] = ReorderMeta {
            occupied: true,
            seq,
            flags,
            offset,
            len: payload.len(),
        };

        Ok(())
    }

    fn deliver_buffered(&mut self, actions: &mut ReceiverActions) -> Result<(), ReceiverError> {
        loop {
            let found_idx = self
                .reorder_meta
                .iter()
                .position(|m| m.occupied && m.seq == self.expected_seq);

            if let Some(idx) = found_idx {
                let meta = self.reorder_meta[idx];
                self.reorder_meta[idx].occupied = false;

                let mut temp = [0u8; REASM];
                let len = meta.len.min(REASM);
                temp[..len].copy_from_slice(&self.reorder_data[meta.offset..meta.offset + len]);

                self.deliver_packet(meta.flags, &temp[..len])?;
                self.advance();

                if self.complete_message_len.is_some() {
                    actions.push(ReceiverAction::MessageReady);
                }
            } else {
                break;
            }
        }

        Ok(())
    }

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

    fn compact_reorder(&mut self) {
        let mut indices: [Option<usize>; WIN] = [None; WIN];
        let mut count = 0;

        for (i, m) in self.reorder_meta.iter().enumerate() {
            if m.occupied {
                let offset = m.offset;
                let mut insert_pos = count;
                for j in 0..count {
                    if let Some(idx) = indices[j] {
                        if self.reorder_meta[idx].offset > offset {
                            insert_pos = j;
                            break;
                        }
                    }
                }
                for j in (insert_pos..count).rev() {
                    indices[j + 1] = indices[j];
                }
                indices[insert_pos] = Some(i);
                count += 1;
            }
        }

        if count == 0 {
            self.reorder_write_pos = 0;
            return;
        }

        let mut new_pos = 0usize;
        for idx_opt in indices.iter().take(count) {
            if let Some(idx) = *idx_opt {
                let old_offset = self.reorder_meta[idx].offset;
                let len = self.reorder_meta[idx].len;

                if new_pos != old_offset {
                    self.reorder_data
                        .copy_within(old_offset..old_offset + len, new_pos);
                }

                self.reorder_meta[idx].offset = new_pos;
                new_pos += len;
            }
        }

        self.reorder_write_pos = new_pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_local_address() -> Address {
        Address::satellite(1, 1)
    }

    fn test_remote_address() -> Address {
        Address::satellite(1, 2)
    }

    fn make_config() -> ReceiverConfig {
        ReceiverConfig {
            local_address: test_local_address(),
            apid: Apid::new(0x42).unwrap(),
            function_code: 0,
            message_id: 0,
            action_code: 0,
            immediate_ack: true,
            ack_delay_ticks: 20,
            progress_timeout_ticks: None,
        }
    }

    fn make_delayed_config() -> ReceiverConfig {
        ReceiverConfig {
            local_address: test_local_address(),
            apid: Apid::new(0x42).unwrap(),
            function_code: 0,
            message_id: 0,
            action_code: 0,
            immediate_ack: false,
            ack_delay_ticks: 20,
            progress_timeout_ticks: None,
        }
    }

    #[test]
    fn test_immediate_ack() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config(), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1, 2, 3],
                },
                &mut actions,
            )
            .unwrap();

        assert!(
            actions
                .iter()
                .any(|a| matches!(a, ReceiverAction::SendAck { .. }))
        );
    }

    #[test]
    fn test_delayed_ack_starts_timer() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_delayed_config(), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1, 2, 3],
                },
                &mut actions,
            )
            .unwrap();

        assert!(
            actions
                .iter()
                .any(|a| matches!(a, ReceiverAction::StartAckTimer { ticks: 20 }))
        );
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, ReceiverAction::SendAck { .. }))
        );
    }

    #[test]
    fn test_ack_timeout_sends_ack() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_delayed_config(), test_remote_address());
        let mut actions = ReceiverActions::new();

        // Receive data - starts timer
        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1, 2, 3],
                },
                &mut actions,
            )
            .unwrap();

        // Timeout - should send ACK
        receiver
            .handle(ReceiverEvent::AckTimeout, &mut actions)
            .unwrap();

        assert!(
            actions
                .iter()
                .any(|a| matches!(a, ReceiverAction::SendAck { .. }))
        );
    }

    #[test]
    fn test_receive_single_packet() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config(), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1, 2, 3, 4, 5],
                },
                &mut actions,
            )
            .unwrap();

        assert!(
            actions
                .iter()
                .any(|a| matches!(a, ReceiverAction::MessageReady))
        );
        assert_eq!(receiver.take_message().unwrap(), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_out_of_order_delivery() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_config(), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(1),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[2],
                },
                &mut actions,
            )
            .unwrap();

        assert!(!receiver.has_message());

        let ack = actions.iter().find_map(|a| {
            if let ReceiverAction::SendAck {
                selective_bitmap, ..
            } = a
            {
                Some(*selective_bitmap)
            } else {
                None
            }
        });
        assert_eq!(ack, Some(0b0001));

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1],
                },
                &mut actions,
            )
            .unwrap();

        let message_count = actions
            .iter()
            .filter(|a| matches!(a, ReceiverAction::MessageReady))
            .count();
        assert_eq!(message_count, 2);

        assert!(receiver.has_message());
    }

    #[test]
    fn test_segmented_message() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config(), test_remote_address());
        let mut actions = ReceiverActions::new();

        // First segment
        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::First,
                    payload: &[1, 2, 3],
                },
                &mut actions,
            )
            .unwrap();
        assert!(!receiver.has_message());

        // Continuation
        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(1),
                    flags: SequenceFlag::Continuation,
                    payload: &[4, 5, 6],
                },
                &mut actions,
            )
            .unwrap();
        assert!(!receiver.has_message());

        // Last segment
        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(2),
                    flags: SequenceFlag::Last,
                    payload: &[7, 8],
                },
                &mut actions,
            )
            .unwrap();

        assert!(receiver.has_message());
        assert_eq!(receiver.take_message().unwrap(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_duplicate_ignored() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config(), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1],
                },
                &mut actions,
            )
            .unwrap();

        receiver.take_message(); // Clear

        // Duplicate
        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[99],
                },
                &mut actions,
            )
            .unwrap();

        assert!(!receiver.has_message());
    }

    fn make_progress_config(ticks: u32) -> ReceiverConfig {
        ReceiverConfig {
            local_address: test_local_address(),
            apid: Apid::new(0x42).unwrap(),
            function_code: 0,
            message_id: 0,
            action_code: 0,
            immediate_ack: true,
            ack_delay_ticks: 20,
            progress_timeout_ticks: Some(ticks),
        }
    }

    #[test]
    fn test_progress_timeout_skips_gap() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_progress_config(50), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1],
                },
                &mut actions,
            )
            .unwrap();
        receiver.take_message();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(2),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[3],
                },
                &mut actions,
            )
            .unwrap();
        assert!(!receiver.has_message());

        receiver
            .handle(ReceiverEvent::ProgressTimeout, &mut actions)
            .unwrap();

        assert_eq!(receiver.expected_seq().value(), 3);
        assert!(receiver.has_message());
        assert_eq!(receiver.take_message().unwrap(), &[3]);
    }

    #[test]
    fn test_progress_timeout_discards_partial_reassembly() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_progress_config(50), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::First,
                    payload: &[1, 2, 3],
                },
                &mut actions,
            )
            .unwrap();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(3),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[10, 11],
                },
                &mut actions,
            )
            .unwrap();
        assert!(!receiver.has_message());

        receiver
            .handle(ReceiverEvent::ProgressTimeout, &mut actions)
            .unwrap();
        assert!(!receiver.has_message());

        receiver
            .handle(ReceiverEvent::ProgressTimeout, &mut actions)
            .unwrap();
        assert!(receiver.has_message());
        assert_eq!(receiver.take_message().unwrap(), &[10, 11]);

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(4),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[20, 21],
                },
                &mut actions,
            )
            .unwrap();
        assert!(receiver.has_message());
        assert_eq!(receiver.take_message().unwrap(), &[20, 21]);
    }

    #[test]
    fn test_no_progress_timeout_in_reliable_mode() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_config(), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1],
                },
                &mut actions,
            )
            .unwrap();
        receiver.take_message();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(2),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[3],
                },
                &mut actions,
            )
            .unwrap();

        assert!(!actions.iter().any(|a| matches!(
            a,
            ReceiverAction::StartProgressTimer { .. }
        )));
    }

    #[test]
    fn test_progress_timer_resets_on_progress() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_progress_config(50), test_remote_address());
        let mut actions = ReceiverActions::new();

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(1),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[2],
                },
                &mut actions,
            )
            .unwrap();

        assert!(actions.iter().any(|a| matches!(
            a,
            ReceiverAction::StartProgressTimer { ticks: 50 }
        )));

        receiver
            .handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(0),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[1],
                },
                &mut actions,
            )
            .unwrap();

        assert!(actions
            .iter()
            .any(|a| matches!(a, ReceiverAction::StopProgressTimer)));
    }
}
