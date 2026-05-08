//! Smoke test: compile-time + link-time check that `cargo test
//! --features=cfs-stubs` resolves cFE / OSAL / PSP symbols against the
//! UT stub libraries built by `make prep && make` with
//! ENABLE_UNIT_TESTS=1.
//!
//! This file does not exercise any specific behavior — it only proves
//! that the link path is wired correctly. Behavior tests live in
//! per-module test files alongside the wrappers.

#![cfg(feature = "cfs-stubs")]

mod shims;

/// Trivial reference to a cFE FFI symbol so the linker is forced to
/// resolve it from the stub library. If the stubs are missing or the
/// link search path is wrong, this test fails to build.
#[test]
fn link_resolves_cfe_psp_get_spacecraft_id() {
    let id = leodos_libcfs::cfe::es::system::get_spacecraft_id();
    // The stub's default return for an int-returning function is 0.
    // Either way we just need the call to link and not crash.
    let _ = id;
}
