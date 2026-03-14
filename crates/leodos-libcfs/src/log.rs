//! Safe, ergonomic logging facilities for cFS.
//!
//! ## System log and console
//!
//! The [`log!`] and [`printf!`] macros write to the cFE System Log
//! and the OSAL console, respectively.
//!
//! ## Event Services (EVS)
//!
//! The [`info!`], [`warn!`], and [`err!`] macros send EVS events
//! with `println!`-like formatting. The event ID is derived from
//! the source line number automatically.
//!
//! ```rust,ignore
//! info!("system nominal")?;
//! warn!("temperature high: {} C", temp)?;
//! err!("{} failed", subsystem)?;
//! ```

use crate::error::{CfsError, OsalError};
use crate::error::Result;
use crate::ffi;
use crate::status::check;
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
        ffi::OS_printf("%s\0".as_ptr() as *const libc::c_char, c_message.as_ptr());
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
///     log!("Starting task...")?;
///
///     let event_count = 10;
///     let status = 0;
///
///     // Formatted message
///     log!("Processed {} events with status {}", event_count, status)?;
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
macro_rules! log {
    // Match a single literal string with no format arguments.
    ($msg:literal) => {
        $crate::log::syslog($msg)
    };
    // Match a format string plus arguments, like `println!`.
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        // The temporary buffer size is determined by the CFE mission config.
        let mut buffer: heapless::String<{ $crate::log::SYSLOG_MAX_MSG_SIZE as usize }> = heapless::String::new();

        // Attempt to format the arguments into our heapless string.
        if write!(&mut buffer, $($arg)*).is_ok() {
            $crate::log::syslog(&buffer)
        } else {
            // This error occurs if the formatted string is too large for the buffer.
            Err($crate::error::CfsError::ValidationFailure)
        }
    }};
}

/// The maximum size of a single cFE System Log message, from cFE configuration.
pub const SYSLOG_MAX_MSG_SIZE: usize = ffi::CFE_PLATFORM_ES_SYSTEM_LOG_SIZE as usize;

/// Writes a message to the cFE system log.
///
/// This is useful for logging critical events, especially during initialization
/// before Event Services (EVS) are available, or in error paths where EVS
/// might fail.
///
/// The `log!` macro provides a more convenient, `println!`-like interface
/// for this functionality.
pub fn syslog(message: &str) -> Result<()> {
    let mut c_string = CString::<256>::new();
    c_string
        .extend_from_bytes(message.as_bytes())
        .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;

    check(unsafe { ffi::CFE_ES_WriteToSysLog(c_string.as_ptr()) })?;
    Ok(())
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

/// The maximum formatted EVS message size, from cFE mission config.
pub const EVS_MAX_MSG_SIZE: usize = ffi::CFE_MISSION_EVS_MAX_MESSAGE_LENGTH as usize;

/// Sends an informational EVS event (`EventType::Info`).
///
/// Event ID is derived from the call-site line number.
///
/// ```rust,ignore
/// info!("system nominal")?;
/// info!("processed {} packets", count)?;
/// ```
#[macro_export]
macro_rules! info {
    ($msg:literal) => {
        $crate::cfe::evs::event::send(
            line!() as u16,
            $crate::cfe::evs::event::EventType::Info,
            $msg,
        )
    };
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut buf: heapless::String<{ $crate::log::EVS_MAX_MSG_SIZE }> = heapless::String::new();
        if write!(&mut buf, $($arg)*).is_ok() {
            $crate::cfe::evs::event::send(
                line!() as u16,
                $crate::cfe::evs::event::EventType::Info,
                &buf,
            )
        } else {
            Err($crate::error::CfsError::ValidationFailure)
        }
    }};
}

/// Sends a warning EVS event (`EventType::Error` — non-catastrophic).
///
/// cFS EVS has no dedicated warning level; this maps to
/// `EventType::Error` which is defined as "not catastrophic."
///
/// ```rust,ignore
/// warn!("temperature high: {} C", temp)?;
/// ```
#[macro_export]
macro_rules! warn {
    ($msg:literal) => {
        $crate::cfe::evs::event::send(
            line!() as u16,
            $crate::cfe::evs::event::EventType::Error,
            $msg,
        )
    };
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut buf: heapless::String<{ $crate::log::EVS_MAX_MSG_SIZE }> = heapless::String::new();
        if write!(&mut buf, $($arg)*).is_ok() {
            $crate::cfe::evs::event::send(
                line!() as u16,
                $crate::cfe::evs::event::EventType::Error,
                &buf,
            )
        } else {
            Err($crate::error::CfsError::ValidationFailure)
        }
    }};
}

/// Sends a critical EVS event (`EventType::Critical`).
///
/// ```rust,ignore
/// err!("{} failed", subsystem)?;
/// ```
#[macro_export]
macro_rules! err {
    ($msg:literal) => {
        $crate::cfe::evs::event::send(
            line!() as u16,
            $crate::cfe::evs::event::EventType::Critical,
            $msg,
        )
    };
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut buf: heapless::String<{ $crate::log::EVS_MAX_MSG_SIZE }> = heapless::String::new();
        if write!(&mut buf, $($arg)*).is_ok() {
            $crate::cfe::evs::event::send(
                line!() as u16,
                $crate::cfe::evs::event::EventType::Critical,
                &buf,
            )
        } else {
            Err($crate::error::CfsError::ValidationFailure)
        }
    }};
}
