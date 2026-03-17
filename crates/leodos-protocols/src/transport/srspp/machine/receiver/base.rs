use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;

use super::{ReceiverAction, ReceiverActions, ReceiverConfig};

/// Shared receiver logic for ACK generation, sequencing, and timers.
pub struct ReceiverBase {
    /// Receiver configuration parameters.
    config: ReceiverConfig,
    /// Address of the remote sender.
    remote_address: Address,
    /// Next expected sequence number.
    expected_seq: u16,
    /// Bitmap of received out-of-order packets relative to `expected_seq`.
    recv_bitmap: u16,
    /// Whether an ACK needs to be sent.
    ack_pending: bool,
    /// Whether the delayed ACK timer is currently running.
    ack_timer_running: bool,
}

impl ReceiverBase {
    /// Create a new receiver core for the given remote sender.
    pub fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self {
            config,
            remote_address,
            expected_seq: 0,
            recv_bitmap: 0,
            ack_pending: false,
            ack_timer_running: false,
        }
    }

    /// Returns the remote sender address.
    pub fn remote_address(&self) -> Address {
        self.remote_address
    }

    /// Returns the next expected sequence number.
    pub fn expected_seq(&self) -> SequenceCount {
        SequenceCount::from(self.expected_seq)
    }

    /// Returns the raw u16 expected sequence number.
    pub fn expected_seq_raw(&self) -> u16 {
        self.expected_seq
    }

    /// Returns a reference to the receiver configuration.
    #[allow(dead_code)]
    pub fn config(&self) -> &ReceiverConfig {
        &self.config
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

    /// Emit a SendAck action with the current cumulative ACK and bitmap.
    pub fn emit_ack(&mut self, actions: &mut ReceiverActions) {
        let cumulative = self.expected_seq.wrapping_sub(1) & SequenceCount::MAX;

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

    /// Handle expiry of the delayed ACK timer.
    pub fn handle_ack_timeout(&mut self, actions: &mut ReceiverActions) {
        self.ack_timer_running = false;
        if self.ack_pending {
            self.emit_ack(actions);
        }
    }

    /// Emit ACK/timer actions after processing a data packet.
    pub fn apply_post_data_logic(
        &mut self,
        actions: &mut ReceiverActions,
        progressed: bool,
        has_gap: bool,
    ) {
        if let Some(ticks) = self.config.progress_timeout_ticks {
            if progressed {
                actions.push(ReceiverAction::StopProgressTimer);
                if has_gap {
                    actions.push(ReceiverAction::StartProgressTimer { ticks });
                }
            } else if has_gap {
                actions.push(ReceiverAction::StartProgressTimer { ticks });
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
    }

    /// Emit timer actions after processing a progress timeout.
    pub fn apply_post_progress_logic(&mut self, actions: &mut ReceiverActions, has_gap: bool) {
        if has_gap {
            if let Some(ticks) = self.config.progress_timeout_ticks {
                actions.push(ReceiverAction::StartProgressTimer { ticks });
            }
        } else {
            actions.push(ReceiverAction::StopProgressTimer);
        }
    }
}
