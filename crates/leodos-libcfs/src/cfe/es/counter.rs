//! Safe, idiomatic wrappers for the CFE Executive Services Generic Counter API.
//!
//! This module provides a `Counter` struct that is a thread-safe, RAII-based
//! handle for creating, incrementing, and managing generic counters.

use crate::error::{Error, Result};
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;
use heapless::{CString, String};

/// A handle to a cFE generic counter.
///
/// The counter can be shared across tasks. The underlying cFE
/// resource is automatically deleted when the `Counter` is dropped.
#[derive(Debug)]
pub struct Counter {
    id: CounterId,
}

/// A type-safe, zero-cost wrapper for a cFE Generic Counter ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterId(ffi::CFE_ES_CounterId_t);

impl Counter {
    /// Registers a new generic counter with cFE.
    ///
    /// The counter is initialized to 0.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the counter.
    pub fn new(name: &str) -> Result<Self> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        let mut counter_id = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_RegisterGenCounter(counter_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(Self {
            id: CounterId(unsafe { counter_id.assume_init() }),
        })
    }

    /// Increments the counter's value by one.
    ///
    /// Note: the C header does not guarantee atomicity for this
    /// operation.
    pub fn inc(&self) -> Result<()> {
        check(unsafe { ffi::CFE_ES_IncrementGenCounter(self.id.0) })?;
        Ok(())
    }

    /// Sets the counter's value to a specific number.
    pub fn set(&self, count: u32) -> Result<()> {
        check(unsafe { ffi::CFE_ES_SetGenCount(self.id.0, count) })?;
        Ok(())
    }

    /// Retrieves the current value of the counter.
    pub fn get(&self) -> Result<u32> {
        let mut count = 0;
        check(unsafe { ffi::CFE_ES_GetGenCount(self.id.0, &mut count) })?;
        Ok(count)
    }

    /// Returns the underlying cFE ID of the counter.
    pub fn id(&self) -> CounterId {
        self.id
    }

    /// Gets the cFE ID for a generic counter by its registered name.
    pub fn get_id_by_name(name: &str) -> Result<CounterId> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        let mut counter_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_ES_GetGenCounterIDByName(counter_id.as_mut_ptr(), c_name.as_ptr())
        })?;
        Ok(CounterId(unsafe { counter_id.assume_init() }))
    }
}

impl Drop for Counter {
    /// Deletes the generic counter when the `Counter` object goes out of scope.
    fn drop(&mut self) {
        let _ = unsafe { ffi::CFE_ES_DeleteGenCounter(self.id.0) };
    }
}

impl CounterId {
    /// Gets the cFE registered name for this generic counter ID.
    pub fn name(&self) -> Result<String<{ ffi::OS_MAX_API_NAME as usize }>> {
        let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
        check(unsafe {
            ffi::CFE_ES_GetGenCounterName(
                buffer.as_mut_ptr() as *mut libc::c_char,
                self.0,
                buffer.len(),
            )
        })?;
        let len = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
        let vec = heapless::Vec::from_slice(&buffer[..len]).map_err(|_| Error::OsErrNameTooLong)?;
        String::from_utf8(vec).map_err(|_| Error::InvalidString)
    }

    /// Converts the Counter ID into a zero-based integer suitable for array indexing.
    pub fn to_index(&self) -> Result<u32> {
        let mut index = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_CounterID_ToIndex(self.0, index.as_mut_ptr()) })?;
        Ok(unsafe { index.assume_init() })
    }
}
