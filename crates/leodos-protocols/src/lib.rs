//! An implementation of CCSDS protocols for space communications.
#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Channel coding, CRC, randomization, and physical layer protocols.
pub mod coding;
/// Data link layer framing and transfer frame protocols.
pub mod datalink;
/// Application layer protocols and services.
pub mod application;
/// Network layer protocols including Space Packet and ISL routing.
pub mod network;
/// Transport layer protocols.
pub mod transport;
/// Physical layer: modulation, demodulation, channel models.
pub mod physical;
/// Shared utilities: bitfield helpers, checksums, time formats.
pub mod utils;
