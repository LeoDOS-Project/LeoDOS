use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;

use super::{ReceiverAction, ReceiverActions, ReceiverConfig};

pub struct ReceiverShell {
    config: ReceiverConfig,
    remote_address: Address,
    expected_seq: u16,
    recv_bitmap: u16,
    ack_pending: bool,
    ack_timer_running: bool,
}

impl ReceiverShell {
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

    pub fn remote_address(&self) -> Address {
        self.remote_address
    }

    pub fn expected_seq(&self) -> SequenceCount {
        SequenceCount::from(self.expected_seq)
    }

    pub fn expected_seq_raw(&self) -> u16 {
        self.expected_seq
    }

    pub fn config(&self) -> &ReceiverConfig {
        &self.config
    }

    pub fn distance(&self, seq: SequenceCount) -> u16 {
        seq.value().wrapping_sub(self.expected_seq) & SequenceCount::MAX
    }

    pub fn is_ooo_duplicate(&self, distance: u16) -> bool {
        debug_assert!(distance > 0);
        let bit_pos = distance - 1;
        let mask = 1u16 << bit_pos;
        self.recv_bitmap & mask != 0
    }

    pub fn record_ooo(&mut self, distance: u16) {
        debug_assert!(distance > 0);
        let bit_pos = distance - 1;
        self.recv_bitmap |= 1u16 << bit_pos;
    }

    pub fn advance(&mut self) {
        self.expected_seq = (self.expected_seq + 1) & SequenceCount::MAX;
        self.recv_bitmap >>= 1;
    }

    pub fn emit_ack(&mut self, actions: &mut ReceiverActions) {
        let cumulative =
            self.expected_seq.wrapping_sub(1) & SequenceCount::MAX;

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

    pub fn handle_ack_timeout(&mut self, actions: &mut ReceiverActions) {
        self.ack_timer_running = false;
        if self.ack_pending {
            self.emit_ack(actions);
        }
    }

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

    pub fn apply_post_progress_logic(
        &mut self,
        actions: &mut ReceiverActions,
        has_gap: bool,
    ) {
        if has_gap {
            if let Some(ticks) = self.config.progress_timeout_ticks {
                actions.push(ReceiverAction::StartProgressTimer { ticks });
            }
        } else {
            actions.push(ReceiverAction::StopProgressTimer);
        }
    }
}
