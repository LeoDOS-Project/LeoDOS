//! SRSP: Simple Reliable Space Packet Protocol
//!
//! A lightweight reliable transport protocol built on CCSDS Space Packets.
//! Designed for point-to-point satellite communication where link latency
//! is predictable and congestion control is unnecessary.
//!
//! SRSP reuses the Space Packet sequence count and segmentation flags,
//! adding only acknowledgment and retransmission mechanisms.

/// Sender and receiver state machines.
pub mod machine;
/// Async API layer over the state machines.
pub mod api;
/// Packet types, builders, and parsers.
pub mod packet;
/// Retransmission timeout policies.
pub mod rto;
/// Delay-tolerant delivery traits.
pub mod dtn;
