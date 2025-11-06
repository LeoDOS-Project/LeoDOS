//! Safe wrappers for WAMR memory operations.

use crate::{ffi, Instance, Result, WamrError};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::slice;
use heapless::String;

/// A smart pointer for memory allocated within a Wasm instance's heap via `module_malloc`.
///
/// This struct is an RAII guard that automatically calls `module_free` when it goes out of scope.
/// It provides safe access to the allocated memory as a slice.
pub struct ModulePtr<'i, T: ?Sized> {
    instance: &'i Instance<'i>,
    offset: u64,
    size: u64,
    _phantom: PhantomData<T>,
}

impl<'i> ModulePtr<'i, [u8]> {
    pub(crate) fn new(instance: &'i Instance, offset: u64, size: u64) -> Self {
        Self {
            instance,
            offset,
            size,
            _phantom: PhantomData,
        }
    }

    /// Returns the offset of the allocated memory within the instance's address space.
    pub fn offset(&self) -> u64 {
        self.offset
    }
}

impl<T: ?Sized> Drop for ModulePtr<'_, T> {
    fn drop(&mut self) {
        self.instance.module_free(self.offset);
    }
}

impl<'i> Deref for ModulePtr<'i, [u8]> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        // Safety: The pointer was valid on creation. `ModulePtr` holds a borrow on the `Instance`,
        // preventing the instance from being dropped. The main risk is memory growth, but safe
        // Rust code cannot cause that while holding an immutable borrow to this `ModulePtr`.
        unsafe {
            let native_ptr = self
                .instance
                .default_memory()
                .unwrap()
                .offset_to_native(self.offset)
                .unwrap();
            slice::from_raw_parts(native_ptr as *const u8, self.size as usize)
        }
    }
}

impl<'i> DerefMut for ModulePtr<'i, [u8]> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: Same as Deref, but we have a mutable reference to self, ensuring no other
        // pointers to this memory exist from the safe Rust side.
        unsafe {
            let native_ptr = self
                .instance
                .default_memory()
                .unwrap()
                .offset_to_native(self.offset)
                .unwrap();
            slice::from_raw_parts_mut(native_ptr, self.size as usize)
        }
    }
}

/// Represents an instance's linear memory.
///
/// Provides safe methods to interact with the Wasm memory space.
///
/// # Safety
///
/// A raw pointer obtained from this memory (e.g., via `offset_to_native`) can be
/// invalidated if the underlying Wasm memory is grown (e.g., by `memory.grow` in
/// Wasm or by calling `grow()` on this object). You must not use any pointers
/// after a memory growth operation.
pub struct Memory<'i> {
    ptr: NonNull<ffi::WASMMemoryInstance>,
    // Reference to the instance to keep it alive and for API calls
    _instance: &'i Instance<'i>,
}

impl<'i> Memory<'i> {
    pub(crate) fn new(ptr: NonNull<ffi::WASMMemoryInstance>, instance: &'i Instance<'i>) -> Self {
        Self {
            ptr,
            _instance: instance,
        }
    }

    /// Returns the current size of the memory in bytes.
    pub fn size(&self) -> u64 {
        let page_count = self.size_in_pages();
        let bytes_per_page = unsafe { ffi::wasm_memory_get_bytes_per_page(self.ptr.as_ptr()) };
        page_count * bytes_per_page
    }

    /// Returns the current size of the memory in WebAssembly pages.
    /// (Each page is 64 KiB).
    pub fn size_in_pages(&self) -> u64 {
        unsafe { ffi::wasm_memory_get_cur_page_count(self.ptr.as_ptr()) }
    }

    /// Grows the memory by a specified number of pages.
    pub fn grow(&mut self, pages: u64) -> Result<()> {
        if unsafe { ffi::wasm_memory_enlarge(self.ptr.as_ptr(), pages) } {
            Ok(())
        } else {
            let error_msg = self
                ._instance
                .get_exception()
                .unwrap_or_else(|| String::try_from("Failed to grow memory").unwrap());
            Err(WamrError::MemoryError(error_msg))
        }
    }

    /// Reads a slice of bytes from a given offset in the Wasm memory.
    ///
    /// Returns an error if the range is out of bounds.
    pub fn read(&self, offset: u64, len: u64) -> Result<&'i [u8]> {
        let end = offset.saturating_add(len);
        if end > self.size() {
            return Err(WamrError::MemoryError(
                String::try_from("Memory read out of bounds").unwrap(),
            ));
        }

        let native_ptr = self.offset_to_native(offset)?;
        // Safety: We've performed the bounds check above. The lifetime 'i is tied
        // to the Instance, ensuring this slice doesn't outlive the memory it points to.
        // Pointers can be invalidated on growth, but this method only takes &self,
        // so the caller cannot call grow() concurrently.
        unsafe { Ok(slice::from_raw_parts(native_ptr as *const u8, len as usize)) }
    }

    /// Reads a mutable slice of bytes from a given offset in the Wasm memory.
    ///
    /// Returns an error if the range is out of bounds.
    pub fn read_mut(&mut self, offset: u64, len: u64) -> Result<&'i mut [u8]> {
        let end = offset.saturating_add(len);
        if end > self.size() {
            return Err(WamrError::MemoryError(
                String::try_from("Memory read out of bounds").unwrap(),
            ));
        }

        let native_ptr = self.offset_to_native(offset)?;
        // Safety: See read(). &mut self prevents aliasing issues.
        unsafe {
            Ok(slice::from_raw_parts_mut(
                native_ptr as *mut u8,
                len as usize,
            ))
        }
    }

    /// Writes a slice of bytes to a given offset in the Wasm memory.
    ///
    /// Returns an error if the write would go out of bounds.
    pub fn write(&mut self, offset: u64, data: &[u8]) -> Result<()> {
        let slice = self.read_mut(offset, data.len() as u64)?;
        slice.copy_from_slice(data);
        Ok(())
    }

    /// Converts an offset within the Wasm memory (an "app address") to a
    /// native pointer in the host's address space.
    ///
    /// # Warning
    /// The returned pointer may be invalidated if the memory is grown.
    pub fn offset_to_native(&self, offset: u64) -> Result<*mut u8> {
        let ptr = unsafe { ffi::wasm_runtime_addr_app_to_native(self._instance.as_ptr(), offset) };
        if ptr.is_null() && offset > 0 {
            return Err(WamrError::MemoryError(
                String::try_from("Invalid memory offset").unwrap(),
            ));
        }
        Ok(ptr as *mut u8)
    }

    /// Converts a native pointer into the host's address space to an offset
    /// within the Wasm memory ("app address").
    pub fn native_to_offset(&self, native_ptr: *mut u8) -> u64 {
        unsafe {
            ffi::wasm_runtime_addr_native_to_app(self._instance.as_ptr(), native_ptr as *mut _)
        }
    }
}
