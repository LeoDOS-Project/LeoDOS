//! OSAL (Operating System Abstraction Layer) interface.
//!
//! This module provides safe, idiomatic Rust wrappers for the OSAL API, which
//! abstracts away the details of the underlying real-time operating system (RTOS).

pub mod app;
pub mod fs;
pub mod heap;
pub mod id;
pub mod module;
pub mod net;
pub mod queue;
pub mod shell;
pub mod sync;
pub mod task;
pub mod time;
pub mod timebase;
pub mod timer;
pub(crate) mod util;
pub mod version;
