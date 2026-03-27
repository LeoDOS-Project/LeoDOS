//! Application channels (Payload Interface).
//!
//! A channel is the interface between a local application and the bundle
//! protocol engine. Applications send and receive Application Data Units
//! (ADUs) through channels. Each channel has a local service number,
//! destination EID, and configuration for bundle properties (lifetime,
//! CRC type, hop limit, etc.).

use crate::bp::types::check_status;
use crate::bp::types::check_status_with_size;
use crate::bp::types::BpError;
use crate::ffi;

/// A handle to a specific application channel on a BP node.
pub struct Channel<'a> {
    inst: &'a mut ffi::BPLib_Instance_t,
    id: u32,
}

impl<'a> Channel<'a> {
    pub(crate) fn new(inst: &'a mut ffi::BPLib_Instance_t, id: u32) -> Self {
        Self { inst, id }
    }

    /// Adds this application channel to the BP node.
    pub fn add(&self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_PI_AddApplication(self.id) })
    }

    /// Starts this application channel, enabling bundle ingress and egress.
    pub fn start(&self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_PI_StartApplication(self.id) })
    }

    /// Stops this application channel.
    pub fn stop(&self) -> Result<(), BpError> {
        check_status(unsafe { ffi::BPLib_PI_StopApplication(self.id) })
    }

    /// Sends an Application Data Unit (ADU) into the bundle protocol engine.
    pub fn send(&mut self, data: &[u8]) -> Result<(), BpError> {
        check_status(unsafe {
            ffi::BPLib_PI_Ingress(self.inst, self.id, data.as_ptr() as *mut _, data.len())
        })
    }

    /// Receives an ADU, blocking up to `timeout_ms` milliseconds.
    pub fn recv_blocking(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, BpError> {
        let mut size = 0usize;
        let status = unsafe {
            ffi::BPLib_PI_Egress(
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

    /// Receives an ADU, yielding to the async executor until data is available.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, BpError> {
        core::future::poll_fn(|_| match self.recv_blocking(buf, 0) {
            Err(BpError::Timeout) | Err(BpError::NoData) => core::task::Poll::Pending,
            result => core::task::Poll::Ready(result),
        })
        .await
    }
}
