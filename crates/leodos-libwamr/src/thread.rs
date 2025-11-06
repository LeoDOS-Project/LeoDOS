//! WAMR threading support.

use crate::{ffi, Result, WamrError};
use core::ffi::c_void;
use core::ptr::null_mut;

/// A handle to a running WAMR thread.
/// This is only available with the `thread-support` feature.
#[cfg(feature = "thread-support")]
pub struct WasmThread {
    pub(crate) tid: ffi::wasm_thread_t,
}

#[cfg(feature = "thread-support")]
impl WasmThread {
    /// Waits for the thread to finish execution.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer returned by the thread's callback
    /// is valid and can be safely cast to `T`.
    pub unsafe fn join<T>(self) -> Result<*mut T> {
        let mut ret_val: *mut c_void = null_mut();
        let result = unsafe { ffi::wasm_runtime_join_thread(self.tid, &mut ret_val) };

        if result == 0 {
            Ok(ret_val as *mut T)
        } else {
            Err(WamrError::ThreadError(result))
        }
    }
}
