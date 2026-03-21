//! An implementation of CCSDS protocols for space communications.
#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Application layer protocols and services.
pub mod application;
/// Channel coding, CRC, randomization, and physical layer protocols.
pub mod coding;
/// Protocol identifier newtypes (SCID, VCID, etc.).
pub mod ids;
/// Data link layer framing and transfer frame protocols.
pub mod datalink;
/// Network layer protocols including Space Packet and ISL routing.
pub mod network;
/// Physical layer: modulation, demodulation, channel models.
pub mod physical;
/// Transport layer protocols.
pub mod transport;
/// Miscellaneous CCSDS protocols: SLE, time codes, etc.
pub mod misc;
/// Shared utilities: bitfield helpers and checksums.
pub mod utils;
