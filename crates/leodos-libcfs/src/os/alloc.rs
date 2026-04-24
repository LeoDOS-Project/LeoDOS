//! A `GlobalAlloc` implementation backed by the POSIX heap
//! (`malloc`/`free`/`posix_memalign`).
//!
//! cFS apps that want to use the `alloc` crate (`Box`, `Vec`,
//! `String`, ...) declare this as their global allocator:
//!
//! ```ignore
//! use leodos_libcfs::os::alloc::CfsAllocator;
//!
//! #[global_allocator]
//! static ALLOCATOR: CfsAllocator = CfsAllocator;
//! ```
//!
//! Alternatively, apps can call [`crate::register_allocator!`]
//! at crate root to install the allocator with one line.
//!
//! Under NOS3 / posix-OSAL, `core-cpu1` is a normal Linux
//! process, so this just delegates to the C library's
//! thread-safe heap.

use core::alloc::GlobalAlloc;
use core::alloc::Layout;

/// A `GlobalAlloc` backed by libc's thread-safe heap.
pub struct CfsAllocator;

unsafe impl GlobalAlloc for CfsAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut out: *mut libc::c_void = core::ptr::null_mut();
        let align = layout.align().max(core::mem::size_of::<usize>());
        let size = layout.size();
        let status = unsafe { libc::posix_memalign(&mut out, align, size) };
        if status == 0 {
            out as *mut u8
        } else {
            core::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { libc::free(ptr as *mut libc::c_void) };
    }
}

/// Declares [`CfsAllocator`] as the crate's `#[global_allocator]`.
///
/// cFS apps place this at the crate root:
///
/// ```ignore
/// leodos_libcfs::register_allocator!();
/// ```
#[macro_export]
macro_rules! register_allocator {
    () => {
        #[global_allocator]
        static __LEODOS_GLOBAL_ALLOCATOR: $crate::os::alloc::CfsAllocator =
            $crate::os::alloc::CfsAllocator;
    };
}
