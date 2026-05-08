//! Safe Rust wrappers over the cFE UT framework's scriptable-stub API.
//!
//! Bindings here are minimal: just the four entry points needed to
//! reset stub state, script a return, and inspect call counts. Add
//! more as tests require.

use core::ffi::c_void;

#[link(name = "ut_assert")]
unsafe extern "C" {
    fn UT_ResetState(func_key: usize);
    fn UT_SetDefaultReturnValue(func_key: usize, value: isize);
    fn UT_GetStubCount(func_key: usize) -> u32;
    fn UT_SetDataBuffer(
        func_key: usize,
        data_buffer: *mut c_void,
        buffer_size: usize,
        allocate_copy: bool,
    );
}

/// Reset all stub state — clears scripted returns, hooks, deferred
/// returns, and call counters. Pass `0` for the C-side `func_key=0`
/// sentinel that resets every entry.
pub fn reset_all() {
    unsafe { UT_ResetState(0) };
}

/// Configure the stub for `func_key` to return `value`. Use for
/// stubs whose generator emits `UT_GenStub_SetDefaultReturnValue` —
/// typically functions returning an `int32` status code.
#[allow(dead_code)]
pub fn set_default_return(func_key: usize, value: isize) {
    unsafe { UT_SetDefaultReturnValue(func_key, value) };
}

/// Number of times the stub for `func_key` has been invoked since the
/// last reset.
pub fn stub_count(func_key: usize) -> u32 {
    unsafe { UT_GetStubCount(func_key) }
}

/// Provide a typed value the stub will copy into its return buffer.
///
/// cFE's newer UT_GenStub mechanism reads non-int return values from
/// a per-stub data buffer; this is the entry point for scripting them.
/// `value` must outlive the call (or be `'static` / on the heap with
/// `allocate_copy = true`).
///
/// # Safety
/// `value` must point to at least `core::mem::size_of::<T>()` bytes
/// of valid memory matching the stubbed function's return type.
pub unsafe fn set_return_value<T>(func_key: usize, value: &mut T) {
    UT_SetDataBuffer(
        func_key,
        value as *mut T as *mut c_void,
        core::mem::size_of::<T>(),
        false,
    );
}
