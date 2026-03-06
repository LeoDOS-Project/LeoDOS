/// Combined sender/receiver SRSPP node.
mod node;
/// Multi-stream SRSPP receiver.
mod receiver;
/// SRSPP sender with retransmission support.
mod sender;

pub use node::{SrsppNode, SrsppNodeDriver};
pub use receiver::{DeliveryToken, SrsppReceiver, SrsppReceiverDriver, SrsppRxHandle};
pub use sender::{SrsppSender, SrsppSenderDriver, SrsppTxHandle};

use core::cell::RefCell;

use leodos_libcfs::cfe::time::SysTime;

use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::receiver::ReceiverError;
use crate::transport::srspp::machine::sender::SenderError;
use crate::transport::srspp::packet;

/// Interior-mutable cell that only allows access through sync closures.
///
/// Wraps `RefCell<T>` but restricts borrows to non-`async` closures,
/// making it a compile error to hold a borrow across an `.await` point.
pub(super) struct SharedCell<T>(RefCell<T>);

impl<T> SharedCell<T> {
    pub(super) fn new(val: T) -> Self {
        Self(RefCell::new(val))
    }

    pub(super) fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        f(&self.0.borrow())
    }

    pub(super) fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        f(&mut self.0.borrow_mut())
    }
}

/// Errors from the SRSPP CFS transport layer.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error<E> {
    /// The sender state machine reported an error.
    #[error(transparent)]
    Sender(#[from] SenderError),
    /// The receiver state machine reported an error.
    #[error(transparent)]
    Receiver(#[from] ReceiverError),
    /// The underlying network link failed.
    #[error("link error: {0}")]
    Link(E),
    /// A packet could not be built or parsed.
    #[error(transparent)]
    Packet(#[from] packet::SrsppPacketError),
}

/// Fixed-capacity set of retransmission timers keyed by sequence number.
struct TimerSet<const N: usize> {
    /// Array of (sequence number, optional deadline) slots.
    timers: [(SequenceCount, Option<SysTime>); N],
}

impl<const N: usize> TimerSet<N> {
    /// Creates an empty timer set.
    fn new() -> Self {
        Self {
            timers: [(SequenceCount::new(), None); N],
        }
    }

    /// Starts a timer for the given sequence number with the specified deadline.
    fn start(&mut self, seq: SequenceCount, deadline: SysTime) {
        for slot in &mut self.timers {
            if slot.1.is_none() {
                *slot = (seq, Some(deadline));
                return;
            }
        }
    }

    /// Cancels the timer for the given sequence number.
    fn stop(&mut self, seq: SequenceCount) {
        for slot in &mut self.timers {
            if slot.0 == seq && slot.1.is_some() {
                slot.1 = None;
            }
        }
    }

    /// Returns an iterator of sequence numbers whose timers have expired.
    fn expired(&mut self, now: SysTime) -> impl Iterator<Item = SequenceCount> + '_ {
        self.timers.iter_mut().filter_map(move |slot| {
            if let Some(deadline) = slot.1 {
                if now >= deadline {
                    slot.1 = None;
                    return Some(slot.0);
                }
            }
            None
        })
    }

    /// Returns the earliest active deadline, if any.
    fn next_deadline(&self) -> Option<SysTime> {
        self.timers
            .iter()
            .filter_map(|(_, deadline)| *deadline)
            .min()
    }
}
