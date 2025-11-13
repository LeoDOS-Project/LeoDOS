use crate::error::{check, Result};
use crate::ffi;

pub const REQUEST: u8 = ffi::CSP_CMP_REQUEST as u8;
pub const REPLY: u8 = ffi::CSP_CMP_REPLY as u8;

pub const IDENT: u8 = ffi::CSP_CMP_IDENT as u8;
pub const ROUTE_SET_V1: u8 = ffi::CSP_CMP_ROUTE_SET_V1 as u8;
pub const ROUTE_SET_V2: u8 = ffi::CSP_CMP_ROUTE_SET_V2 as u8;
pub const IF_STATS: u8 = ffi::CSP_CMP_IF_STATS as u8;
pub const PEEK: u8 = ffi::CSP_CMP_PEEK as u8;
pub const POKE: u8 = ffi::CSP_CMP_POKE as u8;
pub const CLOCK: u8 = ffi::CSP_CMP_CLOCK as u8;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Ident {
    pub hostname: [u8; 20],
    pub model: [u8; 30],
    pub revision: [u8; 20],
    pub date: [u8; 12],
    pub time: [u8; 9],
}

impl Default for Ident {
    fn default() -> Self {
        Self {
            hostname: [0; 20],
            model: [0; 30],
            revision: [0; 20],
            date: [0; 12],
            time: [0; 9],
        }
    }
}

pub fn ident(node: u16, timeout_ms: u32) -> Result<Ident> {
    let mut msg: ffi::csp_cmp_message = unsafe { core::mem::zeroed() };
    msg.type_ = REQUEST;
    msg.code = IDENT;
    let size = core::mem::size_of::<ffi::csp_cmp_message__bindgen_ty_1__bindgen_ty_1>() as i32;
    check(unsafe { ffi::csp_cmp(node, timeout_ms, IDENT, size, &mut msg) })?;
    let ident_data = unsafe { &msg.__bindgen_anon_1.ident };
    Ok(Ident {
        hostname: unsafe { core::mem::transmute(ident_data.hostname) },
        model: unsafe { core::mem::transmute(ident_data.model) },
        revision: unsafe { core::mem::transmute(ident_data.revision) },
        date: unsafe { core::mem::transmute(ident_data.date) },
        time: unsafe { core::mem::transmute(ident_data.time) },
    })
}

pub fn clock_get(node: u16, timeout_ms: u32) -> Result<crate::time::Timestamp> {
    let mut msg: ffi::csp_cmp_message = unsafe { core::mem::zeroed() };
    msg.type_ = REQUEST;
    msg.code = CLOCK;
    let size = core::mem::size_of::<ffi::csp_timestamp_t>() as i32;
    check(unsafe { ffi::csp_cmp(node, timeout_ms, CLOCK, size, &mut msg) })?;
    let ts = unsafe { msg.__bindgen_anon_1.clock };
    Ok(crate::time::Timestamp::from_raw(ts))
}

pub fn clock_set(node: u16, timeout_ms: u32, time: &crate::time::Timestamp) -> Result<()> {
    let mut msg: ffi::csp_cmp_message = unsafe { core::mem::zeroed() };
    msg.type_ = REQUEST;
    msg.code = CLOCK;
    msg.__bindgen_anon_1.clock = time.as_raw();
    let size = core::mem::size_of::<ffi::csp_timestamp_t>() as i32;
    check(unsafe { ffi::csp_cmp(node, timeout_ms, CLOCK, size, &mut msg) })
}

pub fn peek(node: u16, timeout_ms: u32, addr: u32, len: u8, buf: &mut [u8]) -> Result<()> {
    let mut msg: ffi::csp_cmp_message = unsafe { core::mem::zeroed() };
    msg.type_ = REQUEST;
    msg.code = PEEK;
    msg.__bindgen_anon_1.peek.addr = addr;
    msg.__bindgen_anon_1.peek.len = len;
    let size = core::mem::size_of::<ffi::csp_cmp_message__bindgen_ty_1__bindgen_ty_5>() as i32;
    check(unsafe { ffi::csp_cmp(node, timeout_ms, PEEK, size, &mut msg) })?;
    let data = unsafe { &msg.__bindgen_anon_1.peek.data };
    let copy_len = (len as usize).min(buf.len()).min(data.len());
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, buf.as_mut_ptr(), copy_len);
    }
    Ok(())
}

pub fn poke(node: u16, timeout_ms: u32, addr: u32, data: &[u8]) -> Result<()> {
    let mut msg: ffi::csp_cmp_message = unsafe { core::mem::zeroed() };
    msg.type_ = REQUEST;
    msg.code = POKE;
    let len = data.len().min(ffi::CSP_CMP_POKE_MAX_LEN as usize);
    unsafe {
        msg.__bindgen_anon_1.poke.addr = addr;
        msg.__bindgen_anon_1.poke.len = len as u8;
        core::ptr::copy_nonoverlapping(
            data.as_ptr(),
            msg.__bindgen_anon_1.poke.data.as_mut_ptr() as *mut u8,
            len,
        );
    }
    let size = core::mem::size_of::<ffi::csp_cmp_message__bindgen_ty_1__bindgen_ty_6>() as i32;
    check(unsafe { ffi::csp_cmp(node, timeout_ms, POKE, size, &mut msg) })
}
