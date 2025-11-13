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

pub mod class2;
pub mod filestore;
pub mod pdu;
pub mod checksum;

#[derive(Debug, PartialEq, Eq)]
pub enum CfdpError {
    // Build errors
    BufferTooSmall { required: usize, provided: usize },
    DataTooLarge { field: &'static str, max: usize },
    IdLengthInvalid { field: &'static str, len: usize },
    IdLengthMismatch,
    // Other errors
    Custom(&'static str),
    TransactionNotFound,
    TooManyConcurrentTransactions,
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
