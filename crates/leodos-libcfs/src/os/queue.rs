//! Safe, idiomatic wrappers for OSAL Message Queue APIs.
//!
//! This module provides a generic, type-safe `Queue<T>` for intra-application
//! message passing. The `Queue` struct is an RAII wrapper that ensures the
//! underlying OSAL resource is properly cleaned up.

use crate::error::{CfsError, OsalError, Result};
use crate::ffi;
use crate::os::id::OsalId;
use crate::os::util::c_name_from_str;
use crate::string_from_c_buf;
use crate::status::check;
use core::marker::PhantomData;
use core::mem::{self, MaybeUninit};
use core::ops::Drop;
use heapless::String;

/// Properties of a message queue, returned by `Queue::get_info`.
#[derive(Debug, Clone)]
pub struct QueueProp {
    /// The registered name of the queue.
    pub name: String<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created the queue.
    pub creator: OsalId,
}

/// A type-safe message queue for communicating between tasks within an application.
///
/// The type `T` must be `Copy` and `Sized` to be sent over the queue, as the
/// underlying OSAL API works with raw byte copies.
#[derive(Debug)]
pub struct Queue<T: Copy + Sized> {
    id: OsalId,
    _phantom: PhantomData<T>,
}

impl<T: Copy + Sized> Queue<T> {
    /// Creates a new message queue.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the queue.
    /// * `depth`: The maximum number of messages the queue can hold.
    pub fn new(name: &str, depth: usize) -> Result<Self> {
        let c_name = c_name_from_str(name)?;
        let mut queue_id = MaybeUninit::uninit();

        let status = unsafe {
            ffi::OS_QueueCreate(
                queue_id.as_mut_ptr(),
                c_name.as_ptr(),
                depth,
                mem::size_of::<T>(),
                0,
            )
        };
        check(status)?;

        Ok(Self {
            id: OsalId(unsafe { queue_id.assume_init() }),
            _phantom: PhantomData,
        })
    }

    /// Puts a message onto the queue.
    ///
    /// This operation is non-blocking. If the queue is full, an error is returned.
    ///
    /// # Arguments
    /// * `message`: A reference to the message data to send.
    pub fn put(&self, message: &T) -> Result<()> {
        let status = unsafe {
            ffi::OS_QueuePut(
                self.id.0,
                message as *const T as *const _,
                mem::size_of::<T>(),
                0,
            )
        };
        check(status)?;
        Ok(())
    }

    /// Retrieves a message from the queue.
    ///
    /// This operation can block until a message is available, depending on the timeout.
    ///
    /// # Arguments
    /// * `timeout_ms`: Timeout in milliseconds. Can be a positive value,
    ///   `ffi::OS_CHECK` (0) for a non-blocking poll, or `ffi::OS_PEND` (-1)
    ///   to block indefinitely.
    pub fn get(&self, timeout_ms: i32) -> Result<T> {
        let mut message = MaybeUninit::<T>::uninit();
        let mut size_copied = MaybeUninit::uninit();

        let status = unsafe {
            ffi::OS_QueueGet(
                self.id.0,
                message.as_mut_ptr() as *mut _,
                mem::size_of::<T>(),
                size_copied.as_mut_ptr(),
                timeout_ms,
            )
        };
        check(status)?;

        let size_copied = unsafe { size_copied.assume_init() };
        if size_copied != mem::size_of::<T>() {
            return Err(CfsError::Osal(OsalError::QueueInvalidSize));
        }

        Ok(unsafe { message.assume_init() })
    }

    /// Returns the underlying OSAL ID of the queue.
    pub fn id(&self) -> OsalId {
        self.id
    }

    /// Finds an existing queue ID by its name.
    pub fn get_id_by_name(name: &str) -> Result<OsalId> {
        let c_name = c_name_from_str(name)?;
        let mut queue_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_QueueGetIdByName(queue_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(OsalId(unsafe { queue_id.assume_init() }))
    }

    /// Retrieves information about this queue.
    pub fn get_info(&self) -> Result<QueueProp> {
        let mut prop = MaybeUninit::<ffi::OS_queue_prop_t>::uninit();
        check(unsafe { ffi::OS_QueueGetInfo(self.id.0, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        Ok(QueueProp {
            name: string_from_c_buf(&prop.name)?,
            creator: OsalId(prop.creator),
        })
    }
}

impl<T: Copy + Sized> Drop for Queue<T> {
    /// Deletes the OSAL queue when the `Queue` object goes out of scope.
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_QueueDelete(self.id.0) };
    }
}
