//! CCSDS Communications Operation Procedure-1 (COP-1).
//!
//! Spec: CCSDS 232.1-B-2 (<https://ccsds.org/Pubs/232x1b2e2c1.pdf>).
//!
//! COP-1 provides hop-by-hop reliable delivery of TC transfer frames
//! over a single link using go-back-N ARQ. It consists of:
//!
//! - **FOP-1** (sender): sequences AD frames, retransmits on NACK/timeout.
//! - **FARM-1** (receiver): validates sequence numbers, generates CLCWs.
//! - **CLCW**: 32-bit feedback word carried in TM/AOS frames.

pub mod clcw;
pub mod farm;
pub mod fop;
