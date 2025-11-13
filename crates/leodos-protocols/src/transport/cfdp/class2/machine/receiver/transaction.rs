//! Contains the transaction data structures specific to the receiver state machine.

use core::ops::Deref;
use core::ops::DerefMut;

use crate::transport::cfdp::class2::machine::tracker::SegmentTracker;
use crate::transport::cfdp::class2::machine::transaction::TransactionConfig;
use crate::transport::cfdp::pdu::tlv::filestore_request::FilestoreRequest;
use heapless::Vec;

/// The lifecycle state of a receiving transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// The receiver is actively receiving `FileData` PDUs.
    ReceivingFileData,
    /// The receiver has received the `EOF` PDU but has missing data, and is waiting for retransmissions.
    WaitingForNakData,
    /// The receiver has a complete file and is waiting for the Runner to verify the checksum.
    VerifyingChecksum,
    /// The receiver is waiting for the runner to execute filestore requests.
    WaitingForFilestoreResponses,
}

/// Holds all the dynamic and static state for a single, ongoing receiving transaction.
#[derive(Debug)]
pub struct Transaction {
    /// The data structure used to track missing file segments.
    pub tracker: SegmentTracker,
    /// The shared, static configuration for this transaction.
    pub config: TransactionConfig,
    /// Filestore requests to be processed by the Runner.
    pub filestore_requests: Vec<FilestoreRequest, 4, u8>,
    /// The number of bytes of the file that have been successfully received and written.
    pub progress: u64,
    /// Timeout in seconds to wait for missing data after sending a `NAK`.
    pub nak_timeout_secs: u16,
    /// The interval at which to send periodic `KeepAlive` PDUs.
    pub keep_alive_interval_secs: u16,
    /// The current position in the receiver's lifecycle.
    pub state: TransactionState,
    /// A counter for the number of times a `NAK` sequence has been retransmitted.
    pub nak_retries: u8,
    /// The number of times to retry sending a `NAK` sequence before faulting.
    pub nak_limit: u8,
    /// Whether the transaction is currently suspended.
    pub suspended: bool,
}

impl Deref for Transaction {
    type Target = TransactionConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl DerefMut for Transaction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}
