//! Convergence Layer Adapter (CLA) contacts.
//!
//! A contact represents a communication link to a remote BP node. Contacts
//! are configured with a CLA type (UDP, TCP, EPP, LTP), remote address,
//! and a set of destination EID patterns that route through this contact.

use crate::bp::types::BpError;
use crate::bp::types::check_status;
use crate::bp::types::check_status_with_size;
use crate::ffi;

/// A handle to a specific CLA contact on a BP node.
pub struct Contact<'a> {
    inst: &'a mut ffi::BPLib_Instance_t,
    id: u32,
}

impl<'a> Contact<'a> {
    pub(crate) fn new(inst: &'a mut ffi::BPLib_Instance_t, id: u32) -> Self {
        Self { inst, id }
    }

    /// Sets up a contact (allocates resources, does not start communication).
    pub fn setup(&self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_CLA_ContactSetup(self.id) })
    }

    /// Starts a contact (begins bundle transmission and reception).
    pub fn start(&self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_CLA_ContactStart(self.id) })
    }

    /// Stops a contact (pauses communication, retains state).
    pub fn stop(&self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_CLA_ContactStop(self.id) })
    }

    /// Tears down a contact (releases all resources).
    pub fn teardown(&mut self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_CLA_ContactTeardown(self.inst, self.id) })
    }

    /// Feeds a raw bundle into the BP engine, blocking up to `timeout_ms`.
    pub fn ingress_blocking(&mut self, bundle: &[u8], timeout_ms: u32) -> Result<(), BpError> {
        check_status(unsafe {
            ffi::BPLib_CLA_Ingress(
                self.inst,
                self.id,
                bundle.as_ptr() as *const _,
                bundle.len(),
                timeout_ms,
            )
        })
    }

    /// Feeds a raw bundle into the BP engine, yielding until accepted.
    pub async fn ingress(&mut self, bundle: &[u8]) -> Result<(), BpError> {
        core::future::poll_fn(|_| match self.ingress_blocking(bundle, 0) {
            Err(BpError::Timeout) | Err(BpError::NoData) => core::task::Poll::Pending,
            result => core::task::Poll::Ready(result),
        })
        .await
    }

    /// Takes the next bundle for CLA transmission, blocking up to `timeout_ms`.
    pub fn egress_blocking(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, BpError> {
        let mut size = 0usize;
        let status = unsafe {
            ffi::BPLib_CLA_Egress(
                self.inst,
                self.id,
                buf.as_mut_ptr() as *mut _,
                &mut size,
                buf.len(),
                timeout_ms,
            )
        };
        check_status_with_size(status, size)
    }

    /// Takes the next bundle for CLA transmission, yielding until available.
    pub async fn egress(&mut self, buf: &mut [u8]) -> Result<usize, BpError> {
        core::future::poll_fn(|_| match self.egress_blocking(buf, 0) {
            Err(BpError::Timeout) | Err(BpError::NoData) => core::task::Poll::Pending,
            result => core::task::Poll::Ready(result),
        })
        .await
    }
}
