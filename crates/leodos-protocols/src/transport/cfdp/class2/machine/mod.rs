//! The synchronous, core CFDP state machine.
//!
//! This module contains the pure, platform-agnostic logic for the CFDP protocol.
//! It operates by receiving `Event`s and producing `Action`s, without performing
//! any I/O itself.

pub use self::receiver::ReceiverMachine;
pub use self::sender::SenderMachine;
pub use self::transaction::TransactionId;

pub mod receiver;
pub mod sender;
pub mod tracker;
pub mod transaction;

/// The maximum number of actions that can be generated from a single event.
pub const MAX_ACTIONS_PER_EVENT: usize = 8;
/// The maximum number of concurrent transactions supported.
pub const MAX_CONCURRENT_TRANSACTIONS: usize = 8;
/// The maximum size of a file data chunk to send in one PDU.
pub const FILE_DATA_CHUNK_SIZE: usize = 2048;

/// The specific type of timer to be managed.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum TimerType {
    Ack,
    Nak,
    Inactivity,
    KeepAlive,
}

#[derive(Debug)]
pub enum PromptType {
    Nak,
    KeepAlive,
}
