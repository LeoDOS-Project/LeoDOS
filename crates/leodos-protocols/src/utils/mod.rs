//! Shared utilities: bitfield helpers, checksums, formatting, time.

/// Bitfield extraction, checksums, and header trait.
pub mod bits;
/// `no_std` formatting utilities.
pub mod fmt;
/// CCSDS time code formats (CCSDS 301.0-B-4).
pub mod time;
/// Interior-mutable cell that only allows access through sync closures.
pub mod cell;
/// Monotonic clock abstraction.
pub mod clock;
/// Fixed-capacity byte ring buffer for variable-length packets.
pub mod ringbuf;

pub use bits::*;
pub use fmt::*;
