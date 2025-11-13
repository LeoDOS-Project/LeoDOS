//! Receiver state machine for SRSP.
//!
//! Handles reordering, reassembly, and ACK generation.
//! Completely synchronous - no I/O, no async.

use crate::network::spp::{Apid, SequenceCount, SequenceFlag};
use heapless::Vec;

/// Maximum number of actions that can be emitted per event.
const MAX_ACTIONS: usize = 32;

/// Events that drive the receiver state machine.
#[derive(Debug)]
pub enum ReceiverEvent<'a> {
    /// A data packet was received.
    DataReceived {
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &'a [u8],
    },

    /// ACK delay timer expired.
    AckTimeout,
}

/// Actions the receiver machine wants the driver to perform.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ReceiverAction {
    /// Send an ACK packet.
    SendAck {
        cumulative_ack: SequenceCount,
        selective_bitmap: u16,
    },

    /// Start ACK delay timer.
    StartAckTimer { ticks: u32 },

    /// Stop ACK delay timer.
    StopAckTimer,

    /// A complete message is ready. Call `take_message()` to retrieve it.
    MessageReady,
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
pub struct ReceiverConfig {
    /// APID to expect for incoming packets.
    pub apid: Apid,
    /// Whether to send ACKs immediately or delay them.
    pub immediate_ack: bool,
    /// ACK delay in ticks (if not immediate).
    pub ack_delay_ticks: u32,
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

    /// Create a new receiver state machine.
    pub fn new(config: ReceiverConfig) -> Self {
        Self {
            config,
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

        if distance == 0 {
            // In order - deliver immediately
            self.deliver_packet(flags, payload)?;
            self.advance();

            // Check for buffered packets we can now deliver
            self.deliver_buffered(actions)?;

            // Emit message ready if complete
            if self.complete_message_len.is_some() {
                actions.push(ReceiverAction::MessageReady);
            }
        } else if distance < Self::MAX_AHEAD {
            // Out of order but within window - buffer it
            let bit_pos = distance - 1;
            let mask = 1u16 << bit_pos;

            if self.recv_bitmap & mask == 0 {
                // Not a duplicate
                self.store_out_of_order(seq_val, flags, payload)?;
                self.recv_bitmap |= mask;
            }
        } else if distance > SequenceCount::MAX - Self::MAX_AHEAD {
            // Behind expected - duplicate, ignore
        }
        // else: too far ahead, ignore

        // Handle ACK
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

    fn emit_ack(&mut self, actions: &mut ReceiverActions) {
        let cumulative = self.expected_seq.wrapping_sub(1) & SequenceCount::MAX;

        // Stop timer if running
        if self.ack_timer_running {
            actions.push(ReceiverAction::StopAckTimer);
            self.ack_timer_running = false;
        }

        actions.push(ReceiverAction::SendAck {
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
            // Check bitmap bit 0
            if self.recv_bitmap & 1 == 0 {
                break;
            }

            // Find packet with expected_seq
            let found_idx = self
                .reorder_meta
                .iter()
                .position(|m| m.occupied && m.seq == self.expected_seq);

            if let Some(idx) = found_idx {
                let meta = self.reorder_meta[idx];
                self.reorder_meta[idx].occupied = false;

                // Copy payload to avoid borrow issues
                let mut temp = [0u8; REASM];
                let len = meta.len.min(REASM);
                temp[..len].copy_from_slice(&self.reorder_data[meta.offset..meta.offset + len]);

                self.deliver_packet(meta.flags, &temp[..len])?;
                self.advance();

                // Emit message ready if complete
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

    fn make_config() -> ReceiverConfig {
        ReceiverConfig {
            apid: Apid::new(0x42).unwrap(),
            immediate_ack: true,
            ack_delay_ticks: 20,
        }
    }

    fn make_delayed_config() -> ReceiverConfig {
        ReceiverConfig {
            apid: Apid::new(0x42).unwrap(),
            immediate_ack: false,
            ack_delay_ticks: 20,
        }
    }

    #[test]
    fn test_immediate_ack() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config());
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
            ReceiverMachine::new(make_delayed_config());
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
            ReceiverMachine::new(make_delayed_config());
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
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config());
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
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config());
        let mut actions = ReceiverActions::new();

        // Receive packet 1 first (out of order)
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

        // Check selective ACK bitmap in the ACK
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

        // Now receive packet 0
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

        // Should get both messages (two MessageReady actions)
        let message_count = actions
            .iter()
            .filter(|a| matches!(a, ReceiverAction::MessageReady))
            .count();
        assert_eq!(message_count, 2);

        assert_eq!(receiver.take_message().unwrap(), &[1]);
        assert_eq!(receiver.take_message().unwrap(), &[2]);
    }

    #[test]
    fn test_segmented_message() {
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config());
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
        let mut receiver: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new(make_config());
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
}
