//! An implementation of CCSDS protocols for space communications.
#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
// TODO: work toward #![deny(missing_docs)] — ~1045 items remain

pub mod coding;
pub mod datalink;
pub mod application;
pub mod network;
pub mod transport;
pub mod utils;
