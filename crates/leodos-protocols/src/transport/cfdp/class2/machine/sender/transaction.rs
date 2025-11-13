//! Contains the transaction data structures specific to the sender state machine.

use core::ops::Deref;

use crate::transport::cfdp::class2::machine::transaction::TransactionConfig;

/// The lifecycle state of a sending transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// The sender is actively sending `FileData` PDUs.
    SendingFileData,
    /// The sender has sent all file data and is waiting for the file checksum.
    WaitingForChecksum,
    /// The sender has sent an `EOF` PDU and is waiting for the corresponding `ACK`.
    WaitingForEofAck,
    /// The sender has received the `ACK(EOF)` and is now waiting for the `Finished` PDU.
    WaitingForFinishedPdu,
}

/// Holds all the dynamic and static state for a single, ongoing sending transaction.
#[derive(Debug)]
pub struct Transaction {
    /// The shared, static configuration for this transaction.
    pub config: TransactionConfig,
    /// The number of bytes of the file that have been successfully sent.
    pub progress: u64,
    /// The last known progress of the receiver, as reported by `KeepAlive` PDUs.
    pub last_receiver_progress: u64,
    /// The byte discrepancy limit for Keep Alive checks.
    pub keep_alive_limit: u64,
    /// The calculated checksum of the source file. `Some` once calculated.
    pub file_checksum: Option<u32>,
    /// The number of times to retry sending an `EOF` before faulting.
    pub ack_limit: u8,
    /// The current position in the sender's lifecycle.
    pub state: TransactionState,
    /// Whether the transaction is currently suspended.
    pub suspended: bool,
    /// A counter for the number of times an `EOF` PDU has been retransmitted.
    pub ack_retries: u8,
}

impl Deref for Transaction {
    type Target = TransactionConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl core::ops::DerefMut for Transaction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}
