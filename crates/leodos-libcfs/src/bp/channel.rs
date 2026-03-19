//! Application channels (Payload Interface).
//!
//! A channel is the interface between a local application and the bundle
//! protocol engine. Applications send and receive Application Data Units
//! (ADUs) through channels. Each channel has a local service number,
//! destination EID, and configuration for bundle properties (lifetime,
//! CRC type, hop limit, etc.).

use crate::bp::eid::Eid;
use crate::bp::types::{CrcType, Status};
use crate::ffi;

/// Channel identifier (0-based, max BPLIB_MAX_NUM_CHANNELS).
pub type ChannelId = u32;

/// Adds an application channel to the BP node.
pub fn add(chan_id: ChannelId) -> Status {
    unsafe { ffi::BPLib_PI_AddApplication(chan_id) }
}

/// Starts an application channel, enabling bundle ingress and egress.
pub fn start(chan_id: ChannelId) -> Status {
    unsafe { ffi::BPLib_PI_StartApplication(chan_id) }
}

/// Stops an application channel.
pub fn stop(chan_id: ChannelId) -> Status {
    unsafe { ffi::BPLib_PI_StopApplication(chan_id) }
}

/// Sends an Application Data Unit (ADU) into the bundle protocol engine.
///
/// The ADU is wrapped in a bundle with the channel's configured destination
/// EID, lifetime, CRC type, and other properties. The bundle is queued for
/// transmission through the convergence layer.
pub fn send(inst: &mut ffi::BPLib_Instance_t, chan_id: ChannelId, data: &[u8]) -> Status {
    unsafe {
        ffi::BPLib_PI_Ingress(
            inst,
            chan_id,
            data.as_ptr() as *mut _,
            data.len(),
        )
    }
}

/// Receives an Application Data Unit (ADU) from the bundle protocol engine.
///
/// Blocks up to `timeout_ms` milliseconds waiting for a bundle addressed
/// to this channel's local service number. Returns the number of bytes
/// written to `buf`, or an error status.
pub fn recv(
    inst: &mut ffi::BPLib_Instance_t,
    chan_id: ChannelId,
    buf: &mut [u8],
    timeout_ms: u32,
) -> Result<usize, Status> {
    let mut size = 0usize;
    let status = unsafe {
        ffi::BPLib_PI_Egress(
            inst,
            chan_id,
            buf.as_mut_ptr() as *mut _,
            &mut size,
            buf.len(),
            timeout_ms,
        )
    };
    (status >= 0).then_some(size).ok_or(status)
}
