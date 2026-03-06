//! CCSDS Time Code Formats (CCSDS 301.0-B-4)
//!
//! This module implements the CCSDS Unsegmented Code (CUC), which is
//! the most common time format in CCSDS protocols. CUC represents
//! time as a binary count of seconds (and fractional seconds) since
//! a configurable epoch.

/// CCSDS Unsegmented Code (CUC) time format.
pub mod cuc;

pub use cuc::CucTime;
