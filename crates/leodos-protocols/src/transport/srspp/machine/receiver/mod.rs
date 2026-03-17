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
use crate::network::spp::{Apid, SequenceCount, SequenceFlag};
use heapless::Vec;

/// Maximum number of actions that can be emitted per event.
const MAX_ACTIONS: usize = 32;

/// Actions the receiver machine wants the driver to perform.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ReceiverAction {
    /// Send an ACK packet to a specific destination.
    SendAck {
        /// Address to send the ACK to.
        destination: Address,
        /// Highest contiguously received sequence number.
        cumulative_ack: SequenceCount,
        /// Bitmap of selectively acknowledged packets.
        selective_bitmap: u16,
    },
    /// Start ACK delay timer.
    StartAckTimer {
        /// Timer duration in ticks.
        ticks: u32,
    },
    /// Stop ACK delay timer.
    StopAckTimer,
    /// A complete message is ready. Call `take_message()`.
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

/// Trait abstracting over receiver backends.
pub trait ReceiverBackend: Sized {
    /// Create a new receiver for a specific remote sender.
    fn new(config: ReceiverConfig, remote_address: Address) -> Self;
    /// Get the remote address.
    fn remote_address(&self) -> Address;
    /// Process a received data packet.
    fn handle_data(
        &mut self,
        seq: SequenceCount,
        flags: SequenceFlag,
        payload: &[u8],
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError>;
    /// Handle ACK delay timer expiry.
    fn handle_ack(&mut self, actions: &mut ReceiverActions);
    /// Handle progress timer expiry.
    fn handle_timeout(&mut self, actions: &mut ReceiverActions) -> Result<(), ReceiverError>;
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
            immediate_ack: false,
            ack_delay_ticks: 20,
            progress_timeout_ticks: None,
        }
    }

    fn make_progress_config(ticks: u32) -> ReceiverConfig {
        ReceiverConfig {
            local_address: test_local_address(),
            apid: Apid::new(0x42).unwrap(),
            function_code: 0,
            immediate_ack: true,
            ack_delay_ticks: 20,
            progress_timeout_ticks: Some(ticks),
        }
    }

    // ── Generic test functions (all backends) ──

    fn test_immediate_ack<R: ReceiverBackend>() {
        let mut rx = R::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1, 2, 3],
            &mut a,
        )
        .unwrap();
        assert!(
            a.iter()
                .any(|a| matches!(a, ReceiverAction::SendAck { .. }))
        );
    }

    fn test_delayed_ack_starts_timer<R: ReceiverBackend>() {
        let mut rx = R::new(make_delayed_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1, 2, 3],
            &mut a,
        )
        .unwrap();
        assert!(
            a.iter()
                .any(|a| matches!(a, ReceiverAction::StartAckTimer { ticks: 20 }))
        );
        assert!(
            !a.iter()
                .any(|a| matches!(a, ReceiverAction::SendAck { .. }))
        );
    }

    fn test_ack_timeout_sends_ack<R: ReceiverBackend>() {
        let mut rx = R::new(make_delayed_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1, 2, 3],
            &mut a,
        )
        .unwrap();
        rx.handle_ack(&mut a);
        assert!(
            a.iter()
                .any(|a| matches!(a, ReceiverAction::SendAck { .. }))
        );
    }

    fn test_receive_single_packet<R: ReceiverBackend>() {
        let mut rx = R::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1, 2, 3, 4, 5],
            &mut a,
        )
        .unwrap();
        assert!(a.iter().any(|a| matches!(a, ReceiverAction::MessageReady)));
        assert_eq!(rx.take_message().unwrap(), &[1, 2, 3, 4, 5]);
    }

    fn test_out_of_order_delivery<R: ReceiverBackend>() {
        let mut rx = R::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();

        rx.handle_data(
            SequenceCount::from(1),
            SequenceFlag::Unsegmented,
            &[2],
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());

        let bmp = a.iter().find_map(|a| {
            if let ReceiverAction::SendAck {
                selective_bitmap, ..
            } = a
            {
                Some(*selective_bitmap)
            } else {
                None
            }
        });
        assert_eq!(bmp, Some(0b0001));

        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1],
            &mut a,
        )
        .unwrap();

        let cnt = a
            .iter()
            .filter(|a| matches!(a, ReceiverAction::MessageReady))
            .count();
        assert_eq!(cnt, 2);
        assert!(rx.has_message());
    }

    fn test_duplicate_ignored<R: ReceiverBackend>() {
        let mut rx = R::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1],
            &mut a,
        )
        .unwrap();
        rx.take_message();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[99],
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());
    }

    fn test_progress_timeout_skips_gap<R: ReceiverBackend>() {
        let mut rx = R::new(make_progress_config(50), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1],
            &mut a,
        )
        .unwrap();
        rx.take_message();
        rx.handle_data(
            SequenceCount::from(2),
            SequenceFlag::Unsegmented,
            &[3],
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());
        rx.handle_timeout(&mut a).unwrap();
        assert_eq!(rx.expected_seq().value(), 3);
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &[3]);
    }

    fn test_no_progress_timeout_in_reliable_mode<R: ReceiverBackend>() {
        let mut rx = R::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1],
            &mut a,
        )
        .unwrap();
        rx.take_message();
        rx.handle_data(
            SequenceCount::from(2),
            SequenceFlag::Unsegmented,
            &[3],
            &mut a,
        )
        .unwrap();
        assert!(
            !a.iter()
                .any(|a| matches!(a, ReceiverAction::StartProgressTimer { .. }))
        );
    }

    fn test_progress_timer_resets_on_progress<R: ReceiverBackend>() {
        let mut rx = R::new(make_progress_config(50), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(1),
            SequenceFlag::Unsegmented,
            &[2],
            &mut a,
        )
        .unwrap();
        assert!(
            a.iter()
                .any(|a| matches!(a, ReceiverAction::StartProgressTimer { ticks: 50 }))
        );
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[1],
            &mut a,
        )
        .unwrap();
        assert!(
            a.iter()
                .any(|a| matches!(a, ReceiverAction::StopProgressTimer))
        );
    }

    // ── Segmented tests (Packed + Fast only — Lite tiles at MTU boundaries) ──

    fn test_segmented_message<R: ReceiverBackend>() {
        let mut rx = R::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::First,
            &[1, 2, 3],
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());
        rx.handle_data(
            SequenceCount::from(1),
            SequenceFlag::Continuation,
            &[4, 5, 6],
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());
        rx.handle_data(SequenceCount::from(2), SequenceFlag::Last, &[7, 8], &mut a)
            .unwrap();
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    fn test_progress_timeout_discards_partial<R: ReceiverBackend>() {
        let mut rx = R::new(make_progress_config(50), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::First,
            &[1, 2, 3],
            &mut a,
        )
        .unwrap();
        rx.handle_data(
            SequenceCount::from(3),
            SequenceFlag::Unsegmented,
            &[10, 11],
            &mut a,
        )
        .unwrap();
        assert!(!rx.has_message());
        rx.handle_timeout(&mut a).unwrap();
        assert!(!rx.has_message());
        rx.handle_timeout(&mut a).unwrap();
        assert!(rx.has_message());
        assert_eq!(rx.take_message().unwrap(), &[10, 11]);
        rx.handle_data(
            SequenceCount::from(4),
            SequenceFlag::Unsegmented,
            &[20, 21],
            &mut a,
        )
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
    // (Lite tiles segments at MTU boundaries, so sub-MTU payloads
    // produce a different reassembled layout).
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
        let mut rx: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[42],
            &mut a,
        )
        .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[42]);
    }

    #[test]
    fn fast_receiver_seq_wraparound() {
        let mut rx: FastReceiver<8, 512, 8192, 4096> =
            FastReceiver::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();

        for i in 0..SequenceCount::MAX {
            rx.handle_data(
                SequenceCount::from(i),
                SequenceFlag::Unsegmented,
                &[i as u8],
                &mut a,
            )
            .unwrap();
            rx.take_message();
        }

        rx.handle_data(
            SequenceCount::from(SequenceCount::MAX),
            SequenceFlag::Unsegmented,
            &[0xFF],
            &mut a,
        )
        .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[0xFF]);

        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &[0x00],
            &mut a,
        )
        .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[0x00]);
        assert_eq!(rx.expected_seq().value(), 1);
    }

    #[test]
    fn packed_receiver_slab_reset() {
        let mut rx: PackedReceiver<8, 128, 8192> =
            PackedReceiver::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();

        let big = [0xAA; 60];
        rx.handle_data(
            SequenceCount::from(1),
            SequenceFlag::Unsegmented,
            &big,
            &mut a,
        )
        .unwrap();
        rx.handle_data(
            SequenceCount::from(0),
            SequenceFlag::Unsegmented,
            &big,
            &mut a,
        )
        .unwrap();
        rx.take_message();
        rx.take_message();

        rx.handle_data(
            SequenceCount::from(3),
            SequenceFlag::Unsegmented,
            &big,
            &mut a,
        )
        .unwrap();
        rx.handle_data(
            SequenceCount::from(2),
            SequenceFlag::Unsegmented,
            &big,
            &mut a,
        )
        .unwrap();
        assert!(rx.has_message());
    }
}
