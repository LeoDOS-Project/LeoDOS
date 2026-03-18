use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;

use super::AckInfo;
use super::HandleResult;
use super::ReceiverConfig;
use super::TimerAction;

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

    /// Emit a SendAck result with the current cumulative ACK and bitmap.
    pub fn emit_ack(&mut self) -> HandleResult {
        let cumulative = self.expected_seq.wrapping_sub(1) & SequenceCount::MAX;
        let ack_timer = if self.ack_timer_running {
            self.ack_timer_running = false;
            Some(TimerAction::Stop)
        } else {
            None
        };
        self.ack_pending = false;
        HandleResult {
            ack: Some(AckInfo {
                destination: self.remote_address,
                cumulative_ack: SequenceCount::from(cumulative),
                selective_bitmap: self.recv_bitmap,
            }),
            ack_timer,
            progress_timer: None,
        }
    }

    /// Handle expiry of the delayed ACK timer.
    pub fn handle_ack_timeout(&mut self) -> HandleResult {
        self.ack_timer_running = false;
        if self.ack_pending {
            self.emit_ack()
        } else {
            HandleResult::default()
        }
    }

    /// Emit ACK/timer result after processing a data packet.
    pub fn apply_post_data_logic(&mut self, progressed: bool, has_gap: bool) -> HandleResult {
        let progress_timer = if let Some(ticks) = self.config.progress_timeout_ticks {
            if progressed {
                if has_gap {
                    Some(TimerAction::Start { ticks })
                } else {
                    Some(TimerAction::Stop)
                }
            } else if has_gap {
                Some(TimerAction::Start { ticks })
            } else {
                None
            }
        } else {
            None
        };

        self.ack_pending = true;

        let (ack, ack_timer) = if self.config.immediate_ack {
            let r = self.emit_ack();
            (r.ack, r.ack_timer)
        } else if !self.ack_timer_running {
            self.ack_timer_running = true;
            (None, Some(TimerAction::Start { ticks: self.config.ack_delay_ticks }))
        } else {
            (None, None)
        };

        HandleResult { ack, ack_timer, progress_timer }
    }

    /// Emit timer result after processing a progress timeout.
    pub fn apply_post_progress_logic(&mut self, has_gap: bool) -> HandleResult {
        let progress_timer = if has_gap {
            self.config.progress_timeout_ticks.map(|ticks| TimerAction::Start { ticks })
        } else {
            Some(TimerAction::Stop)
        };
        HandleResult { ack: None, ack_timer: None, progress_timer }
    }
}
