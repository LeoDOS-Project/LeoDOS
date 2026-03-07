//! Shared utilities: bitfield helpers, checksums, formatting, time.

/// Bitfield extraction, checksums, and header trait.
pub mod bits;
/// `no_std` formatting utilities.
pub mod fmt;
/// CCSDS time code formats (CCSDS 301.0-B-4).
pub mod time;

pub use bits::*;
pub use fmt::*;
