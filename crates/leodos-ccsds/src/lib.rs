//! An implementation of the CCSDS Space Packet library.
//! * Specification: https://ccsds.org/Pubs/133x0b2e2.pdf
#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::panic)]


pub mod cfdp;
pub mod cfe;
#[cfg(feature = "crc")]
pub mod crc;
pub mod datalink;
pub mod segmentation;
pub mod spp;
