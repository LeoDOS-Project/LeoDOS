//! Tests against cFE's UT stub libraries.
//!
//! Requires `make prep` to have built the stubs (default behavior
//! when ENABLE_UNIT_TESTS=1 is set in the Makefile). The
//! `cfs-stubs` feature on leodos-libcfs links the test binary
//! against libut_core_api_stubs / libut_psp_api_stubs /
//! libut_osapi_stubs / libut_assert.
//!
//! Run via `make cfs-test`.

#![cfg(feature = "cfs-stubs")]

mod shims;
mod ut;

use leodos_libcfs::ffi;

/// Smoke test: a cFE wrapper resolves through the stub link path.
/// If the stubs are missing or the search path is wrong, this fails
/// at link time.
#[test]
fn link_resolves_cfe_psp_get_spacecraft_id() {
    ut::reset_all();
    let _id = leodos_libcfs::cfe::es::system::get_spacecraft_id();
}

// TODO: scripted-return-value test for typed (non-int32) stubs.
// `UT_GenStub_GetReturnValue` reads from a per-stub return buffer
// that the default Basic handler does not auto-populate from
// `UT_SetDataBuffer`. Setting it requires either a hook function
// (UT_SetHookFunction) calling UT_Stub_CopyToReturnValue, or a
// custom handler. Add the FFI shims for those when the first real
// test needs to script a non-int return. For int32-returning
// stubs, `UT_SetDefaultReturnValue` works directly.

/// Two consecutive calls increment the stub's call counter, and
/// `UT_ResetState` clears it. Verifies the per-test reset hygiene
/// our other tests depend on.
#[test]
fn stub_count_increments_and_resets() {
    ut::reset_all();
    let _ = leodos_libcfs::cfe::es::system::get_spacecraft_id();
    let _ = leodos_libcfs::cfe::es::system::get_spacecraft_id();
    assert_eq!(ut::stub_count(ffi::CFE_PSP_GetSpacecraftId as usize), 2);

    ut::reset_all();
    assert_eq!(ut::stub_count(ffi::CFE_PSP_GetSpacecraftId as usize), 0);
}
