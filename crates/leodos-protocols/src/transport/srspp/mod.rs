//! SRSP: Simple Reliable Space Packet Protocol
//!
//! A lightweight reliable transport protocol built on CCSDS Space Packets.
//! Designed for point-to-point satellite communication where link latency
//! is predictable and congestion control is unnecessary.
//!
//! SRSP reuses the Space Packet sequence count and segmentation flags,
//! adding only acknowledgment and retransmission mechanisms.

pub mod machine;
pub mod api;
pub mod packet;
