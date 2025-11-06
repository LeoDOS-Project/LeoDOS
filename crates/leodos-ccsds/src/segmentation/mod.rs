//! Helpers for segmentation and reassembly of large data blocks across multiple Space Packets.
//!
//! This module provides a high-level API to handle data that is too large to fit
//! into a single `SpacePacket`. This implementation is `#![no_std]` and allocator-free.
//! The user is responsible for providing buffers for the reassembly process.
//!
//! # Workflow
//!
//! **Sending (Segmentation):**
//! 1. Create a [`Segmenter`] iterator over your large data slice.
//! 2. The iterator yields [`SegmentedPacketData`] structs.
//! 3. Use the information from each struct to build and send a `SpacePacket`.
//!
//! **Receiving (Reassembly):**
//! 1. The user must manage a pool of [`Reassembler`] instances. The [`ReassemblyManager`]
//!    is a helper for this.
//! 2. When a `First` packet is received, associate a free `Reassembler` (and its buffer)
//!    with the packet's `Apid`.
//! 3. Feed subsequent packets for that `Apid` to the correct `Reassembler`.
//! 4. When the `Last` packet is processed, the `Reassembler` will yield a slice
//!    containing the complete data.

pub mod reassembler;
pub mod segmenter;
