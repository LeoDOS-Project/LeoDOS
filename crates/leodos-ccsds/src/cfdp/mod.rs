//! CCSDS File Delivery Protocol (CFDP) implementation.
//!
//! This module provides a portable, high-level, async implementation of CFDP
//! that is decoupled from the underlying OS and network stack.

pub mod filestore;
pub mod pdu;
pub mod machine;

// The high-level async API.
#[cfg(feature = "async")]
pub mod high_level;
