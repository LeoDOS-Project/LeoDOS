#![cfg_attr(not(feature = "std"), no_std)]
//! # leodos-libcfs: A Safe Rust Wrapper for the Core Flight System
//!
//! `leodos-libcfs` provides safe, idiomatic, and zero-cost wrappers around the C APIs of the
//! NASA Core Flight System (cFS), including the Core Flight Executive (CFE),
//! Operating System Abstraction Layer (OSAL), and Platform Support Package (PSP).
//!
//! This crate is designed to enable the development of cFS applications in Rust
//! with a high degree of safety and ergonomics, leveraging Rust's ownership,
//! borrowing, and type system to prevent common errors found in C-based cFS development.
//!
//! ## Key Features
//!
//! - **Safe Abstractions**: RAII guards for resource management (e.g., `Pipe`, `Table`, `Mutex`),
//!   preventing resource leaks.
//! - **Type Safety**: Generic wrappers for message passing (`Queue<T>`), tables (`Table<T>`),
//!   and critical data stores (`CdsBlock<T>`) ensure data integrity at compile time.
//! - **Ergonomic API**: A high-level application framework (`app::App` builder) simplifies
//!   the boilerplate of a cFS application.
//! - **Comprehensive Coverage**: Wrappers for major cFE services (ES, EVS, SB, TBL, TIME)
//!   and key OSAL/PSP functionalities.
#![deny(missing_docs)]

pub(crate) mod ffi;
pub mod cfe;
pub mod error;
pub mod log;
pub mod os;
pub mod psp;
pub mod status;
pub mod runtime;
pub mod macros;
pub mod app;
#[cfg(feature = "cfdp")]
pub mod cf;
#[cfg(feature = "nos3")]
pub mod nos3;
