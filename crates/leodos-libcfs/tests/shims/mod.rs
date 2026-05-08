//! Stub implementations for the BSP-side of cFE's UT framework.
//!
//! `libut_assert.a` ships `utbsp.c`, which expects an OSAL BSP plus a
//! user-supplied `UtTest_Setup` to act as the test entry point. With
//! cargo's own `#[test]` harness providing `main`, we never reach that
//! code path — but the symbols are still referenced by `utbsp.c.o`
//! when it gets pulled in via ut_assert dependencies, so the linker
//! needs to find *something*. These no-op stubs satisfy the link.
//!
//! All functions here are `extern "C"` and unreachable in practice;
//! they exist only to keep ld(1) happy.

use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_void;
use core::ptr;

#[no_mangle]
pub extern "C" fn OS_BSP_Lock_Impl() {}

#[no_mangle]
pub extern "C" fn OS_BSP_Unlock_Impl() {}

#[no_mangle]
pub extern "C" fn OS_BSP_GetArgC() -> u32 {
    0
}

#[no_mangle]
pub extern "C" fn OS_BSP_GetArgV() -> *const *const c_char {
    ptr::null()
}

#[no_mangle]
pub extern "C" fn OS_BSP_ConsoleOutput_Impl(_str: *const c_char, _str_len: usize) {}

#[no_mangle]
pub extern "C" fn OS_BSP_ConsoleSetMode_Impl(_mode: u32) {}

#[no_mangle]
pub extern "C" fn OS_BSP_Shutdown_Impl() {}

#[no_mangle]
pub extern "C" fn OS_BSP_SetExitCode(_code: c_int) {}

#[no_mangle]
pub extern "C" fn UtTest_Setup() {}

// Keep the items used so the compiler doesn't strip them.
#[allow(dead_code)]
fn _force_link() -> *const c_void {
    OS_BSP_Lock_Impl as *const c_void
}
