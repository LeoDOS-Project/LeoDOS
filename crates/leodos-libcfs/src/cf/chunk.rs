//! CFDP Chunk list types and functions for gap tracking.

use crate::ffi;

/// A single chunk representing a contiguous range of file data.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct Chunk(pub(crate) ffi::CF_Chunk_t);

impl Chunk {
    /// Returns the start offset of the chunk within the file.
    pub fn offset(&self) -> u32 {
        self.0.offset
    }

    /// Returns the size of the chunk.
    pub fn size(&self) -> u32 {
        self.0.size
    }
}

/// Chunk list for tracking file data gaps.
#[repr(transparent)]
pub struct ChunkList(pub(crate) ffi::CF_ChunkList_t);

impl ChunkList {
    /// Initializes a chunk list with the given memory buffer.
    ///
    /// # Safety
    /// The `chunks_mem` slice must remain valid for the lifetime of this ChunkList.
    pub unsafe fn init(chunks_mem: &mut [Chunk]) -> Self {
        let mut list = core::mem::zeroed::<ffi::CF_ChunkList_t>();
        ffi::CF_ChunkListInit(
            &mut list,
            chunks_mem.len() as u32,
            chunks_mem.as_mut_ptr() as *mut ffi::CF_Chunk_t,
        );
        Self(list)
    }

    /// Adds a chunk to the list.
    pub fn add(&mut self, offset: u32, size: u32) {
        unsafe { ffi::CF_ChunkListAdd(&mut self.0, offset, size) }
    }

    /// Resets the chunk list to empty.
    pub fn reset(&mut self) {
        unsafe { ffi::CF_ChunkListReset(&mut self.0) }
    }

    /// Removes data from the first chunk.
    pub fn remove_from_first(&mut self, size: u32) {
        unsafe { ffi::CF_ChunkList_RemoveFromFirst(&mut self.0, size) }
    }

    /// Returns the first chunk, if any.
    pub fn first_chunk(&self) -> Option<&Chunk> {
        let ptr = unsafe { ffi::CF_ChunkList_GetFirstChunk(&self.0) };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { &*(ptr as *const Chunk) })
        }
    }

    /// Returns the number of chunks in the list.
    pub fn count(&self) -> u32 {
        self.0.count
    }

    /// Returns the maximum number of chunks the list can hold.
    pub fn max_chunks(&self) -> u32 {
        self.0.max_chunks
    }

    /// Computes gaps in the chunk list.
    ///
    /// # Safety
    /// The callback must be valid and the total must be correct.
    pub unsafe fn compute_gaps<F>(&self, max_gaps: u32, total: u32, start: u32, mut callback: F) -> u32
    where
        F: FnMut(&ChunkList, &Chunk, *mut core::ffi::c_void),
    {
        unsafe extern "C" fn trampoline<F>(
            chunks: *const ffi::CF_ChunkList_t,
            chunk: *const ffi::CF_Chunk_t,
            context: *mut core::ffi::c_void,
        ) where
            F: FnMut(&ChunkList, &Chunk, *mut core::ffi::c_void),
        {
            let callback = &mut *(context as *mut F);
            callback(&*(chunks as *const ChunkList), &*(chunk as *const Chunk), core::ptr::null_mut());
        }
        let callback_ptr = &mut callback as *mut F as *mut core::ffi::c_void;
        ffi::CF_ChunkList_ComputeGaps(
            &self.0,
            max_gaps,
            total,
            start,
            Some(trampoline::<F>),
            callback_ptr,
        )
    }
}

/// Erases a range of chunks from the list.
pub fn chunks_erase_range(chunks: &mut ChunkList, start: u32, end: u32) {
    unsafe { ffi::CF_Chunks_EraseRange(&mut chunks.0, start, end) }
}

/// Erases a single chunk at the given index.
pub fn chunks_erase_chunk(chunks: &mut ChunkList, index: u32) {
    unsafe { ffi::CF_Chunks_EraseChunk(&mut chunks.0, index) }
}

/// Inserts a chunk at the given index.
pub fn chunks_insert_chunk(chunks: &mut ChunkList, index: u32, chunk: &Chunk) {
    unsafe { ffi::CF_Chunks_InsertChunk(&mut chunks.0, index, &chunk.0) }
}

/// Finds the insert position for a new chunk.
pub fn chunks_find_insert_position(chunks: &mut ChunkList, chunk: &Chunk) -> u32 {
    unsafe { ffi::CF_Chunks_FindInsertPosition(&mut chunks.0, &chunk.0) }
}

/// Combines the chunk at the given index with the previous chunk if possible.
pub fn chunks_combine_previous(chunks: &mut ChunkList, index: u32) -> i32 {
    unsafe { ffi::CF_Chunks_CombinePrevious(&mut chunks.0, index, &chunks.0.chunks.add(index as usize).read()) }
}

/// Combines the chunk at the given index with the next chunk if possible.
pub fn chunks_combine_next(chunks: &mut ChunkList, index: u32) -> i32 {
    unsafe { ffi::CF_Chunks_CombineNext(&mut chunks.0, index, &chunks.0.chunks.add(index as usize).read()) }
}

/// Finds the smallest chunk in the list and returns its index.
pub fn chunks_find_smallest_size(chunks: &ChunkList) -> u32 {
    unsafe { ffi::CF_Chunks_FindSmallestSize(&chunks.0) }
}

/// Inserts a chunk into the list at the given index.
pub fn chunks_insert(chunks: &mut ChunkList, index: u32, chunk: &Chunk) {
    unsafe { ffi::CF_Chunks_Insert(&mut chunks.0, index, &chunk.0) }
}
