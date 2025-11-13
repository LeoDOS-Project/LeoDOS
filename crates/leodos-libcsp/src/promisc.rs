use crate::error::{check, Result};
use crate::ffi;
use crate::types::Packet;
use core::future::Future;
use core::task::Poll;

pub fn enable(queue_size: u32) -> Result<()> {
    check(unsafe { ffi::csp_promisc_enable(queue_size) })
}

pub fn disable() {
    unsafe { ffi::csp_promisc_disable() }
}

pub fn read(timeout_ms: u32) -> Option<Packet> {
    let ptr = unsafe { ffi::csp_promisc_read(timeout_ms) };
    Packet::from_raw(ptr)
}

pub fn read_async() -> impl Future<Output = Option<Packet>> {
    core::future::poll_fn(|_| match read(0) {
        Some(packet) => Poll::Ready(Some(packet)),
        None => Poll::Pending,
    })
}
