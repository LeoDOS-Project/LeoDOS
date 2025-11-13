use crate::ffi;
use crate::iface::Interface;

pub fn set_interfaces(if_a: &Interface, if_b: &Interface) {
    unsafe { ffi::csp_bridge_set_interfaces(if_a.as_ptr(), if_b.as_ptr()) }
}

pub fn work() {
    unsafe { ffi::csp_bridge_work() }
}
