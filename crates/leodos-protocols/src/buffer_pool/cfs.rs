//! [`BufferPool`] implementation over a cFE Executive Services memory pool.
//!
//! Routes pool allocations through `CFE_ES_GetPoolBuf` /
//! `CFE_ES_PutPoolBuf` (via `leodos-libcfs`'s safe wrapper). cFE
//! pools provide flight-grade properties: deterministic allocation
//! time, bounded memory, no fragmentation, and auditable usage via
//! `CFE_ES_GetMemPoolStats`.
//!
//! Alignment from the [`Layout`] argument is satisfied implicitly
//! by the pool's internal bucket layout — cFE rounds requests up to
//! the next available bucket, all of which are aligned to at least
//! `CFE_PLATFORM_ES_MEMPOOL_ALIGN_SIZE_MIN` (typically 4 bytes).
//! Callers needing higher alignment must size their `Layout`
//! accordingly or use a custom-bucket pool created via
//! `MemPool::new_ex`.

use core::alloc::Layout;

use leodos_libcfs::cfe::es::pool::MemPool;
use leodos_libcfs::cfe::es::pool::PoolBuffer;
use leodos_libcfs::error::CfsError;

use crate::buffer_pool::BufferPool;

impl BufferPool for MemPool {
    type Buf<'a> = PoolBuffer<'a>;
    type Error = CfsError;

    fn alloc(&self, layout: Layout) -> Result<Self::Buf<'_>, Self::Error> {
        self.get_buf(layout.size())
    }
}
