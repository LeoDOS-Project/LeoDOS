use crate::ffi;

#[derive(Debug, Clone, Copy)]
pub enum DedupMode {
    Off = 0,
    ForwardOnly = 1,
    IncomingOnly = 2,
    All = 3,
}

pub struct Config {
    ptr: *const ffi::csp_conf_t,
}

impl Config {
    pub fn get() -> Self {
        Self {
            ptr: unsafe { ffi::csp_get_conf() },
        }
    }

    pub fn version(&self) -> u8 {
        unsafe { (*self.ptr).version }
    }

    pub fn hostname(&self) -> &str {
        unsafe {
            let ptr = (*self.ptr).hostname;
            if ptr.is_null() {
                ""
            } else {
                let c_str = core::ffi::CStr::from_ptr(ptr);
                c_str.to_str().unwrap_or("")
            }
        }
    }

    pub fn model(&self) -> &str {
        unsafe {
            let ptr = (*self.ptr).model;
            if ptr.is_null() {
                ""
            } else {
                let c_str = core::ffi::CStr::from_ptr(ptr);
                c_str.to_str().unwrap_or("")
            }
        }
    }

    pub fn revision(&self) -> &str {
        unsafe {
            let ptr = (*self.ptr).revision;
            if ptr.is_null() {
                ""
            } else {
                let c_str = core::ffi::CStr::from_ptr(ptr);
                c_str.to_str().unwrap_or("")
            }
        }
    }

    pub fn dedup(&self) -> DedupMode {
        match unsafe { (*self.ptr).dedup } {
            0 => DedupMode::Off,
            1 => DedupMode::ForwardOnly,
            2 => DedupMode::IncomingOnly,
            _ => DedupMode::All,
        }
    }

    pub fn default_conn_opts(&self) -> u32 {
        unsafe { (*self.ptr).conn_dfl_so }
    }
}
