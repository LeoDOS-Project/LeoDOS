//! Sender state machine for SRSPP.
//!
//! Handles segmentation, buffering, and retransmission of messages.
//! Completely synchronous - no I/O, no async.

use crate::network::isl::address::{Address, RawAddress};
use crate::network::spp::{Apid, SequenceCount, SequenceFlag};
use heapless::Vec;

/// Maximum number of actions that can be emitted per event.
const MAX_ACTIONS: usize = 32;

/// Events that drive the sender state machine.
#[derive(Debug)]
pub enum SenderEvent<'a> {
    /// Application wants to send data.
    SendRequest {
        /// Destination address.
        target: Address,
        /// Data to send.
        data: &'a [u8],
    },

    /// An ACK packet was received from the remote.
    AckReceived {
        /// Highest contiguously acknowledged sequence number.
        cumulative_ack: SequenceCount,
        /// Bitmap of selectively acknowledged packets.
        selective_bitmap: u16,
    },

    /// A retransmission timer expired for a specific packet.
    RetransmitTimeout {
        /// Sequence number of the timed-out packet.
        seq: SequenceCount,
    },
}

/// Actions the sender machine wants the driver to perform.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SenderAction {
    /// Transmit a packet and start its retransmission timer.
    /// Call `get_payload(seq)` to get the payload data.
    Transmit {
        /// Sequence number to transmit.
        seq: SequenceCount,
        /// Retransmission timeout in ticks.
        rto_ticks: u32,
    },

    /// Stop a retransmission timer.
    StopTimer {
        /// Sequence number whose timer should be stopped.
        seq: SequenceCount,
    },

    /// A packet was permanently lost (max retransmits exceeded).
    PacketLost {
        /// Sequence number of the lost packet.
        seq: SequenceCount,
    },

    /// A segmented message was lost (a packet from it was permanently lost).
    MessageLost,

    /// Send buffer has space available (for backpressure signaling).
    SpaceAvailable {
        /// Number of bytes available.
        bytes: usize,
    },
}

/// Collection of actions emitted by the sender.
#[derive(Debug)]
pub struct SenderActions {
    inner: Vec<SenderAction, MAX_ACTIONS>,
}

impl SenderActions {
    /// Create a new empty actions collection.
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Add an action to the collection.
    pub fn push(&mut self, action: SenderAction) {
        let _ = self.inner.push(action);
    }

    /// Clear all actions.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterate over the actions.
    pub fn iter(&self) -> impl Iterator<Item = &SenderAction> {
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

impl Default for SenderActions {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a SenderActions {
    type Item = &'a SenderAction;
    type IntoIter = core::slice::Iter<'a, SenderAction>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

/// Error from sender operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SenderError {
    /// Send buffer has no space for the payload.
    #[error("send buffer full")]
    BufferFull,
    /// All send window slots are occupied.
    #[error("send window full")]
    WindowFull,
}

/// Information about a buffered packet's payload and metadata.
pub struct PayloadInfo<'a> {
    /// Sequence number of the packet.
    pub seq: SequenceCount,
    /// Segmentation flags for this packet.
    pub flags: SequenceFlag,
    /// Destination address for this packet.
    pub target: Address,
    /// Payload data bytes.
    pub payload: &'a [u8],
}

/// Configuration for the sender.
#[derive(Debug, Clone)]
#[derive(bon::Builder)]
pub struct SenderConfig {
    /// Local source address for outgoing packets.
    pub source_address: Address,
    /// APID used for all packets from this sender.
    pub apid: Apid,
    /// cFE function code for outgoing packets.
    pub function_code: u8,
    /// ISL routing message ID.
    pub message_id: u8,
    /// ISL routing action code.
    pub action_code: u8,
    /// Retransmission timeout in ticks.
    pub rto_ticks: u32,
    /// Maximum number of retransmission attempts per packet.
    pub max_retransmits: u8,
    /// Total header overhead per packet in bytes.
    pub header_overhead: usize,
}

/// State of a packet slot.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
enum SlotState {
    #[default]
    Empty,
    PendingTransmit,
    AwaitingAck,
}

/// Metadata for a packet in the send buffer.
#[derive(Clone, Copy)]
struct PacketMeta {
    state: SlotState,
    seq: u16,
    flags: SequenceFlag,
    target: RawAddress,
    retransmit_count: u8,
    offset: usize,
    len: usize,
    is_segmented: bool,
}

impl Default for PacketMeta {
    fn default() -> Self {
        Self {
            state: SlotState::Empty,
            seq: 0,
            flags: SequenceFlag::Unsegmented,
            target: RawAddress::from(Address::ground(0)),
            retransmit_count: 0,
            offset: 0,
            len: 0,
            is_segmented: false,
        }
    }
}

/// Sender state machine.
///
/// Handles segmentation, buffering, and retransmission.
/// Completely synchronous - no I/O, no async.
///
/// # Type Parameters
///
/// * `WIN` - Maximum number of in-flight packets (window size)
/// * `BUF` - Total send buffer size in bytes
/// * `MTU` - Maximum transmission unit (packet size)
pub struct SenderMachine<const WIN: usize, const BUF: usize, const MTU: usize> {
    config: SenderConfig,
    meta: [PacketMeta; WIN],
    data: [u8; BUF],
    write_pos: usize,
    next_seq: u16,
    send_base: u16,
}

impl<const WIN: usize, const BUF: usize, const MTU: usize> SenderMachine<WIN, BUF, MTU> {
    /// Create a new sender state machine with the given configuration.
    pub fn new(config: SenderConfig) -> Self {
        Self {
            config,
            meta: [PacketMeta::default(); WIN],
            data: [0u8; BUF],
            write_pos: 0,
            next_seq: 0,
            send_base: 0,
        }
    }

    /// Returns a reference to the sender configuration.
    pub fn config(&self) -> &SenderConfig {
        &self.config
    }

    /// Maximum payload bytes per packet given the MTU and header overhead.
    pub fn max_payload_per_packet(&self) -> usize {
        MTU.saturating_sub(self.config.header_overhead)
    }

    /// Available space in the send buffer (bytes).
    pub fn available_bytes(&self) -> usize {
        BUF - self.write_pos
    }

    /// Available slots in send window.
    pub fn available_window(&self) -> usize {
        WIN.saturating_sub(self.unacked_count())
    }

    /// Number of unacked packets.
    pub fn unacked_count(&self) -> usize {
        self.meta
            .iter()
            .filter(|m| m.state != SlotState::Empty)
            .count()
    }

    /// Check if all data has been acknowledged.
    pub fn is_idle(&self) -> bool {
        self.meta.iter().all(|m| m.state == SlotState::Empty)
    }

    /// Retrieve payload info for a buffered packet by sequence number.
    pub fn get_payload(&self, seq: SequenceCount) -> Option<PayloadInfo<'_>> {
        let seq_val = seq.value();
        self.meta
            .iter()
            .find(|m| m.state != SlotState::Empty && m.seq == seq_val)
            .map(|m| PayloadInfo {
                seq: SequenceCount::from(m.seq),
                flags: m.flags,
                target: m.target.parse(),
                payload: &self.data[m.offset..m.offset + m.len],
            })
    }

    /// Process an event and produce actions.
    pub fn handle(
        &mut self,
        event: SenderEvent<'_>,
        actions: &mut SenderActions,
    ) -> Result<(), SenderError> {
        actions.clear();

        match event {
            SenderEvent::SendRequest { target, data } => {
                self.handle_send_request(target, data, actions)?;
            }
            SenderEvent::AckReceived {
                cumulative_ack,
                selective_bitmap,
            } => {
                self.handle_ack(cumulative_ack, selective_bitmap, actions);
            }
            SenderEvent::RetransmitTimeout { seq } => {
                self.handle_timeout(seq, actions);
            }
        }
        Ok(())
    }

    fn handle_send_request(
        &mut self,
        target: Address,
        data: &[u8],
        actions: &mut SenderActions,
    ) -> Result<(), SenderError> {
        let max_payload = self.max_payload_per_packet();

        if data.len() <= max_payload {
            self.queue_packet(target, data, SequenceFlag::Unsegmented, false, actions)?;
        } else {
            let mut offset = 0;
            let mut is_first = true;

            while offset < data.len() {
                let remaining = data.len() - offset;
                let chunk_size = remaining.min(max_payload);
                let is_last = offset + chunk_size >= data.len();

                let flags = if is_first {
                    is_first = false;
                    SequenceFlag::First
                } else if is_last {
                    SequenceFlag::Last
                } else {
                    SequenceFlag::Continuation
                };

                self.queue_packet(
                    target,
                    &data[offset..offset + chunk_size],
                    flags,
                    true,
                    actions,
                )?;
                offset += chunk_size;
            }
        }

        Ok(())
    }

    fn queue_packet(
        &mut self,
        target: Address,
        payload: &[u8],
        flags: SequenceFlag,
        is_segmented: bool,
        actions: &mut SenderActions,
    ) -> Result<(), SenderError> {
        let slot_idx = self
            .meta
            .iter()
            .position(|m| m.state == SlotState::Empty)
            .ok_or(SenderError::WindowFull)?;

        if payload.len() > self.available_bytes() {
            self.compact();
            if payload.len() > self.available_bytes() {
                return Err(SenderError::BufferFull);
            }
        }

        let offset = self.write_pos;
        self.data[offset..offset + payload.len()].copy_from_slice(payload);
        self.write_pos += payload.len();

        let seq = SequenceCount::from(self.next_seq);

        self.meta[slot_idx] = PacketMeta {
            state: SlotState::PendingTransmit,
            seq: self.next_seq,
            flags,
            target: RawAddress::from(target),
            retransmit_count: 0,
            offset,
            len: payload.len(),
            is_segmented,
        };

        self.next_seq = (self.next_seq + 1) & SequenceCount::MAX;

        actions.push(SenderAction::Transmit {
            seq,
            rto_ticks: self.config.rto_ticks,
        });

        Ok(())
    }

    fn handle_ack(
        &mut self,
        cumulative_ack: SequenceCount,
        selective_bitmap: u16,
        actions: &mut SenderActions,
    ) {
        let ack_val = cumulative_ack.value();
        let mut freed_bytes = 0usize;

        // Clear acked packets (cumulative)
        for meta in &mut self.meta {
            if meta.state != SlotState::Empty {
                let diff = ack_val.wrapping_sub(meta.seq) & SequenceCount::MAX;
                if diff < (SequenceCount::MAX / 2) {
                    freed_bytes += meta.len;
                    let seq = SequenceCount::from(meta.seq);
                    actions.push(SenderAction::StopTimer { seq });
                    meta.state = SlotState::Empty;
                }
            }
        }

        // Handle selective ACKs
        for bit_pos in 0..16u16 {
            if selective_bitmap & (1 << bit_pos) != 0 {
                let acked_seq = (ack_val + 1 + bit_pos) & SequenceCount::MAX;
                for meta in &mut self.meta {
                    if meta.state != SlotState::Empty && meta.seq == acked_seq {
                        freed_bytes += meta.len;
                        let seq = SequenceCount::from(meta.seq);
                        actions.push(SenderAction::StopTimer { seq });
                        meta.state = SlotState::Empty;
                        break;
                    }
                }
            }
        }

        self.update_send_base();

        if freed_bytes > 0 {
            actions.push(SenderAction::SpaceAvailable {
                bytes: self.available_bytes(),
            });
        }
    }

    fn handle_timeout(&mut self, seq: SequenceCount, actions: &mut SenderActions) {
        let seq_val = seq.value();
        let mut lost_segmented = false;

        for i in 0..self.meta.len() {
            if self.meta[i].state == SlotState::AwaitingAck && self.meta[i].seq == seq_val {
                if self.meta[i].retransmit_count >= self.config.max_retransmits {
                    actions.push(SenderAction::PacketLost { seq });
                    lost_segmented = self.meta[i].is_segmented;
                    self.meta[i].state = SlotState::Empty;
                } else {
                    self.meta[i].state = SlotState::PendingTransmit;
                    self.meta[i].retransmit_count += 1;
                    actions.push(SenderAction::Transmit {
                        seq,
                        rto_ticks: self.config.rto_ticks,
                    });
                }
                break;
            }
        }

        if lost_segmented {
            actions.push(SenderAction::MessageLost);
        }

        self.update_send_base();
    }

    /// Mark a packet as transmitted (awaiting ACK).
    ///
    /// Call this after sending a packet from a `Transmit` action.
    pub fn mark_transmitted(&mut self, seq: SequenceCount) {
        let seq_val = seq.value();
        for meta in &mut self.meta {
            if meta.state == SlotState::PendingTransmit && meta.seq == seq_val {
                meta.state = SlotState::AwaitingAck;
                break;
            }
        }
    }

    fn update_send_base(&mut self) {
        let mut min_unacked = self.next_seq;
        let mut found = false;

        for meta in &self.meta {
            if meta.state != SlotState::Empty {
                found = true;
                let diff = min_unacked.wrapping_sub(meta.seq) & SequenceCount::MAX;
                if diff < (SequenceCount::MAX / 2) && diff > 0 {
                    min_unacked = meta.seq;
                }
            }
        }

        self.send_base = if found { min_unacked } else { self.next_seq };
    }

    fn compact(&mut self) {
        let mut indices: [Option<usize>; WIN] = [None; WIN];
        let mut count = 0;

        for (i, m) in self.meta.iter().enumerate() {
            if m.state != SlotState::Empty {
                let offset = m.offset;
                let mut insert_pos = count;
                for j in 0..count {
                    if let Some(idx) = indices[j] {
                        if self.meta[idx].offset > offset {
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
            self.write_pos = 0;
            return;
        }

        let mut new_pos = 0usize;
        for idx_opt in indices.iter().take(count) {
            if let Some(idx) = *idx_opt {
                let old_offset = self.meta[idx].offset;
                let len = self.meta[idx].len;

                if new_pos != old_offset {
                    self.data.copy_within(old_offset..old_offset + len, new_pos);
                }

                self.meta[idx].offset = new_pos;
                new_pos += len;
            }
        }

        self.write_pos = new_pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::srspp::packet::SrsppDataPacket;

    fn make_config() -> SenderConfig {
        SenderConfig {
            source_address: Address::satellite(1, 5),
            apid: Apid::new(0x42).unwrap(),
            function_code: 0,
            message_id: 0,
            action_code: 0,
            rto_ticks: 100,
            max_retransmits: 3,
            header_overhead: SrsppDataPacket::HEADER_SIZE,
        }
    }

    fn target() -> Address {
        Address::satellite(2, 3)
    }

    #[test]
    fn test_send_emits_transmit_action() {
        let mut sender: SenderMachine<8, 4096, 512> = SenderMachine::new(make_config());
        let mut actions = SenderActions::new();

        sender
            .handle(SenderEvent::SendRequest { target: target(), data: &[1, 2, 3] }, &mut actions)
            .unwrap();

        // Should have one Transmit action
        assert_eq!(actions.len(), 1);
        let action = actions.iter().next().unwrap();
        assert!(matches!(
            action,
            SenderAction::Transmit { seq, rto_ticks: 100 } if seq.value() == 0
        ));

        let info = sender.get_payload(SequenceCount::from(0)).unwrap();
        assert!(!info.payload.is_empty());
    }

    #[test]
    fn test_mark_transmitted() {
        let mut sender: SenderMachine<8, 4096, 512> = SenderMachine::new(make_config());
        let mut actions = SenderActions::new();

        sender
            .handle(SenderEvent::SendRequest { target: target(), data: &[1, 2, 3] }, &mut actions)
            .unwrap();

        // Before marking - packet is PendingTransmit
        assert_eq!(sender.unacked_count(), 1);

        // Mark as transmitted
        sender.mark_transmitted(SequenceCount::from(0));

        // Still unacked (now AwaitingAck)
        assert_eq!(sender.unacked_count(), 1);
    }

    #[test]
    fn test_ack_stops_timer() {
        let mut sender: SenderMachine<8, 4096, 512> = SenderMachine::new(make_config());
        let mut actions = SenderActions::new();

        sender
            .handle(SenderEvent::SendRequest { target: target(), data: &[1, 2, 3] }, &mut actions)
            .unwrap();
        sender.mark_transmitted(SequenceCount::from(0));

        sender
            .handle(
                SenderEvent::AckReceived {
                    cumulative_ack: SequenceCount::from(0),
                    selective_bitmap: 0,
                },
                &mut actions,
            )
            .unwrap();

        assert!(
            actions
                .iter()
                .any(|a| matches!(a, SenderAction::StopTimer { seq } if seq.value() == 0))
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, SenderAction::SpaceAvailable { .. }))
        );
        assert!(sender.is_idle());
    }

    #[test]
    fn test_timeout_emits_retransmit() {
        let mut sender: SenderMachine<8, 4096, 512> = SenderMachine::new(make_config());
        let mut actions = SenderActions::new();

        sender
            .handle(SenderEvent::SendRequest { target: target(), data: &[1, 2, 3] }, &mut actions)
            .unwrap();
        sender.mark_transmitted(SequenceCount::from(0));

        // Timeout
        sender
            .handle(
                SenderEvent::RetransmitTimeout {
                    seq: SequenceCount::from(0),
                },
                &mut actions,
            )
            .unwrap();

        // Should emit another Transmit
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, SenderAction::Transmit { seq, .. } if seq.value() == 0))
        );
    }

    #[test]
    fn test_max_retransmits_emits_packet_lost() {
        let mut sender: SenderMachine<8, 4096, 512> = SenderMachine::new(make_config());
        let mut actions = SenderActions::new();

        sender
            .handle(SenderEvent::SendRequest { target: target(), data: &[1, 2, 3] }, &mut actions)
            .unwrap();
        sender.mark_transmitted(SequenceCount::from(0));

        // Timeout multiple times (max_retransmits = 3)
        for _ in 0..3 {
            sender
                .handle(
                    SenderEvent::RetransmitTimeout {
                        seq: SequenceCount::from(0),
                    },
                    &mut actions,
                )
                .unwrap();
            sender.mark_transmitted(SequenceCount::from(0));
        }

        // One more timeout should trigger PacketLost
        sender
            .handle(
                SenderEvent::RetransmitTimeout {
                    seq: SequenceCount::from(0),
                },
                &mut actions,
            )
            .unwrap();

        assert!(
            actions
                .iter()
                .any(|a| matches!(a, SenderAction::PacketLost { seq } if seq.value() == 0))
        );
        assert!(sender.is_idle());
    }

    #[test]
    fn test_segmentation() {
        let mut sender: SenderMachine<8, 4096, 64> = SenderMachine::new(make_config());
        let mut actions = SenderActions::new();

        let data = [0u8; 150];
        sender
            .handle(SenderEvent::SendRequest { target: target(), data: &data }, &mut actions)
            .unwrap();

        let max_payload = 64 - SrsppDataPacket::HEADER_SIZE;
        let expected = (150 + max_payload - 1) / max_payload;
        let transmit_count = actions
            .iter()
            .filter(|a| matches!(a, SenderAction::Transmit { .. }))
            .count();
        assert_eq!(transmit_count, expected);
    }

    #[test]
    fn test_selective_ack() {
        let mut sender: SenderMachine<8, 4096, 512> = SenderMachine::new(make_config());
        let mut actions = SenderActions::new();

        // Send 3 packets
        for i in 0..3 {
            sender
                .handle(
                    SenderEvent::SendRequest {
                        target: target(),
                        data: &[i as u8; 10],
                    },
                    &mut actions,
                )
                .unwrap();
            sender.mark_transmitted(SequenceCount::from(i));
        }

        // ACK packet 0 cumulatively, packet 2 selectively (bitmap bit 1 = seq 2)
        sender
            .handle(
                SenderEvent::AckReceived {
                    cumulative_ack: SequenceCount::from(0),
                    selective_bitmap: 0b0010, // bit 1 = cumulative + 2 = seq 2
                },
                &mut actions,
            )
            .unwrap();

        // Should have StopTimer for 0 and 2, but not 1
        let stopped: heapless::Vec<u16, 8> = actions
            .iter()
            .filter_map(|a| {
                if let SenderAction::StopTimer { seq } = a {
                    Some(seq.value())
                } else {
                    None
                }
            })
            .collect();

        assert!(stopped.contains(&0));
        assert!(stopped.contains(&2));
        assert!(!stopped.contains(&1));

        // Packet 1 still pending
        assert!(!sender.is_idle());
        assert!(sender.get_payload(SequenceCount::from(1)).is_some());
    }

    #[test]
    fn test_message_lost_on_segmented_packet_loss() {
        let mut sender: SenderMachine<8, 4096, 64> = SenderMachine::new(make_config());
        let mut actions = SenderActions::new();

        let data = [0u8; 150];
        sender
            .handle(SenderEvent::SendRequest { target: target(), data: &data }, &mut actions)
            .unwrap();

        let transmit_count = actions
            .iter()
            .filter(|a| matches!(a, SenderAction::Transmit { .. }))
            .count();
        assert!(transmit_count >= 3);

        for i in 0..transmit_count as u16 {
            sender.mark_transmitted(SequenceCount::from(i));
        }

        for _ in 0..3 {
            sender
                .handle(
                    SenderEvent::RetransmitTimeout {
                        seq: SequenceCount::from(1),
                    },
                    &mut actions,
                )
                .unwrap();
            sender.mark_transmitted(SequenceCount::from(1));
        }

        sender
            .handle(
                SenderEvent::RetransmitTimeout {
                    seq: SequenceCount::from(1),
                },
                &mut actions,
            )
            .unwrap();

        assert!(actions
            .iter()
            .any(|a| matches!(a, SenderAction::PacketLost { seq } if seq.value() == 1)));
        assert!(actions
            .iter()
            .any(|a| matches!(a, SenderAction::MessageLost)));
    }
}
