//! General-purpose `no_std` utilities for LeoDOS.
//!
//! Contains async primitives and data structures shared
//! across crates. Unsafe is allowed here for foundational
//! pinning and cell types.

#![no_std]

pub mod future_pool;
pub mod lending_iterator;
