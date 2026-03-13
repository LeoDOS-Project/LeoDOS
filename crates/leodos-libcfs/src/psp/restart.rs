//! PSP restart and reset functions.

use crate::cfe::es::system::ResetSubtype;
pub use crate::cfe::es::system::ResetType;
use crate::ffi;
use core::mem::MaybeUninit;

/// Requests the PSP to restart the processor. This function does not return.
///
/// # Arguments
/// * `reset_type`: The type of reset to perform (`PowerOn` or `Processor`).
pub fn restart(reset_type: ResetType) -> ! {
    unsafe {
        ffi::CFE_PSP_Restart(reset_type.into());
    }
    // This function never returns.
    loop {}
}

/// Returns the last reset type and subtype recorded by the PSP.
pub fn get_restart_type() -> (ResetType, ResetSubtype) {
    let mut subtype = MaybeUninit::uninit();
    let reset_type = unsafe { ffi::CFE_PSP_GetRestartType(subtype.as_mut_ptr()) };
    (
        (reset_type as u32).into(),
        unsafe { subtype.assume_init() }.into(),
    )
}
