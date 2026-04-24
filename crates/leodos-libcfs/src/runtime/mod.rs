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
//! use leodos_libcfs::join;
//! use leodos_libcfs::runtime::Runtime;
//! use leodos_libcfs::cfe::{evs, sb::pipe::Pipe};
//!
//! async fn task_one(pipe: &Pipe) { /* ... */ }
//! async fn task_two() { /* ... */ }
//!
//! #[no_mangle]
//! pub extern "C" fn CFE_ES_Main() {
//!     Runtime::new().run(async {
//!         evs::event::register(&[]).expect("EVS registration failed");
//!         let pipe = Pipe::new("MY_PIPE", 16).expect("Pipe creation failed");
//!
//!         join!(task_one(&pipe), task_two()).await;
//!     });
//! }
//! ```

pub mod dyn_scope;
pub mod join;
pub mod reactor;
pub mod scope;
pub mod select_either;
pub mod sync;
mod task;
pub mod time;

pub use futures::select_biased;
pub use futures::FutureExt;
pub use pin_utils::pin_mut;

use crate::cfe::es::app;
use crate::cfe::es::perf::PerfMarker;
use crate::log;
use core::future::Future;

/// Default timeout passed to `OS_SelectMultiple` when the task is
/// idle. Bounds how long a `Sleep` or other non-fd future waits
/// before being re-polled.
const REACTOR_TIMEOUT_MS: i32 = 50;

/// An async runtime designed to integrate with the cFS application lifecycle.
///
/// The runtime drives a single `Future` to completion by polling it
/// every time the cFS scheduler wakes the application.
pub struct Runtime {
    perf_id: Option<u32>,
}

impl Runtime {
    /// Creates a new cFS async runtime.
    pub fn new() -> Self {
        Self { perf_id: None }
    }

    /// Sets the performance monitor ID for this app.
    ///
    /// When set, the runtime automatically logs
    /// `PerfLogEntry`/`PerfLogExit` around each poll cycle.
    pub fn perf_id(mut self, id: u32) -> Self {
        self.perf_id = Some(id);
        self
    }

    /// Runs the main async task for the application.
    ///
    /// Polls the future until it completes or cFS commands
    /// the app to exit. All resources owned by the future
    /// are dropped before `CFE_ES_ExitApp` is called.
    pub fn run(self, main_future: impl Future) -> ! {
        let status = self.poll_until_done(main_future);
        app::exit_app(status);
    }

    /// Polls the future in the cFS run loop until completion
    /// or shutdown. Returns the exit status to pass to
    /// `exit_app`. The future and all its owned resources
    /// are dropped when this function returns.
    fn poll_until_done(self, main_future: impl Future) -> app::RunStatus {
        pin_mut!(main_future);

        let reactor = reactor::Reactor::new();
        // SAFETY: `reactor` lives for the whole of this function,
        // and no clones of `waker` escape it.
        let waker = unsafe { reactor::waker_from_reactor(&reactor) };
        let mut context = core::task::Context::from_waker(&waker);

        loop {
            match app::run_loop() {
                Ok(()) => {
                    let _perf = self.perf_id.map(PerfMarker::new);
                    if main_future.as_mut().poll(&mut context).is_ready() {
                        log!("Async task finished.").ok();
                        return app::RunStatus::Exit;
                    }
                    if reactor.was_woken() {
                        // An in-process waker fired during the
                        // poll; re-poll immediately without
                        // blocking.
                        continue;
                    }
                    reactor.block(REACTOR_TIMEOUT_MS);
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
