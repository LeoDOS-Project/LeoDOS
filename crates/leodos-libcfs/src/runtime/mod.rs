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

pub mod dyn_scope;
pub mod join;
pub mod scope;
pub mod select_either;
pub mod sync;
mod task;
pub mod time;

pub use futures::select_biased;
pub use futures::FutureExt;
pub use pin_utils::pin_mut;

use crate::cfe::es::app;
use crate::error::Result;
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
    ///
    /// Polls the future until it completes or cFS commands
    /// the app to exit. All resources owned by the future
    /// are dropped before `CFE_ES_ExitApp` is called.
    pub fn run(self, main_future: impl Future<Output = Result<()>>) -> ! {
        let status = self.poll_until_done(main_future);
        app::exit_app(status);
    }

    /// Polls the future in the cFS run loop until completion
    /// or shutdown. Returns the exit status to pass to
    /// `exit_app`. The future and all its owned resources
    /// are dropped when this function returns.
    fn poll_until_done(self, main_future: impl Future<Output = Result<()>>) -> app::RunStatus {
        pin_mut!(main_future);

        let waker = noop_waker();
        let mut context = core::task::Context::from_waker(&waker);

        loop {
            match app::run_loop() {
                Ok(()) => {
                    if main_future.as_mut().poll(&mut context).is_ready() {
                        log!("Async task finished.").ok();
                        return app::RunStatus::Exit;
                    }
                }
                Err(status) => {
                    log!("Exit requested.").ok();
                    return status;
                }
            }
        }
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
