use crate::error::{check, Result};
use crate::ffi;

#[derive(Debug, Clone, Copy, Default)]
pub struct Timestamp {
    pub sec: u32,
    pub nsec: u32,
}

impl Timestamp {
    pub fn now() -> Self {
        let mut ts = ffi::csp_timestamp_t {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe { ffi::csp_clock_get_time(&mut ts) };
        Self {
            sec: ts.tv_sec,
            nsec: ts.tv_nsec,
        }
    }

    pub fn set(self) -> Result<()> {
        let ts = ffi::csp_timestamp_t {
            tv_sec: self.sec,
            tv_nsec: self.nsec,
        };
        check(unsafe { ffi::csp_clock_set_time(&ts) })
    }

    pub(crate) fn from_raw(ts: ffi::csp_timestamp_t) -> Self {
        Self {
            sec: ts.tv_sec,
            nsec: ts.tv_nsec,
        }
    }

    pub(crate) fn as_raw(&self) -> ffi::csp_timestamp_t {
        ffi::csp_timestamp_t {
            tv_sec: self.sec,
            tv_nsec: self.nsec,
        }
    }
}

pub fn get_ms() -> u32 {
    unsafe { ffi::csp_get_ms() }
}

pub fn get_s() -> u32 {
    unsafe { ffi::csp_get_s() }
}
