//! CCSDS File Delivery Protocol (CFDP) Protocol
//!
//! Spec: https://ccsds.org/Pubs/727x0b5e1.pdf
//!
//! This module provides a complete CFDP implementation with both low-level
//! state machines and high-level async APIs.
//!
//! # Architecture
//!
//! The implementation is split into layers:
//!
//! - [`pdu`]: PDU parsing and serialization
//! - [`machine`]: Pure, synchronous state machines (sender and receiver)
//! - [`filestore`]: Abstract file I/O trait
//! - [`api`]: High-level async API with explicit sender/receiver roles
//!
//! # Features
//!
//! - `tokio`: Enables async API with tokio runtime support
//! - `cfs`: Enables async API with leodos-libcfs runtime support
//!
//! # Example (tokio)
//!
//! ```rust,ignore
//! use leodos_ccsds::cfdp::api::net::{CfdpSender, CfdpReceiver};
//!
//! let mut sender = CfdpSender::new(entity_id);
//! let stream = sender.put("file.txt", "remote.txt", dest_id, addr).await?;
//!
//! let mut receiver = CfdpReceiver::new(entity_id);
//! let file = receiver.accept().await;
//! ```

/// Class 2 (acknowledged) CFDP state machines and transaction management.
pub mod class2;
/// Abstract file I/O trait for platform-independent file operations.
pub mod filestore;
/// PDU parsing, serialization, and zero-copy views.
pub mod pdu;
/// Checksum algorithms for CFDP data integrity verification.
pub mod checksum;

/// Errors that can occur during CFDP operations.
#[derive(Debug, PartialEq, Eq)]
pub enum CfdpError {
    // Build errors
    /// The provided buffer is too small for the required data.
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required: usize,
        /// Actual buffer size provided.
        provided: usize,
    },
    /// A field's data exceeds its maximum allowed size.
    DataTooLarge {
        /// Name of the field that exceeded its limit.
        field: &'static str,
        /// Maximum allowed size in bytes.
        max: usize,
    },
    /// An entity or sequence number ID has an invalid length.
    IdLengthInvalid {
        /// Name of the field with the invalid ID length.
        field: &'static str,
        /// The invalid length that was provided.
        len: usize,
    },
    /// Source and destination entity ID lengths do not match.
    IdLengthMismatch,
    // Other errors
    /// A custom error with a static message.
    Custom(&'static str),
    /// The referenced transaction was not found.
    TransactionNotFound,
    /// The maximum number of concurrent transactions has been reached.
    TooManyConcurrentTransactions,
    /// The action buffer is full and cannot accept more actions.
    ActionBufferFull,
}

impl core::fmt::Display for CfdpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CfdpError::BufferTooSmall { required, provided } => {
                write!(
                    f,
                    "Buffer too small: required {} bytes, provided {} bytes",
                    required, provided
                )
            }
            CfdpError::DataTooLarge { field, max } => {
                write!(f, "Data too large for field '{}': max {} bytes", field, max)
            }
            CfdpError::IdLengthInvalid { field, len } => {
                write!(
                    f,
                    "Invalid ID length for field '{}': got {} bytes",
                    field, len
                )
            }
            CfdpError::IdLengthMismatch => {
                write!(f, "Source and destination entity ID lengths do not match")
            }
            CfdpError::Custom(msg) => write!(f, "CFDP Error: {}", msg),
            CfdpError::TransactionNotFound => write!(f, "CFDP Error: Transaction not found"),
            CfdpError::TooManyConcurrentTransactions => {
                write!(f, "CFDP Error: Too many concurrent transactions")
            }
            CfdpError::ActionBufferFull => write!(f, "CFDP Error: Action buffer full"),
        }
    }
}

// #[cfg(feature = "async")]
// pub mod api;
