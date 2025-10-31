//! Safe, ergonomic logging facilities for cFS.
//!
//! This module provides macros (`syslog!`, `printf!`) that mimic the standard
//! library's `println!` macro, but direct output to the cFE System Log and
//! the OSAL console, respectively.
//!
//! The `syslog!` macro is generally preferred for in-flight logging as its
//! output is captured by cFE Executive Services. The `printf!` macro is useful
//! for development and debugging, as its output typically goes to the console/terminal
//! where the cFS instance is running.

use crate::ffi;
use heapless::CString;

/// The maximum size of a single `OS_printf` message, from OSAL configuration.
pub const MAX_PRINTF_MSG_SIZE: usize = ffi::OS_BUFFER_SIZE as usize;

/// Writes a message string to the OSAL console (`OS_printf`).
///
/// This is a low-level wrapper around the C `OS_printf` function. The `printf!`
/// macro is generally more convenient to use. This function does not return an
/// error and is considered a "best-effort" logging mechanism.
///
/// # Arguments
/// * `message`: The string to write. It will be truncated if its byte length
///   exceeds `MAX_PRINTF_MSG_SIZE`.
pub fn printf(message: &str) {
    let mut c_message = CString::<MAX_PRINTF_MSG_SIZE>::new();

    // extend_from_bytes will truncate if the message is too long, which is
    // acceptable behavior for this best-effort printf wrapper.
    let _ = c_message.extend_from_bytes(message.as_bytes());

    unsafe {
        // We call the variadic C function by passing the fully formatted
        // Rust string as a single argument to a simple "%s" format specifier.
        ffi::OS_printf("%s\0".as_ptr() as *const i8, c_message.as_ptr());
    }
}

/// Enables output from the `printf!` macro and the underlying `OS_printf` function.
pub fn printf_enable() {
    unsafe {
        ffi::OS_printf_enable();
    }
}

/// Disables output from the `printf!` macro and the underlying `OS_printf` function.
pub fn printf_disable() {
    unsafe {
        ffi::OS_printf_disable();
    }
}

/// A macro to write a formatted string to the cFE System Log.
///
/// This macro provides a `println!`-like interface for `CFE_ES_WriteToSysLog`.
/// It handles the necessary string formatting and C string conversion.
///
/// # Usage
///
/// ```rust,ignore
/// use libcfs::log::syslog;
/// use libcfs::error::Result;
///
/// fn my_function() -> Result<()> {
///     // Simple literal message
///     syslog!("Starting task...")?;
///
///     let event_count = 10;
///     let status = 0;
///
///     // Formatted message
///     syslog!("Processed {} events with status {}", event_count, status)?;
///
///     Ok(())
/// }
/// ```
///
/// # Returns
///
/// This macro evaluates to a `libcfs::error::Result<()>`. It will return an
/// error if the message cannot be formatted (e.g., too long for the internal
/// buffer) or if `CFE_ES_WriteToSysLog` returns an error.
#[macro_export]
macro_rules! syslog {
    // Match a single literal string with no format arguments.
    ($msg:literal) => {
        $crate::es::app::App::syslog($msg)
    };
    // Match a format string plus arguments, like `println!`.
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        // The temporary buffer size is determined by the CFE mission config.
        let mut buffer: heapless::String<{ $crate::ffi::CFE_PLATFORM_ES_SYSTEM_LOG_SIZE as usize }> = heapless::String::new();

        // Attempt to format the arguments into our heapless string.
        if write!(&mut buffer, $($arg)*).is_ok() {
            $crate::cfe::es::app::App::syslog(&buffer)
        } else {
            // This error occurs if the formatted string is too large for the buffer.
            Err($crate::error::Error::StatusValidationFailure)
        }
    }};
}

/// A macro to write a formatted string to the OSAL console (`OS_printf`).
///
/// This macro provides a `println!`-like interface for `OS_printf`, which is
/// useful for debugging during development. It is a "fire-and-forget" macro
/// that does not return a result.
///
/// # Usage
///
/// ```rust,ignore
/// use libcfs::log::printf;
///
/// fn my_debug_function() {
///     // Simple literal message
///     printf!("Debug function entered.");
///
///     let value = 42;
///
///     // Formatted message
///     printf!("The current debug value is: {}", value);
/// }
/// ```
#[macro_export]
macro_rules! printf {
    // Match a single literal string with no format arguments.
    ($msg:literal) => {
        $crate::log::printf($msg)
    };
    // Match a format string plus arguments, like `println!`.
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        // The temporary buffer size is determined by the OSAL config.
        let mut buffer: heapless::String<{ $crate::ffi::OS_BUFFER_SIZE as usize }> = heapless::String::new();

        // Format the arguments. We ignore the result because printf is best-effort.
        // If it fails, an empty or truncated string will be printed.
        let _ = write!(&mut buffer, $($arg)*);

        $crate::log::printf(&buffer);
    }};
}
