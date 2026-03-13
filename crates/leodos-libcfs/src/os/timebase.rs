//! Safe, idiomatic wrappers for OSAL Time Base APIs.
//!
//! This module provides a `TimeBase` struct for creating and managing OSAL
//! time bases, which act as sources for timer ticks. The `TimeBase` struct uses
//! RAII to ensure the underlying OSAL resource is properly cleaned up.

use crate::error::{Error, Result};
use crate::ffi;
use crate::os::id::OsalId;
use crate::os::util::c_name_from_str;
use crate::status::check;
use core::ffi::CStr;
use core::mem::MaybeUninit;
use core::time::Duration;
use heapless::CString;

/// A type-safe, zero-cost wrapper for an OSAL Time Base ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TimeBaseId(pub ffi::osal_id_t);

impl TimeBaseId {
    /// Reads the value of the timebase free-running counter.
    ///
    /// This is a lightweight way to poll a monotonically increasing timer. The absolute
    /// value is not relevant, but differences between successive calls can be used for
    /// high-resolution timing.
    pub fn get_free_run(&self) -> Result<u32> {
        let mut freerun_val = MaybeUninit::uninit();
        check(unsafe { ffi::OS_TimeBaseGetFreeRun(self.0, freerun_val.as_mut_ptr()) })?;
        Ok(unsafe { freerun_val.assume_init() })
    }

    /// Finds an existing time base ID by its name.
    pub fn from_name(name: &str) -> Result<Self> {
        let c_name = c_name_from_str(name)?;
        let mut timebase_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_TimeBaseGetIdByName(timebase_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(Self(unsafe { timebase_id.assume_init() }))
    }
}

/// Properties of a time base, returned by `TimeBase::info`.
#[derive(Debug, Clone)]
pub struct TimeBaseProp {
    /// The registered name of the time base.
    pub name: CString<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created the time base.
    pub creator: OsalId,
    /// The nominal interval time in microseconds.
    pub nominal_interval_time: u32,
    /// The free-running time in microseconds.
    pub freerun_time: u32,
    /// The accuracy of the time base in microseconds.
    pub accuracy: u32,
}

/// A handle to an OSAL time base.
///
/// A time base is an abstraction of a "timer tick" that can be used for
/// measuring elapsed time or scheduling timer callbacks. This wrapper manages a
/// software-simulated time base provided by the OSAL.
#[derive(Debug)]
pub struct TimeBase {
    id: TimeBaseId,
}

impl TimeBase {
    /// Creates a new software-simulated OSAL time base.
    ///
    /// This time base will use the underlying OS kernel's timing
    /// facilities. The timer does not start until `set()` is called.
    ///
    /// This creates a servicing task at elevated priority that may
    /// interrupt user tasks. The kernel must be configured for
    /// `OS_MAX_TASKS + OS_MAX_TIMEBASES` threads.
    ///
    /// Must not be called from the context of a timer callback.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the time base.
    pub fn new(name: &str) -> Result<Self> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        let mut timebase_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_TimeBaseCreate(
                timebase_id.as_mut_ptr(),
                c_name.as_ptr(),
                None, // Use software-simulated timer
            )
        })?;

        Ok(Self {
            id: TimeBaseId(unsafe { timebase_id.assume_init() }),
        })
    }

    /// Programs the time base for a one-shot or periodic tick.
    ///
    /// Must not be called from the context of a timer callback.
    ///
    /// # Arguments
    /// * `start`: `Duration` until the first tick.
    /// * `interval`: `Duration` between subsequent ticks. If `Duration::ZERO`,
    ///   the time base will only tick once.
    pub fn set(&self, start: Duration, interval: Duration) -> Result<()> {
        let start_usecs = start
            .as_micros()
            .try_into()
            .map_err(|_| Error::OsErrInvalidArgument)?;
        let interval_usecs = interval
            .as_micros()
            .try_into()
            .map_err(|_| Error::OsErrInvalidArgument)?;

        check(unsafe { ffi::OS_TimeBaseSet(self.id.0, start_usecs, interval_usecs) })?;
        Ok(())
    }

    /// Returns the underlying `TimeBaseId`.
    pub fn id(&self) -> TimeBaseId {
        self.id
    }

    /// Retrieves information about this time base.
    pub fn info(&self) -> Result<TimeBaseProp> {
        let mut prop = MaybeUninit::<ffi::OS_timebase_prop_t>::uninit();
        check(unsafe { ffi::OS_TimeBaseGetInfo(self.id.0, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        let name_cstr = unsafe { CStr::from_ptr(prop.name.as_ptr()) };
        let mut name = CString::new();
        name.extend_from_bytes(name_cstr.to_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        Ok(TimeBaseProp {
            name,
            creator: OsalId(prop.creator),
            nominal_interval_time: prop.nominal_interval_time,
            freerun_time: prop.freerun_time,
            accuracy: prop.accuracy,
        })
    }
}

impl Drop for TimeBase {
    /// Deletes the OSAL time base when the `TimeBase` object goes out of scope.
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_TimeBaseDelete(self.id.0) };
    }
}
