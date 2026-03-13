//! "Zero-copy" buffer management for sending messages.
use core::mem;
use core::slice;

use crate::cfe::sb::msg::MessageMut;
use crate::error::Error;
use crate::error::Result;
use crate::ffi;

/// An owned, writable "zero-copy" software bus message buffer.
///
/// This struct safely manages a memory buffer allocated directly from CFE's
/// internal pool. You can get a writable view of it using `MessageMut::from()`
/// and then send it with zero memory copies.
///
/// If the buffer is dropped without being sent, it is automatically released
/// back to the CFE pool, preventing memory leaks.
#[derive(Debug)]
pub struct SendBuffer {
    pub(crate) ptr: *mut ffi::CFE_SB_Buffer_t,
    pub(crate) size: usize,
}

impl SendBuffer {
    /// Allocates a new zero-copy send buffer of the specified size from the CFE SB pool.
    pub fn new(size: usize) -> Result<Self> {
        let ptr = unsafe { ffi::CFE_SB_AllocateMessageBuffer(size) };
        if ptr.is_null() {
            Err(Error::CfeSbBufAlocErr)
        } else {
            Ok(Self { ptr, size })
        }
    }

    /// Transmits the message in this buffer.
    ///
    /// This consumes the `SendBuffer`, transferring ownership of the
    /// memory to CFE. After this call, the buffer is no longer
    /// accessible from Rust.
    ///
    /// On failure, the caller still owns the buffer (state is
    /// unchanged) and the `Drop` impl will release it.
    ///
    /// # Arguments
    /// * `is_origination`: Set to `true` to have CFE automatically fill in fields like
    ///   sequence count and timestamp. Set to `false` when forwarding a message.
    pub fn send(self, is_origination: bool) -> Result<()> {
        let status = unsafe { ffi::CFE_SB_TransmitBuffer(self.ptr, is_origination) };

        if status == ffi::CFE_SUCCESS {
            // CFE now owns the buffer. We call mem::forget to prevent our Drop logic
            // from running to avoid a double-free.
            mem::forget(self);
            Ok(())
        } else {
            Err(Error::from(status))
        }
    }

    /// Returns a read-only slice view of the buffer's contents.
    pub fn view(&mut self) -> MessageMut<'_> {
        MessageMut {
            slice: unsafe { slice::from_raw_parts_mut(self.ptr as *mut u8, self.size) },
        }
    }

    /// Returns the raw byte slice of the buffer's contents.
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.size) }
    }

    /// Returns the raw mutable byte slice of the buffer's contents.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut u8, self.size) }
    }
}

impl Drop for SendBuffer {
    /// Automatically releases the buffer back to the CFE pool if it hasn't been sent.
    fn drop(&mut self) {
        // This is the cleanup path for when a SendBuffer is created but never sent.
        let _ = unsafe { ffi::CFE_SB_ReleaseMessageBuffer(self.ptr) };
    }
}
