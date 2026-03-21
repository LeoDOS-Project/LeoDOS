//! CCSDS Time Code Formats (CCSDS 301.0-B-4)
//!
//! Implements two CCSDS time formats:
//! - **CUC** (Unsegmented Code): binary seconds + fractional seconds
//! - **CDS** (Day Segmented): day count + milliseconds of day

/// CCSDS Unsegmented Code (CUC) time format.
pub mod cuc;
/// CCSDS Day Segmented (CDS) time format.
pub mod cds;

pub use cds::CdsTime;
pub use cuc::CucTime;
