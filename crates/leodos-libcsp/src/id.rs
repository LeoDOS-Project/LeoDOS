use crate::ffi;
use crate::iface::Interface;

pub fn get_host_bits() -> u32 {
    unsafe { ffi::csp_id_get_host_bits() }
}

pub fn get_max_nodeid() -> u32 {
    unsafe { ffi::csp_id_get_max_nodeid() }
}

pub fn get_max_port() -> u32 {
    unsafe { ffi::csp_id_get_max_port() }
}

pub fn is_broadcast(addr: u16, iface: &Interface) -> bool {
    unsafe { ffi::csp_id_is_broadcast(addr, iface.as_ptr()) != 0 }
}

pub fn get_header_size() -> i32 {
    unsafe { ffi::csp_id_get_header_size() }
}
