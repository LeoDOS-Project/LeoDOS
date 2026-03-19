//! Convergence Layer Adapter (CLA) contacts.
//!
//! A contact represents a communication link to a remote BP node. Contacts
//! are configured with a CLA type (UDP, TCP, EPP, LTP), remote address,
//! and a set of destination EID patterns that route through this contact.

use crate::bp::types::Status;
use crate::ffi;

/// Contact identifier (0-based, max BPLIB_MAX_NUM_CONTACTS).
pub type ContactId = u32;

/// Sets up a contact (allocates resources, does not start communication).
pub fn setup(contact_id: ContactId) -> Status {
    unsafe { ffi::BPLib_CLA_ContactSetup(contact_id) }
}

/// Starts a contact (begins bundle transmission and reception).
pub fn start(contact_id: ContactId) -> Status {
    unsafe { ffi::BPLib_CLA_ContactStart(contact_id) }
}

/// Stops a contact (pauses communication, retains state).
pub fn stop(contact_id: ContactId) -> Status {
    unsafe { ffi::BPLib_CLA_ContactStop(contact_id) }
}

/// Tears down a contact (releases all resources).
pub fn teardown(inst: &mut ffi::BPLib_Instance_t, contact_id: ContactId) -> Status {
    unsafe { ffi::BPLib_CLA_ContactTeardown(inst, contact_id) }
}

/// Feeds a raw bundle received from the CLA into the BP engine.
pub fn ingress(
    inst: &mut ffi::BPLib_Instance_t,
    contact_id: ContactId,
    bundle: &[u8],
    timeout_ms: u32,
) -> Status {
    unsafe {
        ffi::BPLib_CLA_Ingress(
            inst,
            contact_id,
            bundle.as_ptr() as *const _,
            bundle.len(),
            timeout_ms,
        )
    }
}

/// Takes the next bundle from the BP engine for transmission through the CLA.
///
/// Returns the number of bytes written to `buf`, or an error status.
pub fn egress(
    inst: &mut ffi::BPLib_Instance_t,
    contact_id: ContactId,
    buf: &mut [u8],
    timeout_ms: u32,
) -> Result<usize, Status> {
    let mut size = 0usize;
    let status = unsafe {
        ffi::BPLib_CLA_Egress(
            inst,
            contact_id,
            buf.as_mut_ptr() as *mut _,
            &mut size,
            buf.len(),
            timeout_ms,
        )
    };
    (status >= 0).then_some(size).ok_or(status)
}
