//! Top-level OSAL application lifecycle API.
//!
//! # Note
//!
//! These functions are typically part of the BSP/runtime and not called by
//! individual cFS applications, but are provided for completeness and for
//! special cases like unit testing environments.

use crate::error::Result;
use crate::ffi;
use crate::status::check;

/// Initializes the OS Abstraction Layer.
///
/// This must be called before any other OSAL routine. It is
/// typically handled by the cFE Main entry point.
///
/// Failure means subsequent OSAL calls have undefined behavior.
/// The typical response is to abort the application.
pub fn api_init() -> Result<()> {
    check(unsafe { ffi::OS_API_Init() })?;
    Ok(())
}

/// Tears down and de-initializes the OSAL.
///
/// This is best-effort — it may not recover all resources.
/// Intended for testing or controlled shutdown scenarios.
pub fn api_teardown() {
    unsafe { ffi::OS_API_Teardown() };
}

/// A background thread implementation that waits for events.
///
/// This is typically called by the BSP main routine after all
/// other initialization has taken place. It waits until
/// [`application_shutdown`] is called.
pub fn idle_loop() {
    unsafe { ffi::OS_IdleLoop() };
}

/// Deletes all resources (tasks, queues, etc.) created in OSAL.
///
/// Useful for cleaning up during an orderly shutdown or for testing.
pub fn delete_all_objects() {
    unsafe { ffi::OS_DeleteAllObjects() };
}

/// Initiates or cancels an orderly shutdown of the OSAL application.
///
/// Passing `true` initiates shutdown, waking the task currently
/// blocked in [`idle_loop`]. Passing `false` cancels a
/// previously-initiated shutdown.
pub fn application_shutdown(should_shutdown: bool) {
    unsafe { ffi::OS_ApplicationShutdown(should_shutdown as u8) };
}

/// Exits/aborts the entire application process immediately.
///
/// This function does not return and is typically only used in
/// non-production scenarios like unit testing.
pub fn application_exit(status: i32) -> ! {
    unsafe { ffi::OS_ApplicationExit(status) };
    loop {}
}
