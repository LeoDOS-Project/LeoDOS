//! A simple, single-threaded async runtime for `no_std` cFS applications.
//!
//! This module provides an ergonomic runtime that integrates with the cFS
//! scheduler. The primary entry point is the [`Runtime`] struct.
//!
//! # Usage
//!
//! The entire application, from initialization to the main processing loops,
//! can be defined within a single `async` block.
//!
//! ```rust,ignore
//! use leodos_libcfs::runtime::{Runtime, join};
//! use leodos_libcfs::cfe::{evs, sb::pipe::Pipe};
//!
//! async fn task_one(pipe: &Pipe) { /* ... */ }
//! async fn task_two() { /* ... */ }
//!
//! #[no_mangle]
//! pub extern "C" fn CFE_ES_Main() {
//!     Runtime::new().run(async {
//!         // Initialization and resource creation happens here.
//!         evs::event::register(&[]).expect("EVS registration failed");
//!         let pipe = Pipe::new("MY_PIPE", 16).expect("Pipe creation failed");
//!
//!         // The main application logic runs concurrently.
//!         // Variables from the init phase are captured automatically.
//!         join(task_one(&pipe), task_two()).await;
//!     });
//! }
//! ```

pub mod join;
pub mod select_either;
pub mod sync;
mod task;
pub mod time;
pub mod scope;
pub mod dyn_scope;

pub use futures::select_biased;
pub use futures::FutureExt;
pub use pin_utils::pin_mut;

use crate::cfe::es::app::RunStatus;
use crate::error::Result;
use crate::ffi;
use crate::log;
use core::future::Future;
use core::task::{RawWaker, RawWakerVTable, Waker};

/// An async runtime designed to integrate with the cFS application lifecycle.
///
/// The runtime drives a single `Future` to completion by polling it
/// every time the cFS scheduler wakes the application.
pub struct Runtime {
    _private: (),
}

impl Runtime {
    /// Creates a new cFS async runtime.
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Runs the main async task for the application.
    pub fn run<F>(self, main_future: F) -> !
    where
        F: Future<Output = Result<()>>,
    {
        // Pin the future on the stack so it can be polled.
        pin_mut!(main_future);

        // Create the waker and context needed to poll the future.
        let waker = noop_waker();
        let mut context = core::task::Context::from_waker(&waker);

        loop {
            let mut status = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_APP_RUN;
            let should_run = unsafe { ffi::CFE_ES_RunLoop(&mut status) };

            match RunStatus::from(status) {
                RunStatus::Run if should_run => {
                    if main_future.as_mut().poll(&mut context).is_ready() {
                        log!("Top-level async task finished. Exiting.").ok();
                        break;
                    }
                }
                _ => {
                    // cFE has commanded us to shut down.
                    log!("Received command to exit application.").ok();
                    break;
                }
            }
        }

        // Perform a graceful exit.
        unsafe { ffi::CFE_ES_ExitApp(RunStatus::Exit as u32) };

        // CFE_ES_ExitApp may not be immediate. Loop forever to ensure
        // this function's `!` return type is satisfied.
        loop {}
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// A waker that does nothing, since our executor polls continuously.
/// The CFE scheduler controls when we should poll. For example, our loop will run every 100ms.
fn noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RawWaker::new(core::ptr::null(), &VTABLE), // clone
        |_| {},                                        // wake
        |_| {},                                        // wake_by_ref
        |_| {},                                        // drop
    );
    unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}
