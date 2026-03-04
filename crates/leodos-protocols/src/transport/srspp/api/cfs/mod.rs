mod node;
mod receiver;
mod sender;

pub use node::{SrsppNode, SrsppNodeDriver};
pub use receiver::{SrsppReceiver, SrsppReceiverDriver, SrsppRxHandle};
pub use sender::{SrsppSender, SrsppSenderDriver, SrsppTxHandle};

use leodos_libcfs::cfe::time::SysTime;

use crate::transport::srspp::machine::receiver::ReceiverError;
use crate::transport::srspp::machine::sender::SenderError;
use crate::transport::srspp::packet;

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

struct TimerSet<const N: usize> {
    timers: [(u16, Option<SysTime>); N],
}

impl<const N: usize> TimerSet<N> {
    fn new() -> Self {
        Self {
            timers: [(0, None); N],
        }
    }

    fn start(&mut self, seq: u16, deadline: SysTime) {
        for slot in &mut self.timers {
            if slot.1.is_none() {
                *slot = (seq, Some(deadline));
                return;
            }
        }
    }

    fn stop(&mut self, seq: u16) {
        for slot in &mut self.timers {
            if slot.0 == seq && slot.1.is_some() {
                slot.1 = None;
            }
        }
    }

    fn expired(&mut self, now: SysTime) -> impl Iterator<Item = u16> + '_ {
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

    fn next_deadline(&self) -> Option<SysTime> {
        self.timers
            .iter()
            .filter_map(|(_, deadline)| *deadline)
            .min()
    }
}
