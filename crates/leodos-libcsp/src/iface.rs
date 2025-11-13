use crate::ffi;

pub struct Interface {
    ptr: *mut ffi::csp_iface_t,
}

impl Interface {
    pub(crate) fn from_raw(ptr: *mut ffi::csp_iface_t) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut ffi::csp_iface_t {
        self.ptr
    }

    pub fn name(&self) -> &str {
        unsafe {
            let name_ptr = (*self.ptr).name;
            if name_ptr.is_null() {
                ""
            } else {
                let c_str = core::ffi::CStr::from_ptr(name_ptr);
                c_str.to_str().unwrap_or("")
            }
        }
    }

    pub fn addr(&self) -> u16 {
        unsafe { (*self.ptr).addr }
    }

    pub fn netmask(&self) -> u16 {
        unsafe { (*self.ptr).netmask }
    }

    pub fn tx_count(&self) -> u32 {
        unsafe { (*self.ptr).tx }
    }

    pub fn rx_count(&self) -> u32 {
        unsafe { (*self.ptr).rx }
    }

    pub fn tx_error(&self) -> u32 {
        unsafe { (*self.ptr).tx_error }
    }

    pub fn rx_error(&self) -> u32 {
        unsafe { (*self.ptr).rx_error }
    }

    pub fn drop_count(&self) -> u32 {
        unsafe { (*self.ptr).drop }
    }

    pub fn tx_bytes(&self) -> u32 {
        unsafe { (*self.ptr).txbytes }
    }

    pub fn rx_bytes(&self) -> u32 {
        unsafe { (*self.ptr).rxbytes }
    }

    pub fn is_default(&self) -> bool {
        unsafe { (*self.ptr).is_default != 0 }
    }
}

pub fn get_by_name(name: &str) -> Option<Interface> {
    let c_str = name.as_bytes();
    let mut buf = [0u8; 32];
    let len = c_str.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&c_str[..len]);
    let ptr = unsafe { ffi::csp_iflist_get_by_name(buf.as_ptr() as *const libc::c_char) };
    Interface::from_raw(ptr)
}

pub fn get_by_addr(addr: u16) -> Option<Interface> {
    let ptr = unsafe { ffi::csp_iflist_get_by_addr(addr) };
    Interface::from_raw(ptr)
}

pub fn get_by_index(idx: i32) -> Option<Interface> {
    let ptr = unsafe { ffi::csp_iflist_get_by_index(idx) };
    Interface::from_raw(ptr)
}

pub fn get_default() -> Option<Interface> {
    let ptr = unsafe { ffi::csp_iflist_get_by_isdfl(core::ptr::null_mut()) };
    Interface::from_raw(ptr)
}

pub fn get_first() -> Option<Interface> {
    let ptr = unsafe { ffi::csp_iflist_get() };
    Interface::from_raw(ptr)
}

pub fn is_within_subnet(addr: u16, iface: &Interface) -> bool {
    unsafe { ffi::csp_iflist_is_within_subnet(addr, iface.ptr) != 0 }
}

pub fn check_default() {
    unsafe { ffi::csp_iflist_check_dfl() }
}

pub fn print() {
    unsafe { ffi::csp_iflist_print() }
}

pub fn add(iface: &mut Interface) {
    unsafe { ffi::csp_iflist_add(iface.ptr) }
}

pub fn remove(iface: &mut Interface) {
    unsafe { ffi::csp_iflist_remove(iface.ptr) }
}

pub fn get_by_subnet(addr: u16, from: Option<&Interface>) -> Option<Interface> {
    let from_ptr = from.map(|i| i.ptr).unwrap_or(core::ptr::null_mut());
    let ptr = unsafe { ffi::csp_iflist_get_by_subnet(addr, from_ptr) };
    Interface::from_raw(ptr)
}

pub struct InterfaceIter {
    current: *mut ffi::csp_iface_t,
}

impl Iterator for InterfaceIter {
    type Item = Interface;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() {
            None
        } else {
            let iface = Interface { ptr: self.current };
            self.current = unsafe { (*self.current).next };
            Some(iface)
        }
    }
}

pub fn iter() -> InterfaceIter {
    InterfaceIter {
        current: unsafe { ffi::csp_iflist_get() },
    }
}
