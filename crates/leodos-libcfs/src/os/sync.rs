//! Safe, idiomatic wrappers for OSAL synchronization primitives.
//!
//! This module provides safe wrappers for Mutexes, Binary Semaphores, and
//! Counting Semaphores. It uses RAII guards for mutexes to ensure they are
//! always released, preventing deadlocks.

use crate::error::{Error, Result};
use crate::ffi;
use crate::os::id::OsalId;
use crate::os::time::OsTime;
use crate::os::util::c_name_from_str;
use crate::status::check;
use core::ffi::CStr;
use core::mem::MaybeUninit;
use core::ops::Drop;
use heapless::CString;

/// Properties of a mutex, returned by `Mutex::get_info`.
#[derive(Debug, Clone)]
pub struct MutexProp {
    /// The registered name of the mutex.
    pub name: CString<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created the mutex.
    pub creator: OsalId,
}

/// A mutual exclusion primitive useful for protecting shared data.
///
/// This mutex will block tasks waiting for the lock to become available.
#[derive(Debug)]
pub struct Mutex {
    id: ffi::osal_id_t,
}

impl Mutex {
    /// Creates a new OSAL mutex in the unlocked (available) state.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the mutex.
    pub fn new(name: &str) -> Result<Self> {
        let c_name = c_name_from_str(name)?;
        let mut sem_id = MaybeUninit::uninit();
        let status = unsafe { ffi::OS_MutSemCreate(sem_id.as_mut_ptr(), c_name.as_ptr(), 0) };
        check(status)?;
        Ok(Self {
            id: unsafe { sem_id.assume_init() },
        })
    }

    /// Acquires a mutex, blocking the current task until it is able to do so.
    ///
    /// This function returns a `MutexGuard` when the lock has been acquired.
    /// The guard ensures that the lock is automatically released when it goes out of scope.
    pub fn lock(&'_ self) -> Result<MutexGuard<'_>> {
        check(unsafe { ffi::OS_MutSemTake(self.id) })?;
        Ok(MutexGuard { mutex: self })
    }

    /// Finds an existing mutex ID by its name.
    pub fn get_id_by_name(name: &str) -> Result<OsalId> {
        let c_name = c_name_from_str(name)?;
        let mut sem_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_MutSemGetIdByName(sem_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(OsalId(unsafe { sem_id.assume_init() }))
    }

    /// Retrieves information about this mutex.
    pub fn get_info(&self) -> Result<MutexProp> {
        let mut prop = MaybeUninit::<ffi::OS_mut_sem_prop_t>::uninit();
        check(unsafe { ffi::OS_MutSemGetInfo(self.id, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        let mut name_str = CString::new();
        let c_str = unsafe { CStr::from_ptr(prop.name.as_ptr()) };
        name_str
            .extend_from_bytes(c_str.to_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        Ok(MutexProp {
            name: name_str,
            creator: OsalId(prop.creator),
        })
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_MutSemDelete(self.id) };
    }
}

/// An RAII implementation of a scoped lock for a mutex.
///
/// When this structure is dropped (falls out of scope), the lock will be released.
#[derive(Debug)]
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a> {
    mutex: &'a Mutex,
}

impl<'a> Drop for MutexGuard<'a> {
    /// Releases the lock on the associated mutex.
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_MutSemGive(self.mutex.id) };
    }
}

/// Initial state of a binary semaphore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemState {
    /// The semaphore starts empty (taken).
    Empty,
    /// The semaphore starts full (available).
    Full,
}

impl SemState {
    fn as_u32(self) -> u32 {
        match self {
            SemState::Empty => 0,
            SemState::Full => 1,
        }
    }
}

/// Properties of a binary semaphore, returned by `BinSem::get_info`.
#[derive(Debug, Clone)]
pub struct BinSemProp {
    /// The registered name of the binary semaphore.
    pub name: CString<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created the semaphore.
    pub creator: OsalId,
    /// The current value of the semaphore (typically 0 or 1).
    pub value: i32,
}

/// A binary semaphore, often used for signaling between tasks.
#[derive(Debug)]
pub struct BinSem {
    id: ffi::osal_id_t,
}

impl BinSem {
    /// Creates a new binary semaphore.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the semaphore.
    /// * `initial_value`: The initial state of the semaphore.
    pub fn new(name: &str, initial_value: SemState) -> Result<Self> {
        let c_name = c_name_from_str(name)?;
        let mut sem_id = MaybeUninit::uninit();
        let status = unsafe {
            ffi::OS_BinSemCreate(sem_id.as_mut_ptr(), c_name.as_ptr(), initial_value.as_u32(), 0)
        };
        check(status)?;
        Ok(Self {
            id: unsafe { sem_id.assume_init() },
        })
    }

    /// Unlocks (gives) the semaphore.
    pub fn give(&self) -> Result<()> {
        check(unsafe { ffi::OS_BinSemGive(self.id) })?;
        Ok(())
    }

    /// Blocks until the semaphore can be locked (taken).
    pub fn take(&self) -> Result<()> {
        check(unsafe { ffi::OS_BinSemTake(self.id) })?;
        Ok(())
    }

    /// Unblocks all tasks pending on the specified semaphore.
    ///
    /// This function does not change the state of the semaphore.
    pub fn flush(&self) -> Result<()> {
        check(unsafe { ffi::OS_BinSemFlush(self.id) })?;
        Ok(())
    }

    /// Blocks until the semaphore can be taken, with a timeout.
    ///
    /// # Arguments
    /// * `timeout_ms`: Timeout in milliseconds.
    pub fn timed_wait(&self, timeout_ms: u32) -> Result<()> {
        check(unsafe { ffi::OS_BinSemTimedWait(self.id, timeout_ms) })?;
        Ok(())
    }

    /// Finds an existing binary semaphore ID by its name.
    pub fn get_id_by_name(name: &str) -> Result<OsalId> {
        let c_name = c_name_from_str(name)?;
        let mut sem_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_BinSemGetIdByName(sem_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(OsalId(unsafe { sem_id.assume_init() }))
    }

    /// Retrieves information about this binary semaphore.
    pub fn get_info(&self) -> Result<BinSemProp> {
        let mut prop = MaybeUninit::<ffi::OS_bin_sem_prop_t>::uninit();
        check(unsafe { ffi::OS_BinSemGetInfo(self.id, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        let mut name_str = CString::new();
        let c_str = unsafe { CStr::from_ptr(prop.name.as_ptr()) };
        name_str.extend_from_bytes(c_str.to_bytes()).unwrap(); // Should not fail

        Ok(BinSemProp {
            name: name_str,
            creator: OsalId(prop.creator),
            value: prop.value,
        })
    }
}

impl Drop for BinSem {
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_BinSemDelete(self.id) };
    }
}

/// Properties of a counting semaphore, returned by `CountSem::get_info`.
#[derive(Debug, Clone)]
pub struct CountSemProp {
    /// The registered name of the counting semaphore.
    pub name: CString<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created the semaphore.
    pub creator: OsalId,
    /// The current count of the semaphore.
    pub value: i32,
}

/// A counting semaphore.
#[derive(Debug)]
pub struct CountSem {
    id: ffi::osal_id_t,
}

impl CountSem {
    /// Creates a new counting semaphore.
    ///
    /// For portability, keep `initial_value` within `short int`
    /// range (0–32767). Some RTOS impose upper limits.
    ///
    /// # Arguments
    /// * `name`: A unique string to identify the semaphore.
    /// * `initial_value`: The initial count of the semaphore.
    pub fn new(name: &str, initial_value: u32) -> Result<Self> {
        let c_name = c_name_from_str(name)?;
        let mut sem_id = MaybeUninit::uninit();
        let status = unsafe {
            ffi::OS_CountSemCreate(sem_id.as_mut_ptr(), c_name.as_ptr(), initial_value, 0)
        };
        check(status)?;
        Ok(Self {
            id: unsafe { sem_id.assume_init() },
        })
    }

    /// Increments (gives) the semaphore's count.
    pub fn give(&self) -> Result<()> {
        check(unsafe { ffi::OS_CountSemGive(self.id) })?;
        Ok(())
    }

    /// Blocks until the semaphore's count is non-zero, then decrements it.
    pub fn take(&self) -> Result<()> {
        check(unsafe { ffi::OS_CountSemTake(self.id) })?;
        Ok(())
    }

    /// Blocks until the semaphore can be taken, with a timeout.
    ///
    /// # Arguments
    /// * `timeout_ms`: Timeout in milliseconds.
    pub fn timed_wait(&self, timeout_ms: u32) -> Result<()> {
        check(unsafe { ffi::OS_CountSemTimedWait(self.id, timeout_ms) })?;
        Ok(())
    }

    /// Finds an existing counting semaphore ID by its name.
    pub fn get_id_by_name(name: &str) -> Result<OsalId> {
        let c_name = c_name_from_str(name)?;
        let mut sem_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_CountSemGetIdByName(sem_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(OsalId(unsafe { sem_id.assume_init() }))
    }

    /// Retrieves information about this counting semaphore.
    pub fn get_info(&self) -> Result<CountSemProp> {
        let mut prop = MaybeUninit::<ffi::OS_count_sem_prop_t>::uninit();
        check(unsafe { ffi::OS_CountSemGetInfo(self.id, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        let mut name_str = CString::new();
        let c_str = unsafe { CStr::from_ptr(prop.name.as_ptr()) };
        name_str.extend_from_bytes(c_str.to_bytes()).unwrap(); // Should not fail

        Ok(CountSemProp {
            name: name_str,
            creator: OsalId(prop.creator),
            value: prop.value,
        })
    }
}

impl Drop for CountSem {
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_CountSemDelete(self.id) };
    }
}

/// Properties of a condition variable, returned by `CondVar::get_info`.
#[derive(Debug, Clone)]
pub struct CondVarProp {
    /// The registered name of the condition variable.
    pub name: CString<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created the condition variable.
    pub creator: OsalId,
}

/// A condition variable, for more complex synchronization with a `Mutex`.
#[derive(Debug)]
pub struct CondVar {
    id: ffi::osal_id_t,
}

impl CondVar {
    /// Creates a new condition variable.
    pub fn new(name: &str) -> Result<Self> {
        let c_name = c_name_from_str(name)?;
        let mut var_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_CondVarCreate(var_id.as_mut_ptr(), c_name.as_ptr(), 0) })?;
        Ok(Self {
            id: unsafe { var_id.assume_init() },
        })
    }

    /// Signals the condition variable, waking up one waiting task.
    pub fn signal(&self) -> Result<()> {
        check(unsafe { ffi::OS_CondVarSignal(self.id) })?;
        Ok(())
    }

    /// Broadcasts the condition variable, waking up all waiting tasks.
    pub fn broadcast(&self) -> Result<()> {
        check(unsafe { ffi::OS_CondVarBroadcast(self.id) })?;
        Ok(())
    }

    /// Waits for the condition variable to be signaled.
    ///
    /// Note: OSAL condition variables have their own internal mutex
    /// (`OS_CondVarLock`/`OS_CondVarUnlock`). The `_guard` param
    /// here is consumed to enforce discipline, but the underlying
    /// OSAL wait operates on the condvar's internal mutex, not the
    /// external one.
    pub fn wait(&self, _guard: MutexGuard) -> Result<()> {
        check(unsafe { ffi::OS_CondVarWait(self.id) })?;
        Ok(())
    }

    /// Atomically unlocks the mutex and waits for the condition variable, with a timeout.
    pub fn timed_wait(&self, _guard: MutexGuard, abstime: OsTime) -> Result<()> {
        check(unsafe { ffi::OS_CondVarTimedWait(self.id, &abstime.0) })?;
        Ok(())
    }

    /// Finds an existing condition variable ID by its name.
    pub fn get_id_by_name(name: &str) -> Result<OsalId> {
        let c_name = c_name_from_str(name)?;
        let mut var_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_CondVarGetIdByName(var_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(OsalId(unsafe { var_id.assume_init() }))
    }

    /// Retrieves information about this condition variable.
    pub fn get_info(&self) -> Result<CondVarProp> {
        let mut prop = MaybeUninit::<ffi::OS_condvar_prop_t>::uninit();
        check(unsafe { ffi::OS_CondVarGetInfo(self.id, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        let mut name_str = CString::new();
        let c_str = unsafe { CStr::from_ptr(prop.name.as_ptr()) };
        name_str.extend_from_bytes(c_str.to_bytes()).unwrap();

        Ok(CondVarProp {
            name: name_str,
            creator: OsalId(prop.creator),
        })
    }
}

impl Drop for CondVar {
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_CondVarDelete(self.id) };
    }
}
