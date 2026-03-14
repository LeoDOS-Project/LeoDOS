//! Safe, idiomatic wrappers for OSAL Timer APIs.
//!
//! This module provides a `Timer` struct for creating, configuring, and managing
//! OSAL timers that execute a callback function at a specified interval. The `Timer`
//! struct uses RAII to ensure the underlying OSAL resource is deleted when it
//! is dropped.

use crate::error::{CfsError, OsalError, Result};
use crate::ffi;
use crate::os::id::OsalId;
use crate::os::timebase::TimeBaseId;
use crate::os::util::c_name_from_str;
use crate::status::check;
use core::ffi::{c_void, CStr};
use core::mem::MaybeUninit;
use core::ops::Drop;
use heapless::CString;

/// A type alias for the callback function used by an OSAL timer.
///
/// The function receives the ID of the timer that triggered it.
pub type TimerCallback = unsafe extern "C" fn(timer_id: ffi::osal_id_t);

/// A type alias for a timer callback that accepts a user-defined argument.
pub type TimerArgCallback = unsafe extern "C" fn(timer_id: ffi::osal_id_t, arg: *mut c_void);

/// Properties of an OSAL timer.
#[derive(Debug, Clone)]
pub struct TimerProp {
    /// The registered name of the timer.
    pub name: CString<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created the timer.
    pub creator: OsalId,
    /// The configured start time in microseconds.
    pub start_time: u32,
    /// The configured interval time in microseconds.
    pub interval_time: u32,
    /// The accuracy of the timer in microseconds.
    pub accuracy: u32,
}

/// A handle to an OSAL timer.
///
/// This is a wrapper around an `osal_id_t` that will automatically call
/// `OS_TimerDelete` when it goes out of scope, preventing resource leaks.
#[derive(Debug)]
pub struct Timer {
    id: ffi::osal_id_t,
}

impl Timer {
    /// Creates a new OSAL timer and associates it with a callback
    /// function.
    ///
    /// This also creates a dedicated hidden time base object
    /// (consuming a resource slot) that is deleted when the timer
    /// is dropped.
    ///
    /// On success, returns the `Timer` instance and the clock
    /// accuracy in microseconds. The timer does not start until
    /// [`set`](Self::set) is called.
    ///
    /// Must not be called from the context of a timer callback.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the timer.
    /// * `callback`: The function to be executed when the timer expires.
    pub fn new(name: &str, callback: TimerCallback) -> Result<(Self, u32)> {
        let c_name = c_name_from_str(name)?;
        let mut timer_id = MaybeUninit::uninit();
        let mut clock_accuracy = MaybeUninit::uninit();

        let status = unsafe {
            ffi::OS_TimerCreate(
                timer_id.as_mut_ptr(),
                c_name.as_ptr(),
                clock_accuracy.as_mut_ptr(),
                Some(callback),
            )
        };

        check(status)?;

        Ok((
            Self {
                id: unsafe { timer_id.assume_init() },
            },
            unsafe { clock_accuracy.assume_init() },
        ))
    }

    /// Programs the timer for a one-shot or periodic execution.
    ///
    /// Both `start_time_usecs` and `interval_time_usecs` being zero
    /// is an error. Values below the clock accuracy are rounded up
    /// to the timer's resolution.
    ///
    /// Must not be called from the context of a timer callback.
    ///
    /// # Arguments
    /// * `start_time_usecs`: Time in microseconds until the first expiration.
    /// * `interval_time_usecs`: Time in microseconds between subsequent expirations.
    ///   If set to 0, the timer is a one-shot timer and will only fire once.
    pub fn set(&self, start_time_usecs: u32, interval_time_usecs: u32) -> Result<()> {
        let status = unsafe { ffi::OS_TimerSet(self.id, start_time_usecs, interval_time_usecs) };
        check(status)?;
        Ok(())
    }

    /// Finds an existing timer ID by its name.
    ///
    /// Must not be called from the context of a timer callback.
    pub fn get_id_by_name(name: &str) -> Result<ffi::osal_id_t> {
        let c_name = c_name_from_str(name)?;
        let mut timer_id = MaybeUninit::uninit();
        let status = unsafe { ffi::OS_TimerGetIdByName(timer_id.as_mut_ptr(), c_name.as_ptr()) };
        check(status)?;
        Ok(unsafe { timer_id.assume_init() })
    }

    /// Creates a new OSAL timer and attaches it to an existing time base.
    ///
    /// This allows multiple timers to share a single underlying timing source.
    pub fn add(
        name: &str,
        timebase_id: TimeBaseId,
        callback: TimerArgCallback,
        callback_arg: *mut c_void,
    ) -> Result<Self> {
        let c_name = c_name_from_str(name)?;
        let mut timer_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_TimerAdd(
                timer_id.as_mut_ptr(),
                c_name.as_ptr(),
                timebase_id.0,
                Some(callback),
                callback_arg,
            )
        })?;
        Ok(Self {
            id: unsafe { timer_id.assume_init() },
        })
    }

    /// Retrieves information about this timer.
    ///
    /// Must not be called from the context of a timer callback.
    pub fn info(&self) -> Result<TimerProp> {
        let mut prop = MaybeUninit::<ffi::OS_timer_prop_t>::uninit();
        check(unsafe { ffi::OS_TimerGetInfo(self.id, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        let name_ptr = prop.name.as_ptr();
        let name_cstr = unsafe { CStr::from_ptr(name_ptr) };
        let mut name_string = CString::new();
        name_string
            .extend_from_bytes(name_cstr.to_bytes())
            .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;

        Ok(TimerProp {
            name: name_string,
            creator: OsalId(prop.creator),
            start_time: prop.start_time,
            interval_time: prop.interval_time,
            accuracy: prop.accuracy,
        })
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_TimerDelete(self.id) };
    }
}
