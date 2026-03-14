//! Safe, idiomatic wrappers for CFE Executive Services Task query APIs.

use crate::cfe::es::app::AppId;
use crate::error::{CfsError, OsalError, Result};
use crate::ffi;
use crate::os::task::TaskFlags;
use crate::status::check;
use core::ffi::CStr;
use core::mem::MaybeUninit;
use heapless::CString;

/// A type-safe, zero-cost wrapper for a cFE Task ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(pub ffi::CFE_ES_TaskId_t);

/// Type alias for a cFE child task entry point function.
pub type TaskEntryPoint = unsafe extern "C" fn();

/// A handle to a cFE child task.
///
/// This is a wrapper around a CFE-level task ID that will automatically call
/// `CFE_ES_DeleteChildTask` when it goes out of scope, preventing resource leaks.
/// It must not be used for an Application's Main Task.
#[derive(Debug)]
pub struct ChildTask {
    id: TaskId,
}

impl ChildTask {
    /// Creates a new cFE child task and starts it running.
    ///
    /// The new task is owned by the calling Application. The stack for the task
    /// can be provided or allocated from the system memory heap.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the task.
    /// * `entry_point`: The function that the new task will execute. This must
    ///   be a free function with `extern "C"` linkage.
    /// * `stack_ptr`: A pointer to the task's stack, or `core::ptr::null_mut()`
    ///   to have cFE allocate it.
    /// * `stack_size`: The size of the stack to allocate for the new task.
    /// * `priority`: The priority of the new task (0=highest, 255=lowest).
    /// * `flags`: Task creation flags (e.g. `TaskFlags::FP_ENABLED`).
    pub fn new(
        name: &str,
        entry_point: TaskEntryPoint,
        stack_ptr: *mut (),
        stack_size: usize,
        priority: u16,
        flags: TaskFlags,
    ) -> Result<Self> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;

        let mut task_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_ES_CreateChildTask(
                task_id.as_mut_ptr(),
                c_name.as_ptr(),
                Some(entry_point),
                stack_ptr as *mut _,
                stack_size,
                priority,
                flags.bits(),
            )
        })?;

        Ok(Self {
            id: TaskId(unsafe { task_id.assume_init() }),
        })
    }

    /// Returns the underlying `TaskId` for this child task.
    pub fn id(&self) -> TaskId {
        self.id
    }
}

impl Drop for ChildTask {
    /// Deletes the cFE child task when the `ChildTask` object goes out of scope.
    fn drop(&mut self) {
        // CFE_ES_DeleteChildTask can return an error, but we ignore it in drop.
        let _ = unsafe { ffi::CFE_ES_DeleteChildTask(self.id.0) };
    }
}

/// A high-level wrapper around the FFI's `CFE_ES_TaskInfo_t`.
#[derive(Debug, Clone)]
pub struct TaskInfo {
    inner: ffi::CFE_ES_TaskInfo_t,
}

impl TaskInfo {
    /// Returns the OSAL Task ID for this task.
    pub fn task_id(&self) -> TaskId {
        TaskId(self.inner.TaskId)
    }

    /// Returns the parent Application ID for this task.
    pub fn app_id(&self) -> AppId {
        AppId(self.inner.AppId)
    }

    /// Returns the execution counter for this task.
    pub fn execution_counter(&self) -> u32 {
        self.inner.ExecutionCounter
    }

    /// Returns the registered name of the task.
    pub fn name(&self) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
        let c_str = unsafe { CStr::from_ptr(self.inner.TaskName.as_ptr()) };
        let mut s = CString::new();
        s.extend_from_bytes(c_str.to_bytes())
            .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
        Ok(s)
    }

    /// Returns the registered name of the parent application.
    pub fn app_name(&self) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
        let c_str = unsafe { CStr::from_ptr(self.inner.AppName.as_ptr()) };
        let mut s = CString::new();
        s.extend_from_bytes(c_str.to_bytes())
            .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
        Ok(s)
    }
}

impl TaskId {
    /// Retrieves detailed information about the task with this ID.
    pub fn info(&self) -> Result<TaskInfo> {
        let mut task_info_uninit = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetTaskInfo(task_info_uninit.as_mut_ptr(), self.0) })?;
        Ok(TaskInfo {
            inner: unsafe { task_info_uninit.assume_init() },
        })
    }

    /// Deletes a child task with this ID.
    ///
    /// This function is a standalone wrapper for `CFE_ES_DeleteChildTask`. Using the
    /// `ChildTask` RAII struct is generally preferred to ensure the task is always deleted.
    /// It must not be called for an Application's Main Task.
    pub fn delete(&self) -> Result<()> {
        check(unsafe { ffi::CFE_ES_DeleteChildTask(self.0) })?;
        Ok(())
    }

    /// Retrieves the cFE Task Name for this task ID.
    pub fn name(&self) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
        let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
        check(unsafe {
            ffi::CFE_ES_GetTaskName(
                buffer.as_mut_ptr() as *mut libc::c_char,
                self.0,
                buffer.len(),
            )
        })?;

        // Find the null terminator to determine the actual length.
        let len = buffer.iter().position(|&b| b == 0).unwrap_or(0);
        let mut s = CString::new();
        s.extend_from_bytes(&buffer[..len])
            .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
        Ok(s)
    }

    /// Converts this CFE Task ID into a zero-based integer suitable for array indexing.
    pub fn to_index(&self) -> Result<u32> {
        let mut index = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_TaskID_ToIndex(self.0, index.as_mut_ptr()) })?;
        Ok(unsafe { index.assume_init() })
    }

    /// Retrieves the cFE Task ID for a given task name.
    pub fn from_name(name: &str) -> Result<Self> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;

        let mut task_id = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetTaskIDByName(task_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(TaskId(unsafe { task_id.assume_init() }))
    }

    /// Retrieves the CFE Task ID of the currently executing task.
    pub fn current() -> Result<Self> {
        let mut task_id = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetTaskID(task_id.as_mut_ptr()) })?;
        Ok(TaskId(unsafe { task_id.assume_init() }))
    }
}

/// Exits the calling child task.
///
/// This function terminates the currently running child task and does not return.
/// It must not be called from an Application's Main Task.
pub fn exit_child_task() -> ! {
    unsafe {
        ffi::CFE_ES_ExitChildTask();
    }
    // This function never returns, but we add a loop to satisfy the compiler.
    loop {}
}

/// Increments the execution counter for the calling task.
///
/// This is typically not needed for main application tasks that call `run_cycle`
/// (via `CFE_ES_RunLoop`), as the counter is incremented automatically.
/// It is useful for child tasks or other contexts where the counter needs to
/// be manually managed to indicate liveness.
pub fn increment_task_counter() {
    unsafe {
        ffi::CFE_ES_IncrementTaskCounter();
    }
}
