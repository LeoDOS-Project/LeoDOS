//! PSP restart and reset functions.

use crate::cfe::es::system::ResetSubtype;
use crate::ffi;
use core::mem::MaybeUninit;

/// Requests the PSP to restart the processor. This function does not return.
///
/// # C-API Mapping
/// This is a safe wrapper for `CFE_PSP_Restart`.
///
/// # Arguments
/// * `reset_type`: The type of reset to perform (e.g., `ffi::CFE_PSP_RST_TYPE_PROCESSOR`).
pub fn restart(reset_type: u32) -> ! {
    unsafe {
        ffi::CFE_PSP_Restart(reset_type);
    }
    // This function never returns.
    loop {}
}

/// Returns the last reset type and subtype recorded by the PSP.
///
/// # C-API Mapping
/// This is a safe wrapper for `CFE_PSP_GetRestartType`.
pub fn get_restart_type() -> (u32, ResetSubtype) {
    let mut subtype = MaybeUninit::uninit();
    let reset_type = unsafe { ffi::CFE_PSP_GetRestartType(subtype.as_mut_ptr()) };
    (reset_type, unsafe { subtype.assume_init() }.into())
}
