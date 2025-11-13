use crate::error::{check, Result};
use crate::ffi;
use crate::iface::Interface;

pub fn set(dest_address: u16, netmask: i8, iface: &Interface, via: u16) -> Result<()> {
    check(unsafe { ffi::csp_rtable_set(dest_address, netmask as i32, iface.as_ptr(), via) })
}

pub fn clear() {
    unsafe { ffi::csp_rtable_clear() }
}

pub fn free() {
    unsafe { ffi::csp_rtable_free() }
}

pub fn print() {
    unsafe { ffi::csp_rtable_print() }
}

pub fn load(rtable: &str) -> Result<()> {
    let c_str = rtable.as_bytes();
    let mut buf = [0u8; 256];
    let len = c_str.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&c_str[..len]);
    check(unsafe { ffi::csp_rtable_load(buf.as_ptr() as *const libc::c_char) })
}

pub fn check_str(rtable: &str) -> Result<()> {
    let c_str = rtable.as_bytes();
    let mut buf = [0u8; 256];
    let len = c_str.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&c_str[..len]);
    check(unsafe { ffi::csp_rtable_check(buf.as_ptr() as *const libc::c_char) })
}

pub fn save(buffer: &mut [u8]) -> Result<usize> {
    let result =
        unsafe { ffi::csp_rtable_save(buffer.as_mut_ptr() as *mut libc::c_char, buffer.len()) };
    if result < 0 {
        Err(crate::error::Error::Unknown(result))
    } else {
        Ok(result as usize)
    }
}

pub struct Route {
    ptr: *mut ffi::csp_route_t,
}

impl Route {
    pub(crate) fn from_raw(ptr: *mut ffi::csp_route_t) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    pub fn address(&self) -> u16 {
        unsafe { (*self.ptr).address }
    }

    pub fn netmask(&self) -> u16 {
        unsafe { (*self.ptr).netmask }
    }

    pub fn via(&self) -> u16 {
        unsafe { (*self.ptr).via }
    }
}

pub fn find_route(dest_address: u16) -> Option<Route> {
    let ptr = unsafe { ffi::csp_rtable_find_route(dest_address) };
    Route::from_raw(ptr)
}

unsafe extern "C" fn iterate_trampoline<F>(ctx: *mut libc::c_void, route: *mut ffi::csp_route_t) -> bool
where
    F: FnMut(&Route) -> bool,
{
    let closure = &mut *(ctx as *mut F);
    if let Some(r) = Route::from_raw(route) {
        closure(&r)
    } else {
        false
    }
}

pub fn iterate<F>(mut callback: F)
where
    F: FnMut(&Route) -> bool,
{
    unsafe {
        ffi::csp_rtable_iterate(
            Some(iterate_trampoline::<F>),
            &mut callback as *mut F as *mut libc::c_void,
        );
    }
}
