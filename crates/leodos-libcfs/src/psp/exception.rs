//! Safe wrappers for PSP exception log functions.
//!
//! These functions allow an application to read and process the log of
//! processor exceptions captured by the PSP. This is useful for advanced
//! health monitoring or crash analysis tools.

use crate::error::{Error, Result};
use crate::ffi;
use crate::os::task::TaskId;
use crate::status::check;
use core::mem::MaybeUninit;
use heapless::String;

/// A summary of an exception log entry.
#[derive(Debug, Clone)]
pub struct ExceptionSummary {
    /// The ID of the exception log entry.
    pub context_log_id: u32,
    /// The OSAL task ID of the task that caused the exception.
    pub task_id: TaskId,
    /// A descriptive reason for the exception.
    pub reason: String<{ ffi::OS_ERROR_NAME_LENGTH as usize }>,
}

/// Returns the number of unread exceptions in the log.
pub fn get_count() -> u32 {
    unsafe { ffi::CFE_PSP_Exception_GetCount() }
}

/// Retrieves and **pops** the next exception log entry
/// (destructive read).
///
/// Success does not guarantee all output fields contain valid
/// data — only that they have been initialized.
pub fn get_summary() -> Result<ExceptionSummary> {
    let mut context_id = MaybeUninit::uninit();
    let mut task_id = MaybeUninit::uninit();
    let mut reason_buf = [0u8; ffi::OS_ERROR_NAME_LENGTH as usize];

    check(unsafe {
        ffi::CFE_PSP_Exception_GetSummary(
            context_id.as_mut_ptr(),
            task_id.as_mut_ptr(),
            reason_buf.as_mut_ptr() as *mut libc::c_char,
            reason_buf.len() as u32,
        )
    })?;

    let len = reason_buf
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(reason_buf.len());
    let reason_vec =
        heapless::Vec::from_slice(&reason_buf[..len]).map_err(|_| Error::OsErrNameTooLong)?;
    let reason = String::from_utf8(reason_vec).map_err(|_| Error::InvalidString)?;

    Ok(ExceptionSummary {
        context_log_id: unsafe { context_id.assume_init() },
        task_id: TaskId(unsafe { task_id.assume_init() }),
        reason,
    })
}

/// Copies the processor context of a specific exception log entry
/// into a buffer.
///
/// Returns the number of bytes copied. May return
/// `NO_EXCEPTION_DATA` if the context data has expired from
/// the circular memory log.
///
/// # Safety
/// The `context_buf` must be a valid, writable buffer of at least `context_buf.len()` bytes.
pub unsafe fn copy_context(context_log_id: u32, context_buf: &mut [u8]) -> Result<usize> {
    let bytes_copied = ffi::CFE_PSP_Exception_CopyContext(
        context_log_id,
        context_buf.as_mut_ptr() as *mut _,
        context_buf.len() as u32,
    );

    if bytes_copied < 0 {
        Err(Error::from(bytes_copied))
    } else {
        Ok(bytes_copied as usize)
    }
}
