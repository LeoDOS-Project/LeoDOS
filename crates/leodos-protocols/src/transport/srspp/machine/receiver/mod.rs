//! Receiver state machine for SRSPP.
//!
//! Three backends behind the same public API:
//! - [`ReceiverA`]: indexed slots (`seq % WIN`), fixed MTU-sized slots
//! - [`ReceiverB`]: gap-tracked contiguous buffer, direct byte placement
//! - [`ReceiverC`]: indexed slab, append-only bump allocator
//!
//! [`ReceiverMachine`] is a type alias for [`ReceiverC`].

mod primitives;
mod shell;

/// Backend A: indexed slots with fixed MTU-sized payloads.
pub mod indexed_slots;
/// Backend B: gap-tracked contiguous reassembly buffer.
pub mod gap_tracked;
/// Backend C: indexed slab with append-only bump allocator.
pub mod indexed_slab;

pub use gap_tracked::ReceiverB;
pub use indexed_slab::ReceiverC;
pub use indexed_slots::ReceiverA;

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

/// Receiver state machine (alias for [`ReceiverC`]).
///
/// Handles reordering, reassembly, and ACK generation.
///
/// * `WIN` — Maximum number of out-of-order packets to buffer
/// * `BUF` — Total reorder buffer size in bytes
/// * `REASM` — Maximum reassembled message size
pub type ReceiverMachine<
    const WIN: usize,
    const BUF: usize,
    const REASM: usize,
> = ReceiverC<WIN, BUF, REASM>;

/// Trait abstracting over receiver backends.
pub trait ReceiverBackend: Sized {
    /// Create a new receiver for a specific remote sender.
    fn new(
        config: ReceiverConfig,
        remote_address: Address,
    ) -> Self;
    /// Get the remote address.
    fn remote_address(&self) -> Address;
    /// Process an event and produce actions.
    fn handle(
        &mut self,
        event: ReceiverEvent<'_>,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError>;
    /// Take the complete message.
    fn take_message(&mut self) -> Option<&[u8]>;
    /// Returns a slice of the reassembly buffer.
    fn reassembly_data(&self, len: usize) -> &[u8];
    /// Check if there's a complete message ready.
    fn has_message(&self) -> bool;
    /// Get the current expected sequence number.
    fn expected_seq(&self) -> SequenceCount;
}

impl<
    const WIN: usize,
    const MTU: usize,
    const REASM: usize,
    const TOTAL: usize,
> ReceiverBackend for ReceiverA<WIN, MTU, REASM, TOTAL>
{
    fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self::new(config, remote_address)
    }
    fn remote_address(&self) -> Address {
        self.remote_address()
    }
    fn handle(
        &mut self,
        event: ReceiverEvent<'_>,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        self.handle(event, actions)
    }
    fn take_message(&mut self) -> Option<&[u8]> {
        self.take_message()
    }
    fn reassembly_data(&self, len: usize) -> &[u8] {
        self.reassembly_data(len)
    }
    fn has_message(&self) -> bool {
        self.has_message()
    }
    fn expected_seq(&self) -> SequenceCount {
        self.expected_seq()
    }
}

impl<const REASM: usize, const WIN: usize, const MTU: usize>
    ReceiverBackend for ReceiverB<REASM, WIN, MTU>
{
    fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self::new(config, remote_address)
    }
    fn remote_address(&self) -> Address {
        self.remote_address()
    }
    fn handle(
        &mut self,
        event: ReceiverEvent<'_>,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        self.handle(event, actions)
    }
    fn take_message(&mut self) -> Option<&[u8]> {
        self.take_message()
    }
    fn reassembly_data(&self, len: usize) -> &[u8] {
        self.reassembly_data(len)
    }
    fn has_message(&self) -> bool {
        self.has_message()
    }
    fn expected_seq(&self) -> SequenceCount {
        self.expected_seq()
    }
}

impl<const WIN: usize, const BUF: usize, const REASM: usize>
    ReceiverBackend for ReceiverC<WIN, BUF, REASM>
{
    fn new(config: ReceiverConfig, remote_address: Address) -> Self {
        Self::new(config, remote_address)
    }
    fn remote_address(&self) -> Address {
        self.remote_address()
    }
    fn handle(
        &mut self,
        event: ReceiverEvent<'_>,
        actions: &mut ReceiverActions,
    ) -> Result<(), ReceiverError> {
        self.handle(event, actions)
    }
    fn take_message(&mut self) -> Option<&[u8]> {
        self.take_message()
    }
    fn reassembly_data(&self, len: usize) -> &[u8] {
        self.reassembly_data(len)
    }
    fn has_message(&self) -> bool {
        self.has_message()
    }
    fn expected_seq(&self) -> SequenceCount {
        self.expected_seq()
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

    macro_rules! receiver_tests {
        ($mod_name:ident, $ty:ty, $new:expr) => {
            mod $mod_name {
                use super::*;

                fn make(cfg: ReceiverConfig) -> $ty {
                    $new(cfg, test_remote_address())
                }

                #[test]
                fn immediate_ack() {
                    let mut rx = make(make_config());
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1, 2, 3],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(a.iter().any(
                        |a| matches!(a, ReceiverAction::SendAck { .. })
                    ));
                }

                #[test]
                fn delayed_ack_starts_timer() {
                    let mut rx = make(make_delayed_config());
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1, 2, 3],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(a.iter().any(|a| matches!(
                        a,
                        ReceiverAction::StartAckTimer { ticks: 20 }
                    )));
                    assert!(!a
                        .iter()
                        .any(|a| matches!(
                            a,
                            ReceiverAction::SendAck { .. }
                        )));
                }

                #[test]
                fn ack_timeout_sends_ack() {
                    let mut rx = make(make_delayed_config());
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1, 2, 3],
                        },
                        &mut a,
                    )
                    .unwrap();
                    rx.handle(ReceiverEvent::AckTimeout, &mut a)
                        .unwrap();
                    assert!(a.iter().any(
                        |a| matches!(a, ReceiverAction::SendAck { .. })
                    ));
                }

                #[test]
                fn receive_single_packet() {
                    let mut rx = make(make_config());
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1, 2, 3, 4, 5],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(a
                        .iter()
                        .any(|a| matches!(a, ReceiverAction::MessageReady)));
                    assert_eq!(
                        rx.take_message().unwrap(),
                        &[1, 2, 3, 4, 5]
                    );
                }

                #[test]
                fn out_of_order_delivery() {
                    let mut rx = make(make_config());
                    let mut a = ReceiverActions::new();

                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(1),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[2],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(!rx.has_message());

                    let bmp = a.iter().find_map(|a| {
                        if let ReceiverAction::SendAck {
                            selective_bitmap,
                            ..
                        } = a
                        {
                            Some(*selective_bitmap)
                        } else {
                            None
                        }
                    });
                    assert_eq!(bmp, Some(0b0001));

                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1],
                        },
                        &mut a,
                    )
                    .unwrap();

                    let cnt = a
                        .iter()
                        .filter(|a| {
                            matches!(a, ReceiverAction::MessageReady)
                        })
                        .count();
                    assert_eq!(cnt, 2);
                    assert!(rx.has_message());
                }

                #[test]
                fn segmented_message() {
                    let mut rx = make(make_config());
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::First,
                            payload: &[1, 2, 3],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(!rx.has_message());
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(1),
                            flags: SequenceFlag::Continuation,
                            payload: &[4, 5, 6],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(!rx.has_message());
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(2),
                            flags: SequenceFlag::Last,
                            payload: &[7, 8],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(rx.has_message());
                    assert_eq!(
                        rx.take_message().unwrap(),
                        &[1, 2, 3, 4, 5, 6, 7, 8]
                    );
                }

                #[test]
                fn duplicate_ignored() {
                    let mut rx = make(make_config());
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1],
                        },
                        &mut a,
                    )
                    .unwrap();
                    rx.take_message();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[99],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(!rx.has_message());
                }

                #[test]
                fn progress_timeout_skips_gap() {
                    let mut rx = make(make_progress_config(50));
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1],
                        },
                        &mut a,
                    )
                    .unwrap();
                    rx.take_message();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(2),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[3],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(!rx.has_message());
                    rx.handle(
                        ReceiverEvent::ProgressTimeout,
                        &mut a,
                    )
                    .unwrap();
                    assert_eq!(rx.expected_seq().value(), 3);
                    assert!(rx.has_message());
                    assert_eq!(rx.take_message().unwrap(), &[3]);
                }

                #[test]
                fn progress_timeout_discards_partial() {
                    let mut rx = make(make_progress_config(50));
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::First,
                            payload: &[1, 2, 3],
                        },
                        &mut a,
                    )
                    .unwrap();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(3),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[10, 11],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(!rx.has_message());
                    rx.handle(
                        ReceiverEvent::ProgressTimeout,
                        &mut a,
                    )
                    .unwrap();
                    assert!(!rx.has_message());
                    rx.handle(
                        ReceiverEvent::ProgressTimeout,
                        &mut a,
                    )
                    .unwrap();
                    assert!(rx.has_message());
                    assert_eq!(
                        rx.take_message().unwrap(),
                        &[10, 11]
                    );
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(4),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[20, 21],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(rx.has_message());
                    assert_eq!(
                        rx.take_message().unwrap(),
                        &[20, 21]
                    );
                }

                #[test]
                fn no_progress_timeout_in_reliable_mode() {
                    let mut rx = make(make_config());
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1],
                        },
                        &mut a,
                    )
                    .unwrap();
                    rx.take_message();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(2),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[3],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(!a.iter().any(|a| matches!(
                        a,
                        ReceiverAction::StartProgressTimer { .. }
                    )));
                }

                #[test]
                fn progress_timer_resets_on_progress() {
                    let mut rx = make(make_progress_config(50));
                    let mut a = ReceiverActions::new();
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(1),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[2],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(a.iter().any(|a| matches!(
                        a,
                        ReceiverAction::StartProgressTimer {
                            ticks: 50
                        }
                    )));
                    rx.handle(
                        ReceiverEvent::DataReceived {
                            seq: SequenceCount::from(0),
                            flags: SequenceFlag::Unsegmented,
                            payload: &[1],
                        },
                        &mut a,
                    )
                    .unwrap();
                    assert!(a.iter().any(|a| matches!(
                        a,
                        ReceiverAction::StopProgressTimer
                    )));
                }
            }
        };
    }

    receiver_tests!(
        receiver_c_tests,
        ReceiverC<8, 4096, 8192>,
        ReceiverC::new
    );

    receiver_tests!(
        receiver_a_tests,
        ReceiverA<8, 512, 8192, 4096>,
        ReceiverA::new
    );

    // Legacy alias tests
    #[test]
    fn test_receiver_machine_alias() {
        let mut rx: ReceiverMachine<8, 4096, 8192> =
            ReceiverMachine::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();
        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(0),
                flags: SequenceFlag::Unsegmented,
                payload: &[42],
            },
            &mut a,
        )
        .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[42]);
    }

    #[test]
    fn receiver_a_seq_wraparound() {
        let mut rx: ReceiverA<8, 512, 8192, 4096> =
            ReceiverA::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();

        for i in 0..SequenceCount::MAX {
            rx.handle(
                ReceiverEvent::DataReceived {
                    seq: SequenceCount::from(i),
                    flags: SequenceFlag::Unsegmented,
                    payload: &[i as u8],
                },
                &mut a,
            )
            .unwrap();
            rx.take_message();
        }

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(SequenceCount::MAX),
                flags: SequenceFlag::Unsegmented,
                payload: &[0xFF],
            },
            &mut a,
        )
        .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[0xFF]);

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(0),
                flags: SequenceFlag::Unsegmented,
                payload: &[0x00],
            },
            &mut a,
        )
        .unwrap();
        assert_eq!(rx.take_message().unwrap(), &[0x00]);
        assert_eq!(rx.expected_seq().value(), 1);
    }

    #[test]
    fn receiver_c_slab_reset() {
        let mut rx: ReceiverC<8, 128, 8192> =
            ReceiverC::new(make_config(), test_remote_address());
        let mut a = ReceiverActions::new();

        let big = [0xAA; 60];
        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(1),
                flags: SequenceFlag::Unsegmented,
                payload: &big,
            },
            &mut a,
        )
        .unwrap();
        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(0),
                flags: SequenceFlag::Unsegmented,
                payload: &big,
            },
            &mut a,
        )
        .unwrap();
        rx.take_message();
        rx.take_message();

        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(3),
                flags: SequenceFlag::Unsegmented,
                payload: &big,
            },
            &mut a,
        )
        .unwrap();
        rx.handle(
            ReceiverEvent::DataReceived {
                seq: SequenceCount::from(2),
                flags: SequenceFlag::Unsegmented,
                payload: &big,
            },
            &mut a,
        )
        .unwrap();
        assert!(rx.has_message());
    }
}
