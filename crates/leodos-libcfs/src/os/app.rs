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
/// # C-API Mapping
/// This is a safe wrapper for `OS_API_Init`.
///
/// This must be called before any other OSAL routine. It is typically handled
/// by the cFE Main entry point.
pub fn api_init() -> Result<()> {
    check(unsafe { ffi::OS_API_Init() })?;
    Ok(())
}

/// Tears down and de-initializes the OSAL.
///
/// # C-API Mapping
/// This is a safe wrapper for `OS_API_Teardown`.
///
/// This will release all OS resources and is intended for testing or controlled
/// shutdown scenarios.
pub fn api_teardown() {
    unsafe { ffi::OS_API_Teardown() };
}

/// A background thread implementation that waits for events.
///
/// # C-API Mapping
/// This is a safe wrapper for `OS_IdleLoop`.
///
/// This is typically called by the BSP main routine after all other initialization
/// has taken place. It waits until `application_shutdown` is called.
pub fn idle_loop() {
    unsafe { ffi::OS_IdleLoop() };
}

/// Deletes all resources (tasks, queues, etc.) created in OSAL.
///
/// # C-API Mapping
/// This is a safe wrapper for `OS_DeleteAllObjects`.
///
/// This is useful for cleaning up during an orderly shutdown or for testing.
pub fn delete_all_objects() {
    unsafe { ffi::OS_DeleteAllObjects() };
}

/// Initiates an orderly shutdown of the OSAL application.
///
/// # C-API Mapping
/// This is a safe wrapper for `OS_ApplicationShutdown`.
///
/// This allows the task currently blocked in `idle_loop` to wake up and return.
pub fn application_shutdown(should_shutdown: bool) {
    unsafe { ffi::OS_ApplicationShutdown(should_shutdown as u8) };
}

/// Exits/aborts the entire application process immediately.
///
/// # C-API Mapping
/// This is a safe wrapper for `OS_ApplicationExit`.
///
/// This function does not return and is typically only used in non-production
/// scenarios like unit testing.
pub fn application_exit(status: i32) -> ! {
    unsafe { ffi::OS_ApplicationExit(status) };
    loop {}
}
