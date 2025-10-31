//! Wrappers for PSP watchdog timer functions.

use crate::ffi;

/// Initializes the watchdog timer.
pub fn init() {
    unsafe { ffi::CFE_PSP_WatchdogInit() };
}

/// Enables the watchdog timer.
pub fn enable() {
    unsafe { ffi::CFE_PSP_WatchdogEnable() };
}

/// Disables the watchdog timer.
pub fn disable() {
    unsafe { ffi::CFE_PSP_WatchdogDisable() };
}

/// Services (i.e., "pets") the watchdog timer to prevent it from expiring.
pub fn service() {
    unsafe { ffi::CFE_PSP_WatchdogService() };
}

/// Sets the watchdog timeout period in milliseconds.
pub fn set(value_ms: u32) {
    unsafe { ffi::CFE_PSP_WatchdogSet(value_ms) };
}

/// Gets the current watchdog timeout period in milliseconds.
pub fn get() -> u32 {
    unsafe { ffi::CFE_PSP_WatchdogGet() }
}
