//! BP engine instance lifecycle.
//!
//! `BpNode` owns a bplib instance and its memory pool. It provides safe
//! access to channels (application I/O) and contacts (CLA I/O).

use core::marker::PhantomData;
use core::mem::MaybeUninit;

use crate::bp::channel::Channel;
use crate::bp::contact::Contact;
use crate::bp::types::BpError;
use crate::bp::types::check_status;
use crate::ffi;

/// A BP engine node.
///
/// # Safety
///
/// `pool_init` hands the backing memory pointer to the C library, which
/// stores it internally. The caller must ensure that the `&mut [u8]`
/// passed to [`BpNode::new`] outlives the `BpNode`. `PhantomData<*mut u8>`
/// marks the struct as holding a raw pointer so it is neither `Send` nor
/// `Sync` by default.
pub struct BpNode {
    inst: ffi::BPLib_Instance_t,
    pool: ffi::BPLib_MEM_Pool_t,
    _marker: PhantomData<*mut u8>,
}

impl BpNode {
    /// Initializes a new BP node backed by `mem`.
    ///
    /// `max_jobs` is the maximum number of concurrent worker jobs in the
    /// queue manager. The caller must ensure `mem` outlives the returned
    /// `BpNode`.
    pub fn new(mem: &mut [u8], max_jobs: usize) -> Result<Self, BpError> {
        let mut pool = unsafe { MaybeUninit::<ffi::BPLib_MEM_Pool_t>::zeroed().assume_init() };
        check_status(unsafe {
            ffi::BPLib_MEM_PoolInit(&mut pool, mem.as_mut_ptr() as *mut _, mem.len())
        })?;

        let mut inst = unsafe { MaybeUninit::<ffi::BPLib_Instance_t>::zeroed().assume_init() };
        check_status(unsafe { ffi::BPLib_QM_QueueTableInit(&mut inst, max_jobs) })?;
        check_status(unsafe { ffi::BPLib_STOR_Init(&mut inst) })?;
        unsafe { ffi::BPLib_CRC_Init() };
        check_status(unsafe { ffi::BPLib_EM_Init() })?;

        Ok(Self {
            inst,
            pool,
            _marker: PhantomData,
        })
    }

    /// Returns a typed channel handle for the given channel ID.
    pub fn channel(&mut self, id: u32) -> Channel<'_> {
        Channel::new(&mut self.inst, id)
    }

    /// Returns a typed contact handle for the given contact ID.
    pub fn contact(&mut self, id: u32) -> Contact<'_> {
        Contact::new(&mut self.inst, id)
    }

    /// Registers a worker thread with the queue manager.
    pub fn register_worker(&mut self) -> Result<i32, BpError> {
        let mut worker_id: i32 = 0;
        check_status(unsafe { ffi::BPLib_QM_RegisterWorker(&mut self.inst, &mut worker_id) })?;
        Ok(worker_id)
    }

    /// Runs one job from the queue for the given worker.
    ///
    /// Blocks up to `timeout_ms` milliseconds waiting for a job.
    pub fn worker_run_job(&mut self, worker_id: i32, timeout_ms: i32) -> Result<(), BpError> {
        check_status(unsafe {
            ffi::BPLib_QM_WorkerRunJob(&mut self.inst, worker_id, timeout_ms)
        })
    }

    /// Flushes pending bundles from the insert batch to storage.
    pub fn flush_storage(&mut self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_STOR_FlushPending(&mut self.inst) })
    }

    /// Runs garbage collection on expired bundles.
    pub fn garbage_collect(&mut self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_STOR_GarbageCollect(&mut self.inst) })
    }
}

impl Drop for BpNode {
    fn drop(&mut self) {
        unsafe {
            ffi::BPLib_STOR_Destroy(&mut self.inst);
            ffi::BPLib_QM_QueueTableDestroy(&mut self.inst);
            ffi::BPLib_MEM_PoolDestroy(&mut self.pool);
        }
    }
}
