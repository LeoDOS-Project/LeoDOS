//! Receiver state machine for SRSPP.

mod base;
/// Shared data structures used by receiver backends.
pub mod utils;

/// Receiver backends with different performance and memory tradeoffs.
pub mod backends;

pub use backends::fast::FastReceiver;
pub use backends::lite::LiteReceiver;
pub use backends::packed::PackedReceiver;

use crate::network::isl::address::Address;
use crate::network::spp::Apid;
use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;

/// ACK information emitted by the receiver state machine.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct AckInfo {
    /// Address to send the ACK to.
    pub destination: Address,
    /// Highest contiguously received sequence number.
    pub cumulative_ack: SequenceCount,
    /// Bitmap of selectively acknowledged packets.
    pub selective_bitmap: u16,
}

/// Timer action from the receiver state machine.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TimerAction {
    /// Start (or restart) the timer with the given duration.
    Start {
        /// Timer duration in ticks.
        ticks: u32,
    },
    /// Stop the timer.
    Stop,
}

/// Result of a receiver state machine event.
#[derive(Debug, Copy, Clone, Default)]
pub struct HandleResult {
    /// ACK to send, if any.
    pub ack: Option<AckInfo>,
    /// Action for the delayed ACK timer; `None` means no change.
    pub ack_timer: Option<TimerAction>,
    /// Action for the progress timer; `None` means no change.
    pub progress_timer: Option<TimerAction>,
}

/// Outcome of processing a data packet in the backend.
#[derive(Debug, Copy, Clone)]
pub struct DataOutcome {
    /// Whether the expected sequence advanced (gap filled or in-order delivery).
    pub progressed: bool,
    /// Whether out-of-order packets remain buffered.
    pub has_gap: bool,
}

/// Outcome of skipping a gap in the backend.
#[derive(Debug, Copy, Clone)]
pub struct GapOutcome {
    /// Whether out-of-order packets remain buffered after the skip.
    pub has_gap: bool,
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
#[derive(Debug, Clone, bon::Builder)]
pub struct ReceiverConfig {
    /// Local address of this receiver.
    pub local_address: Address,
    /// APID filter for incoming packets.
    pub apid: Apid,
    /// cFE function code for outgoing ACK packets.
    pub function_code: u8,
    /// If true, send ACKs immediately; otherwise use delayed ACKs.
    pub immediate_ack: bool,
    /// Delayed ACK timer duration in ticks.
    pub ack_delay_ticks: u32,
    /// Progress timeout in ticks; `None` disables gap-skipping.
    pub progress_timeout_ticks: Option<u32>,
}

/// Default receiver backend (alias for [`PackedReceiver`]).
///
/// * `WIN` — maximum number of out-of-order packets to buffer
/// * `BUF` — reorder slab capacity in bytes
/// * `REASM` — maximum reassembled message size
pub type ReceiverMachine<const WIN: usize, const BUF: usize, const REASM: usize> =
    PackedReceiver<WIN, BUF, REASM>;

/// ACK and timer state, driven by `DataOutcome`/`GapOutcome` from a backend.
pub struct AckState {
    /// Address of the remote sender (ACK destination).
    remote_address: Address,
    /// If true, send ACKs immediately; otherwise use delayed ACKs.
    immediate_ack: bool,
    /// Delayed ACK timer duration in ticks.
    ack_delay_ticks: u32,
    /// Progress timeout in ticks; `None` disables gap-skipping.
    progress_timeout_ticks: Option<u32>,
    /// Whether an ACK needs to be sent.
    ack_pending: bool,
    /// Whether the delayed ACK timer is currently running.
    ack_timer_running: bool,
}

impl AckState {
    /// Create ACK state for a stream from receiver config and remote address.
    pub fn new(config: &ReceiverConfig, remote_address: Address) -> Self {
        Self {
            remote_address,
            immediate_ack: config.immediate_ack,
            ack_delay_ticks: config.ack_delay_ticks,
            progress_timeout_ticks: config.progress_timeout_ticks,
            ack_pending: false,
            ack_timer_running: false,
        }
    }

    fn emit_ack(&mut self, seq: SequenceCount, bitmap: u16) -> HandleResult {
        let cumulative = seq.value().wrapping_sub(1) & SequenceCount::MAX;
        let ack_timer = self.ack_timer_running.then(|| TimerAction::Stop);
        self.ack_timer_running = false;
        self.ack_pending = false;
        HandleResult {
            ack: Some(AckInfo {
                destination: self.remote_address,
                cumulative_ack: SequenceCount::from(cumulative),
                selective_bitmap: bitmap,
            }),
            ack_timer,
            progress_timer: None,
        }
    }

    /// Handle delayed ACK timer expiry.
    pub fn on_ack_timeout(&mut self, seq: SequenceCount, bitmap: u16) -> HandleResult {
        self.ack_timer_running = false;
        if self.ack_pending {
            self.emit_ack(seq, bitmap)
        } else {
            HandleResult::default()
        }
    }

    /// Produce ACK/timer actions after the backend processed a data packet.
    pub fn on_data(
        &mut self,
        outcome: DataOutcome,
        seq: SequenceCount,
        bitmap: u16,
    ) -> HandleResult {
        let progress_timer = self.progress_timeout_ticks.and_then(|ticks| {
            outcome
                .has_gap
                .then(|| TimerAction::Start { ticks })
                .or_else(|| outcome.progressed.then(|| TimerAction::Stop))
        });

        self.ack_pending = true;

        let (ack, ack_timer) = if self.immediate_ack {
            let r = self.emit_ack(seq, bitmap);
            (r.ack, r.ack_timer)
        } else if !self.ack_timer_running {
            self.ack_timer_running = true;
            let timer = TimerAction::Start {
                ticks: self.ack_delay_ticks,
            };
            (None, Some(timer))
        } else {
            (None, None)
        };

        HandleResult {
            ack,
            ack_timer,
            progress_timer,
        }
    }

    /// Produce timer actions after the backend skipped a gap.
    pub fn on_gap_skip(&mut self, outcome: GapOutcome) -> HandleResult {
        let progress_timer = (!outcome.has_gap).then(|| TimerAction::Stop).or_else(|| {
            self.progress_timeout_ticks
                .map(|ticks| TimerAction::Start { ticks })
        });
        HandleResult {
            ack: None,
            ack_timer: None,
            progress_timer,
        }
    }
}

/// Trait abstracting over receiver backends (buffering and delivery only).
pub trait ReceiverBackend: Sized {
    /// Create a new backend with empty buffers.
    fn new() -> Self;
    /// Process a received data packet.
    fn handle_data(
        &mut self,
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &[u8],
    ) -> Result<DataOutcome, ReceiverError>;
    /// Skip the current gap (advance past missing packets).
    fn skip_gap(&mut self) -> Result<GapOutcome, ReceiverError>;
    /// Take the complete message.
    fn take_message(&mut self) -> Option<&[u8]>;
    /// Returns a slice of the reassembly buffer.
    fn reassembly_data(&self, len: usize) -> &[u8];
    /// Check if there's a complete message ready.
    fn has_message(&self) -> bool;
    /// Returns the length of the pending message, if any.
    fn message_len(&self) -> Option<usize>;
    /// Pass the pending message to `f` and mark it consumed.
    fn consume_message<Ret>(&mut self, f: impl FnOnce(&[u8]) -> Ret) -> Option<Ret>;
    /// Get the current expected sequence number.
    fn expected_seq(&self) -> SequenceCount;
    /// Get the selective ACK bitmap.
    fn recv_bitmap(&self) -> u16;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_remote_address() -> Address {
        Address::satellite(1, 2)
    }

    fn make_config() -> ReceiverConfig {
        ReceiverConfig {
            local_address: Address::satellite(1, 1),
            apid: Apid::new(0x42).unwrap(),
            function_code: 0,
            immediate_ack: true,
            ack_delay_ticks: 20,
            progress_timeout_ticks: None,
        }
    }

    fn make_delayed_config() -> ReceiverConfig {
        ReceiverConfig {
            local_address: Address::satellite(1, 1),
            apid: Apid::new(0x42).unwrap(),
            function_code: 0,
            immediate_ack: false,
            ack_delay_ticks: 20,
            progress_timeout_ticks: None,
        }
    }

    fn make_progress_config(ticks: u32) -> ReceiverConfig {
        ReceiverConfig {
            local_address: Address::satellite(1, 1),
            apid: Apid::new(0x42).unwrap(),
            function_code: 0,
            immediate_ack: true,
            ack_delay_ticks: 20,
            progress_timeout_ticks: Some(ticks),
        }
    }

    // ── Generic test functions (all backends) ──

    fn test_immediate_ack<R: ReceiverBackend>() {
        let mut rx = R::new();
        let mut ack = AckState::new(&make_config(), test_remote_address());
        let outcome = rx
            .handle_data(
                SequenceCount::from(0),
                SequenceFlag::Unsegmented,
                &[1, 2, 3],
            )
            .unwrap();
        let r = ack.on_data(outcome, rx.expected_seq(), rx.recv_bitmap());
        assert!(r.ack.is_some());
    }

    fn test_delayed_ack_starts_timer<R: ReceiverBackend>() {
        let mut rx = R::new();
        let mut ack = AckState::new(&make_delayed_config(), test_remote_address());
        let outcome = rx
            .handle_data(
                SequenceCount::from(0),
                SequenceFlag::Unsegmented,
                &[1, 2, 3],
            )
            .unwrap();
        let r = ack.on_data(outcome, rx.expected_seq(), rx.recv_bitmap());
        assert!(matches!(
            r.ack_timer,
            Some(TimerAction::Start { ticks: 20 })
        ));
        assert!(r.ack.is_none());
    }

    fn test_ack_timeout_sends_ack<R: ReceiverBackend>() {
        let mut rx = R::new();
        let mut ack = AckState::new(&make_delayed_config(), test_remote_address());
        let outcome = rx
            .handle_data(
                SequenceCount::from(0),
                SequenceFlag::Unsegmented,
                &[1, 2, 3],
            )
            .unwrap();
        ack.on_data(outcome, rx.expected_seq(), rx.recv_bitmap());
        let r = ack.on_ack_timeout(rx.expected_seq(), rx.recv_bitmap());
        assert!(r.ack.is_some());
    }

    fn test_receive_single_packet<R: ReceiverBackend>() {
        let mut rx = R::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1, 2, 3, 4, 5],
        )
        .unwrap();
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &[1, 2, 3, 4, 5]);
    }

    fn test_out_of_order_delivery<R: ReceiverBackend>() {
        let mut rx = R::new();
        let mut ack = AckState::new(&make_config(), test_remote_address());
        let outcome = rx
            .handle_data(SequenceCount::from(1), SequenceFlag::Unsegmented, &[2])
            .unwrap();
        let r1 = ack.on_data(outcome, rx.expected_seq(), rx.recv_bitmap());
        assert!(!rx.has_message());
        assert_eq!(r1.ack.map(|a| a.selective_bitmap), Some(0b0001));

        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &[1])
            .unwrap();
        assert!(rx.has_message());
    }

    fn test_duplicate_ignored<R: ReceiverBackend>() {
        let mut rx = R::new();
        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &[1])
            .unwrap();
        rx.take_message();
        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &[99])
            .unwrap();
        assert!(!rx.has_message());
    }

    fn test_progress_timeout_skips_gap<R: ReceiverBackend>() {
        let mut rx = R::new();
        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &[1])
            .unwrap();
        rx.take_message();
        rx.handle_data(SequenceCount::from(2), SequenceFlag::Unsegmented, &[3])
            .unwrap();
        assert!(!rx.has_message());
        rx.skip_gap().unwrap();
        assert_eq!(rx.expected_seq().value(), 3);
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &[3]);
    }

    fn test_no_progress_timeout_in_reliable_mode<R: ReceiverBackend>() {
        let mut rx = R::new();
        let mut ack = AckState::new(&make_config(), test_remote_address());
        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &[1])
            .unwrap();
        rx.take_message();
        let outcome = rx
            .handle_data(SequenceCount::from(2), SequenceFlag::Unsegmented, &[3])
            .unwrap();
        let r = ack.on_data(outcome, rx.expected_seq(), rx.recv_bitmap());
        assert!(!matches!(r.progress_timer, Some(TimerAction::Start { .. })));
    }

    fn test_progress_timer_resets_on_progress<R: ReceiverBackend>() {
        let mut rx = R::new();
        let mut ack = AckState::new(&make_progress_config(50), test_remote_address());
        let o1 = rx
            .handle_data(SequenceCount::from(1), SequenceFlag::Unsegmented, &[2])
            .unwrap();
        let r1 = ack.on_data(o1, rx.expected_seq(), rx.recv_bitmap());
        assert!(matches!(
            r1.progress_timer,
            Some(TimerAction::Start { ticks: 50 })
        ));
        let o2 = rx
            .handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &[1])
            .unwrap();
        let r2 = ack.on_data(o2, rx.expected_seq(), rx.recv_bitmap());
        assert!(matches!(r2.progress_timer, Some(TimerAction::Stop)));
    }

    // ── Segmented tests (Packed + Fast only — Lite tiles at MTU boundaries) ──

    fn test_segmented_message<R: ReceiverBackend>() {
        let mut rx = R::new();
        rx.handle_data(SequenceCount::from(0), SequenceFlag::First, &[1, 2, 3])
            .unwrap();
        assert!(!rx.has_message());
        rx.handle_data(
            SequenceCount::from(1),
            SequenceFlag::Continuation,
            &[4, 5, 6],
        )
        .unwrap();
        assert!(!rx.has_message());
        rx.handle_data(SequenceCount::from(2), SequenceFlag::Last, &[7, 8])
            .unwrap();
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    fn test_progress_timeout_discards_partial<R: ReceiverBackend>() {
        let mut rx = R::new();
        rx.handle_data(SequenceCount::from(0), SequenceFlag::First, &[1, 2, 3])
            .unwrap();
        rx.handle_data(SequenceCount::from(3), SequenceFlag::Unsegmented, &[10, 11])
            .unwrap();
        assert!(!rx.has_message());
        rx.skip_gap().unwrap();
        assert!(!rx.has_message());
        rx.skip_gap().unwrap();
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &[10, 11]);
        rx.handle_data(SequenceCount::from(4), SequenceFlag::Unsegmented, &[20, 21])
            .unwrap();
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &[20, 21]);
    }

    // ── Test instantiation ──

    macro_rules! backend_tests {
        ($mod_name:ident, $ty:ty) => {
            mod $mod_name {
                use super::*;
                #[test]
                fn immediate_ack() {
                    test_immediate_ack::<$ty>()
                }
                #[test]
                fn delayed_ack_starts_timer() {
                    test_delayed_ack_starts_timer::<$ty>()
                }
                #[test]
                fn ack_timeout_sends_ack() {
                    test_ack_timeout_sends_ack::<$ty>()
                }
                #[test]
                fn receive_single_packet() {
                    test_receive_single_packet::<$ty>()
                }
                #[test]
                fn out_of_order_delivery() {
                    test_out_of_order_delivery::<$ty>()
                }
                #[test]
                fn duplicate_ignored() {
                    test_duplicate_ignored::<$ty>()
                }
                #[test]
                fn progress_timeout_skips_gap() {
                    test_progress_timeout_skips_gap::<$ty>()
                }
                #[test]
                fn no_progress_timeout_in_reliable_mode() {
                    test_no_progress_timeout_in_reliable_mode::<$ty>()
                }
                #[test]
                fn progress_timer_resets_on_progress() {
                    test_progress_timer_resets_on_progress::<$ty>()
                }
            }
        };
    }

    backend_tests!(packed, PackedReceiver<8, 4096, 8192>);
    backend_tests!(fast, FastReceiver<8, 512, 8192, 4096>);
    backend_tests!(lite, LiteReceiver<4096, 8, 512>);

    // Segmented reassembly tests — not applicable to LiteReceiver
    mod packed_segmented {
        use super::*;
        #[test]
        fn segmented_message() {
            test_segmented_message::<PackedReceiver<8, 4096, 8192>>()
        }
        #[test]
        fn progress_timeout_discards_partial() {
            test_progress_timeout_discards_partial::<PackedReceiver<8, 4096, 8192>>()
        }
    }
    mod fast_segmented {
        use super::*;
        #[test]
        fn segmented_message() {
            test_segmented_message::<FastReceiver<8, 512, 8192, 4096>>()
        }
        #[test]
        fn progress_timeout_discards_partial() {
            test_progress_timeout_discards_partial::<FastReceiver<8, 512, 8192, 4096>>()
        }
    }

    // ── Backend-specific standalone tests ──

    #[test]
    fn test_receiver_machine_alias() {
        let mut rx: ReceiverMachine<8, 4096, 8192> = ReceiverMachine::new();
        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &[42])
            .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[42]);
    }

    #[test]
    fn fast_receiver_seq_wraparound() {
        let mut rx: FastReceiver<8, 512, 8192, 4096> = FastReceiver::new();

        for i in 0..SequenceCount::MAX {
            rx.handle_data(
                SequenceCount::from(i),
                SequenceFlag::Unsegmented,
                &[i as u8],
            )
            .unwrap();
            rx.take_message();
        }

        rx.handle_data(
            SequenceCount::from(SequenceCount::MAX),
            SequenceFlag::Unsegmented,
            &[0xFF],
        )
        .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[0xFF]);

        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &[0x00])
            .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[0x00]);
        assert_eq!(rx.expected_seq().value(), 1);
    }

    #[test]
    fn packed_receiver_slab_reset() {
        let mut rx: PackedReceiver<8, 128, 8192> = PackedReceiver::new();

        let big = [0xAA; 60];
        rx.handle_data(SequenceCount::from(1), SequenceFlag::Unsegmented, &big)
            .unwrap();
        rx.handle_data(SequenceCount::from(0), SequenceFlag::Unsegmented, &big)
            .unwrap();
        rx.take_message();
        rx.take_message();

        rx.handle_data(SequenceCount::from(3), SequenceFlag::Unsegmented, &big)
            .unwrap();
        rx.handle_data(SequenceCount::from(2), SequenceFlag::Unsegmented, &big)
            .unwrap();
        assert!(rx.has_message());
    }
}
