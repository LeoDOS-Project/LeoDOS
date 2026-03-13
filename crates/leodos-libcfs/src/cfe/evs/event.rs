//! Event generation and management APIs.
use crate::cfe::es::app::AppId;
use crate::cfe::time::SysTime;
use crate::error::{Error, Result};
use crate::ffi::{self, CFE_EVS_BinFilter_t};
use crate::status::check;

/// The type of a cFE event message, indicating its severity.
#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum EventType {
    /// A low-priority event for debugging purposes.
    Debug = ffi::CFE_EVS_EventType_CFE_EVS_EventType_DEBUG as u16,
    /// An informational event about a nominal state or action.
    Info = ffi::CFE_EVS_EventType_CFE_EVS_EventType_INFORMATION as u16,
    /// An error event that is not catastrophic.
    Error = ffi::CFE_EVS_EventType_CFE_EVS_EventType_ERROR as u16,
    /// A critical error event that may require intervention.
    Critical = ffi::CFE_EVS_EventType_CFE_EVS_EventType_CRITICAL as u16,
}

/// A binary event filter for controlling event reporting.
#[repr(transparent)]
pub struct BinFilter(CFE_EVS_BinFilter_t);

impl BinFilter {
    /// Creates a new binary filter for the specified event ID and mask.
    ///
    /// # Arguments
    /// * `event_id` - The event ID to filter.
    /// * `mask` - The filter mask to apply.
    pub fn new(event_id: u16, mask: u16) -> Self {
        BinFilter(CFE_EVS_BinFilter_t {
            EventID: event_id,
            Mask: mask,
        })
    }

    /// Returns the event ID of the filter.
    pub fn event_id(&self) -> u16 {
        self.0.EventID
    }

    /// Returns the mask of the filter.
    pub fn mask(&self) -> u16 {
        self.0.Mask
    }
}

/// Registers the calling application with the Event Services.
/// Must be called before sending events.
///
/// Calling more than once wipes all previous filter settings.
/// Filter registration is NOT cumulative.
///
/// # Arguments
/// * `filters` - A slice of binary filters to register. Can be empty.
pub fn register(filters: &[BinFilter]) -> Result<()> {
    // For simplicity, not registering any filters initially. This can be expanded.
    let status = if filters.is_empty() {
        unsafe {
            ffi::CFE_EVS_Register(
                core::ptr::null(),
                0,
                ffi::CFE_EVS_EventFilter_CFE_EVS_EventFilter_BINARY as u16,
            )
        }
    } else {
        unsafe {
            ffi::CFE_EVS_Register(
                filters.as_ptr() as *const core::ffi::c_void,
                filters.len() as u16,
                ffi::CFE_EVS_EventFilter_CFE_EVS_EventFilter_BINARY as u16,
            )
        }
    };
    check(status)?;
    Ok(())
}

/// Sends a formatted software event.
///
/// This is a safe wrapper around `CFE_EVS_SendEvent`. It handles creating a
/// C-style format string and passing the arguments.
///
/// Only works within the context of a registered application (after
/// calling [`register`]). For messages outside that context (e.g.
/// early in init), use `CFE_ES_WriteToSysLog` instead.
///
/// NOTE: Due to the varargs nature of the underlying C function, this wrapper uses `core::fmt`
/// and a temporary buffer. Ensure the buffer is large enough for your event messages.
pub fn send(event_id: u16, event_type: EventType, message: &str) -> Result<()> {
    // Create a heapless CString for the message.
    // The max length comes from the CFE mission config.
    let mut c_message: heapless::CString<{ ffi::CFE_MISSION_EVS_MAX_MESSAGE_LENGTH as usize }> =
        heapless::CString::new();

    c_message
        .extend_from_bytes(message.as_bytes())
        .map_err(|_| Error::CfeStatusValidationFailure)?;

    let status = unsafe {
        // We pass the string as a single argument to a "%s" format specifier.
        ffi::CFE_EVS_SendEvent(
            event_id,
            event_type as ffi::CFE_EVS_EventType_Enum_t,
            "%s\0".as_ptr() as *const libc::c_char,
            c_message.as_ptr(),
        )
    };
    check(status)?;
    Ok(())
}

/// Sends a debug-level software event.
pub fn debug(event_id: u16, message: &str) -> Result<()> {
    send(event_id, EventType::Debug, message)
}

/// Sends an info-level software event.
pub fn info(event_id: u16, message: &str) -> Result<()> {
    send(event_id, EventType::Info, message)
}

/// Sends an error-level software event.
pub fn error(event_id: u16, message: &str) -> Result<()> {
    send(event_id, EventType::Error, message)
}

/// Sends a critical-level software event.
pub fn critical(event_id: u16, message: &str) -> Result<()> {
    send(event_id, EventType::Critical, message)
}

impl AppId {
    /// Sends a formatted software event that appears to originate from this application.
    ///
    /// This allows a library or shared service to send an event on behalf of
    /// another application.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying `CFE_EVS_SendEventWithAppID` call fails.
    pub fn send_event(&self, event_id: u16, event_type: EventType, message: &str) -> Result<()> {
        let mut c_message: heapless::CString<{ ffi::CFE_MISSION_EVS_MAX_MESSAGE_LENGTH as usize }> =
            heapless::CString::new();

        c_message
            .extend_from_bytes(message.as_bytes())
            .map_err(|_| Error::CfeStatusValidationFailure)?;

        let status = unsafe {
            ffi::CFE_EVS_SendEventWithAppID(
                event_id,
                event_type as ffi::CFE_EVS_EventType_Enum_t,
                self.0,
                "%s\0".as_ptr() as *const libc::c_char,
                c_message.as_ptr(),
            )
        };
        check(status)?;
        Ok(())
    }
}

/// Sends a formatted software event with a specific time tag.
pub fn send_timed_event(
    time: SysTime,
    event_id: u16,
    event_type: EventType,
    message: &str,
) -> Result<()> {
    let mut c_message: heapless::CString<{ ffi::CFE_MISSION_EVS_MAX_MESSAGE_LENGTH as usize }> =
        heapless::CString::new();

    c_message
        .extend_from_bytes(message.as_bytes())
        .map_err(|_| Error::CfeStatusValidationFailure)?;

    let status = unsafe {
        ffi::CFE_EVS_SendTimedEvent(
            time.0,
            event_id,
            event_type as ffi::CFE_EVS_EventType_Enum_t,
            "%s\0".as_ptr() as *const libc::c_char,
            c_message.as_ptr(),
        )
    };
    check(status)?;
    Ok(())
}

/// Resets the filter for a single event ID for the calling application.
pub fn reset_filter(event_id: u16) -> Result<()> {
    check(unsafe { ffi::CFE_EVS_ResetFilter(event_id) })?;
    Ok(())
}

/// Resets all event filters for the calling application.
pub fn reset_all_filters() -> Result<()> {
    check(unsafe { ffi::CFE_EVS_ResetAllFilters() })?;
    Ok(())
}
