//! Generic OSAL object ID APIs.
//!
//! This module provides utilities for working with generic `OsalId`s, which
//! are the underlying type for all OSAL resources (tasks, queues, mutexes, etc.).

use crate::error::{Error, Result};
use crate::ffi::{self, OS_OBJECT_CREATOR_ANY};
use crate::status::check;
use core::mem::MaybeUninit;
use heapless::CString;

/// A generic, type-safe wrapper for an OSAL object ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct OsalId(pub ffi::osal_id_t);

impl From<ffi::osal_id_t> for OsalId {
    fn from(id: ffi::osal_id_t) -> Self {
        Self(id)
    }
}

impl OsalId {
    /// Retrieves the name of this OSAL object.
    ///
    /// # Errors
    ///
    /// Returns an error if the object ID is invalid, the buffer is too small
    /// (unlikely with `heapless`), or the name is not valid UTF-8.
    pub fn name(&self) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
        let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
        check(unsafe {
            ffi::OS_GetResourceName(
                self.0,
                buffer.as_mut_ptr() as *mut libc::c_char,
                buffer.len(),
            )
        })?;

        let len = buffer.iter().position(|&b| b == 0).unwrap_or(0);
        let mut s = CString::new();
        s.extend_from_bytes(&buffer[..len])
            .map_err(|_| Error::OsErrNameTooLong)?;
        Ok(s)
    }

    /// Identifies the type of this OSAL object.
    pub fn object_type(&self) -> ObjectType {
        let type_val = unsafe { ffi::OS_IdentifyObject(self.0) };
        ObjectType::from(type_val)
    }

    /// Converts this abstract ID into a zero-based integer suitable
    /// for use as an array index.
    ///
    /// This does NOT verify that the ID refers to a currently valid
    /// or active resource — it only performs the numeric conversion.
    pub fn to_index(&self) -> Result<u32> {
        let mut index = MaybeUninit::uninit();
        check(unsafe { ffi::OS_ConvertToArrayIndex(self.0, index.as_mut_ptr()) })?;
        Ok(unsafe { index.assume_init() })
    }

    /// Converts this abstract ID of a specific type into a zero-based integer.
    ///
    /// # Errors
    ///
    /// Returns `Error::OsErrInvalidId` if this ID is not of the
    /// specified `obj_type` or does not map to a valid index.
    pub fn to_index_as_type(&self, obj_type: ObjectType) -> Result<u32> {
        let mut index = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_ObjectIdToArrayIndex(obj_type.into(), self.0, index.as_mut_ptr())
        })?;
        Ok(unsafe { index.assume_init() })
    }
}

/// The type of an OSAL object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ObjectType {
    /// An undefined or invalid object type.
    Undefined = ffi::OS_OBJECT_TYPE_UNDEFINED,
    /// An OSAL task.
    Task = ffi::OS_OBJECT_TYPE_OS_TASK,
    /// An OSAL message queue.
    Queue = ffi::OS_OBJECT_TYPE_OS_QUEUE,
    /// An OSAL counting semaphore.
    CountSem = ffi::OS_OBJECT_TYPE_OS_COUNTSEM,
    /// An OSAL binary semaphore.
    BinSem = ffi::OS_OBJECT_TYPE_OS_BINSEM,
    /// An OSAL mutex.
    Mutex = ffi::OS_OBJECT_TYPE_OS_MUTEX,
    /// An OSAL stream (file descriptor).
    Stream = ffi::OS_OBJECT_TYPE_OS_STREAM,
    /// An OSAL directory handle.
    Dir = ffi::OS_OBJECT_TYPE_OS_DIR,
    /// An OSAL time base.
    TimeBase = ffi::OS_OBJECT_TYPE_OS_TIMEBASE,
    /// An OSAL timer callback.
    TimerCb = ffi::OS_OBJECT_TYPE_OS_TIMECB,
    /// An OSAL loadable module.
    Module = ffi::OS_OBJECT_TYPE_OS_MODULE,
    /// An OSAL file system.
    FileSys = ffi::OS_OBJECT_TYPE_OS_FILESYS,
    /// An OSAL console device.
    Console = ffi::OS_OBJECT_TYPE_OS_CONSOLE,
    /// An OSAL condition variable.
    CondVar = ffi::OS_OBJECT_TYPE_OS_CONDVAR,
    /// A user-defined object type.
    User = ffi::OS_OBJECT_TYPE_USER,
    /// An unknown or unhandled object type.
    Unknown(u32),
}

impl From<u32> for ObjectType {
    fn from(val: u32) -> Self {
        match val {
            ffi::OS_OBJECT_TYPE_UNDEFINED => Self::Undefined,
            ffi::OS_OBJECT_TYPE_OS_TASK => Self::Task,
            ffi::OS_OBJECT_TYPE_OS_QUEUE => Self::Queue,
            ffi::OS_OBJECT_TYPE_OS_COUNTSEM => Self::CountSem,
            ffi::OS_OBJECT_TYPE_OS_BINSEM => Self::BinSem,
            ffi::OS_OBJECT_TYPE_OS_MUTEX => Self::Mutex,
            ffi::OS_OBJECT_TYPE_OS_STREAM => Self::Stream,
            ffi::OS_OBJECT_TYPE_OS_DIR => Self::Dir,
            ffi::OS_OBJECT_TYPE_OS_TIMEBASE => Self::TimeBase,
            ffi::OS_OBJECT_TYPE_OS_TIMECB => Self::TimerCb,
            ffi::OS_OBJECT_TYPE_OS_MODULE => Self::Module,
            ffi::OS_OBJECT_TYPE_OS_FILESYS => Self::FileSys,
            ffi::OS_OBJECT_TYPE_OS_CONSOLE => Self::Console,
            ffi::OS_OBJECT_TYPE_OS_CONDVAR => Self::CondVar,
            ffi::OS_OBJECT_TYPE_USER => Self::User,
            other => Self::Unknown(other),
        }
    }
}

impl Into<u32> for ObjectType {
    fn into(self) -> u32 {
        match self {
            Self::Undefined => ffi::OS_OBJECT_TYPE_UNDEFINED,
            Self::Task => ffi::OS_OBJECT_TYPE_OS_TASK,
            Self::Queue => ffi::OS_OBJECT_TYPE_OS_QUEUE,
            Self::CountSem => ffi::OS_OBJECT_TYPE_OS_COUNTSEM,
            Self::BinSem => ffi::OS_OBJECT_TYPE_OS_BINSEM,
            Self::Mutex => ffi::OS_OBJECT_TYPE_OS_MUTEX,
            Self::Stream => ffi::OS_OBJECT_TYPE_OS_STREAM,
            Self::Dir => ffi::OS_OBJECT_TYPE_OS_DIR,
            Self::TimeBase => ffi::OS_OBJECT_TYPE_OS_TIMEBASE,
            Self::TimerCb => ffi::OS_OBJECT_TYPE_OS_TIMECB,
            Self::Module => ffi::OS_OBJECT_TYPE_OS_MODULE,
            Self::FileSys => ffi::OS_OBJECT_TYPE_OS_FILESYS,
            Self::Console => ffi::OS_OBJECT_TYPE_OS_CONSOLE,
            Self::CondVar => ffi::OS_OBJECT_TYPE_OS_CONDVAR,
            Self::User => ffi::OS_OBJECT_TYPE_USER,
            Self::Unknown(val) => val,
        }
    }
}

/// A generic trampoline function to bridge a C callback to a Rust closure.
///
/// This function is not meant to be called directly. It's a helper to allow
/// passing closures to C APIs that expect a function pointer and a `void *` context.
unsafe extern "C" fn trampoline<F>(id: ffi::osal_id_t, arg: *mut core::ffi::c_void)
where
    F: FnMut(OsalId),
{
    // Cast the `void *` argument back into a mutable reference to our closure.
    let callback = &mut *(arg as *mut F);
    // Call the closure with the provided ID.
    callback(OsalId(id));
}

/// Iterates over all OSAL objects and calls a closure for each one.
///
/// # Arguments
/// * `creator_id`: If `Some(id)`, only objects created by that task ID are processed.
///   If `None`, all objects are processed.
/// * `callback`: A closure that will be called with the ID of each object.
pub fn for_each_object<F>(creator_id: Option<OsalId>, mut callback: F)
where
    F: FnMut(OsalId),
{
    unsafe {
        ffi::OS_ForEachObject(
            creator_id.map_or(OS_OBJECT_CREATOR_ANY, |id| id.0),
            // Pass the generic trampoline function pointer.
            Some(trampoline::<F>),
            // Pass a pointer to our closure as the `void *` context.
            &mut callback as *mut F as *mut core::ffi::c_void,
        );
    }
}

/// Iterates over all OSAL objects of a specific type and calls a closure for each one.
///
/// # Arguments
/// * `obj_type`: The type of object to iterate over.
/// * `creator_id`: If `Some(id)`, only objects created by that task ID are processed.
///   If `None`, all objects are processed.
/// * `callback`: A closure that will be called with the ID of each object.
pub fn for_each_object_of_type<F>(obj_type: ObjectType, creator_id: Option<OsalId>, mut callback: F)
where
    F: FnMut(OsalId),
{
    unsafe {
        ffi::OS_ForEachObjectOfType(
            obj_type.into(),
            creator_id.map_or(OS_OBJECT_CREATOR_ANY, |id| id.0),
            // Reuse the same generic trampoline function pointer.
            Some(trampoline::<F>),
            // Pass a pointer to our closure as the `void *` context.
            &mut callback as *mut F as *mut core::ffi::c_void,
        );
    }
}
