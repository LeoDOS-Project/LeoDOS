//! BP engine instance lifecycle.
//!
//! A `BpInstance` owns a bplib instance and its memory pool. It provides
//! safe access to channels (application I/O) and contacts (CLA I/O).

use crate::bp::channel;
use crate::bp::contact;
use crate::bp::types::Status;
use crate::ffi;

/// Initializes the bplib memory pool.
///
/// `pool` must point to a valid `BPLib_MEM_Pool_t`. `mem` is the backing
/// memory buffer that the pool will manage.
pub fn pool_init(pool: &mut ffi::BPLib_MEM_Pool_t, mem: &mut [u8]) -> Status {
    unsafe { ffi::BPLib_MEM_PoolInit(pool, mem.as_mut_ptr() as *mut _, mem.len()) }
}

/// Destroys a bplib memory pool.
pub fn pool_destroy(pool: &mut ffi::BPLib_MEM_Pool_t) {
    unsafe { ffi::BPLib_MEM_PoolDestroy(pool) }
}

/// Initializes the queue manager for a bplib instance.
pub fn queue_init(inst: &mut ffi::BPLib_Instance_t, max_jobs: usize) -> Status {
    unsafe { ffi::BPLib_QM_QueueTableInit(inst, max_jobs) }
}

/// Destroys the queue manager for a bplib instance.
pub fn queue_destroy(inst: &mut ffi::BPLib_Instance_t) {
    unsafe { ffi::BPLib_QM_QueueTableDestroy(inst) }
}

/// Registers a worker thread with the queue manager.
///
/// Returns the worker ID on success.
pub fn register_worker(inst: &mut ffi::BPLib_Instance_t) -> Result<i32, Status> {
    let mut worker_id: i32 = 0;
    let status = unsafe { ffi::BPLib_QM_RegisterWorker(inst, &mut worker_id) };
    (status >= 0).then_some(worker_id).ok_or(status)
}

/// Runs one job from the queue for the given worker.
///
/// Blocks up to `timeout_ms` milliseconds waiting for a job.
pub fn worker_run_job(
    inst: &mut ffi::BPLib_Instance_t,
    worker_id: i32,
    timeout_ms: i32,
) -> Status {
    unsafe { ffi::BPLib_QM_WorkerRunJob(inst, worker_id, timeout_ms) }
}

/// Initializes the bundle storage subsystem.
pub fn storage_init(inst: &mut ffi::BPLib_Instance_t) -> Status {
    unsafe { ffi::BPLib_STOR_Init(inst) }
}

/// Destroys the bundle storage subsystem.
pub fn storage_destroy(inst: &mut ffi::BPLib_Instance_t) {
    unsafe { ffi::BPLib_STOR_Destroy(inst) }
}

/// Flushes pending bundles from the insert batch to storage.
pub fn storage_flush(inst: &mut ffi::BPLib_Instance_t) -> Status {
    unsafe { ffi::BPLib_STOR_FlushPending(inst) }
}

/// Runs garbage collection on expired bundles.
pub fn storage_gc(inst: &mut ffi::BPLib_Instance_t) -> Status {
    unsafe { ffi::BPLib_STOR_GarbageCollect(inst) }
}

/// Initializes the CRC subsystem.
pub fn crc_init() {
    unsafe { ffi::BPLib_CRC_Init() }
}

/// Initializes the event management subsystem.
pub fn event_init() -> Status {
    unsafe { ffi::BPLib_EM_Init() }
}
