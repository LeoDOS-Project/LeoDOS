//! Safe, idiomatic wrappers for OSAL Task APIs.
//!
//! This module provides utilities for task management, such as delaying the
//! current task, retrieving task IDs, and managing task priorities.

use bitflags::bitflags;

use crate::error::{Error, Result};
use crate::ffi;
use crate::os::id::OsalId;
use crate::status::check;
use core::ffi::CStr;
use core::mem::MaybeUninit;
use core::time::Duration;
use heapless::CString;

bitflags! {
    /// Options for task creation.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct TaskFlags: u32 {
        /// Enable floating-point register context switching
        /// for this task. Without this flag, using FP
        /// instructions may corrupt other tasks' FP state.
        const FP_ENABLED = ffi::OS_FP_ENABLED;
    }
}

/// A type-safe, zero-cost wrapper for an OSAL Task ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TaskId(pub ffi::osal_id_t);

impl TaskId {
    /// Sets the priority of this task.
    ///
    /// # Arguments
    /// * `priority`: The new priority (0=highest, 255=lowest).
    pub fn set_priority(&self, priority: u8) -> Result<()> {
        check(unsafe { ffi::OS_TaskSetPriority(self.0, priority) })?;
        Ok(())
    }

    /// Retrieves the `TaskId` of the currently executing task.
    pub fn current() -> Result<Self> {
        let id = unsafe { ffi::OS_TaskGetId() };
        if id == 0 {
            // OS_TaskGetId returns 0 if not called from a valid task context.
            Err(Error::OsErrInvalidId)
        } else {
            Ok(Self(id))
        }
    }

    /// Finds an existing OSAL task ID by its name.
    pub fn from_name(name: &str) -> Result<Self> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        let mut task_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_TaskGetIdByName(task_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(Self(unsafe { task_id.assume_init() }))
    }
}

/// Type alias for a handler function called on task deletion.
pub type TaskDeleteHandler = unsafe extern "C" fn();

/// Properties of an OSAL task, returned by `Task::get_info`.
#[derive(Debug, Clone)]
pub struct TaskProp {
    /// The registered name of the task.
    pub name: CString<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created this task.
    pub creator: OsalId,
    /// The allocated stack size of the task in bytes.
    pub stack_size: usize,
    /// The priority of the task (0=highest, 255=lowest).
    pub priority: u8,
}

/// A handle to an OSAL task.
///
/// This is a wrapper around an `osal_id_t` that will automatically call
/// `OS_TaskDelete` when it goes out of scope, preventing resource leaks.
#[derive(Debug)]
pub struct Task {
    id: TaskId,
}

impl Task {
    /// Creates a new OSAL task and starts it running.
    ///
    /// The stack for the task is allocated from the system memory heap.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the task.
    /// * `entry_point`: The function that the new task will execute. This must
    ///   be a free function with `extern "C"` linkage.
    /// * `stack_size`: The size of the stack to allocate for the new task.
    /// * `priority`: The priority of the new task (0=highest, 255=lowest).
    /// * `flags`: Task creation flags (e.g. `TaskFlags::FP_ENABLED`).
    pub fn new(
        name: &str,
        entry_point: unsafe extern "C" fn(),
        stack_size: usize,
        priority: u8,
        flags: TaskFlags,
    ) -> Result<Self> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        let mut task_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_TaskCreate(
                task_id.as_mut_ptr(),
                c_name.as_ptr(),
                Some(entry_point),
                core::ptr::null_mut(), // OSAL_TASK_STACK_ALLOCATE
                stack_size,
                priority,
                flags.bits(),
            )
        })?;

        Ok(Self {
            id: TaskId(unsafe { task_id.assume_init() }),
        })
    }

    /// Returns the underlying `TaskId` for this task.
    pub fn id(&self) -> TaskId {
        self.id
    }

    /// Retrieves information about this task.
    pub fn get_info(&self) -> Result<TaskProp> {
        let mut prop = MaybeUninit::<ffi::OS_task_prop_t>::uninit();
        check(unsafe { ffi::OS_TaskGetInfo(self.id.0, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        let mut name_str = CString::new();
        let c_str = unsafe { CStr::from_ptr(prop.name.as_ptr()) };
        name_str
            .extend_from_bytes(c_str.to_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        Ok(TaskProp {
            name: name_str,
            creator: OsalId(prop.creator),
            stack_size: prop.stack_size,
            priority: prop.priority,
        })
    }
}

impl Drop for Task {
    /// Deletes the OSAL task when the `Task` object goes out of scope.
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_TaskDelete(self.id.0) };
    }
}

/// Delays the execution of the current task for at least the specified duration.
///
/// # Arguments
/// * `duration`: The minimum amount of time to delay.
pub fn delay(duration: Duration) -> Result<()> {
    let millis = duration.as_millis();
    // Clamp to u32::MAX, which is ~49 days. This is a reasonable upper limit for a delay.
    let millis_u32 = millis.try_into().unwrap_or(u32::MAX);
    check(unsafe { ffi::OS_TaskDelay(millis_u32) })?;
    Ok(())
}

/// Installs a handler function to be called when the current task is deleted.
///
/// This is useful for cleaning up resources that a task creates before it is
/// removed from the system.
///
/// # Safety
/// The provided `handler` function must be a valid `extern "C"` function pointer.
/// It will be called in the context of task deletion, so it should be brief
/// and avoid complex operations or blocking, especially any that would try to
/// interact further with OSAL.
pub fn install_delete_handler(handler: TaskDeleteHandler) -> Result<()> {
    check(unsafe { ffi::OS_TaskInstallDeleteHandler(Some(handler)) })?;
    Ok(())
}

/// Exits the calling task.
///
/// This function terminates the currently running task and does not return.
pub fn exit() -> ! {
    unsafe {
        ffi::OS_TaskExit();
    }
    // This function never returns, but we add a loop to satisfy the compiler.
    loop {}
}

/// Reverse-looks up an OSAL task ID from an underlying operating system ID.
///
/// This is for special cases like exception handling where the OS provides a
/// native task ID that needs to be mapped back to an OSAL ID.
///
/// # Safety
/// The `sys_data` pointer must be a valid pointer to the OS-specific task identifier data,
/// and `sys_data_size` must be the correct size of that data.
pub unsafe fn find_id_by_system_data(sys_data: *const (), sys_data_size: usize) -> Result<TaskId> {
    let mut task_id = MaybeUninit::uninit();
    check(ffi::OS_TaskFindIdBySystemData(
        task_id.as_mut_ptr(),
        sys_data as *const _,
        sys_data_size,
    ))?;
    Ok(TaskId(task_id.assume_init()))
}
