use crate::error::{check, Result};
use crate::ffi;
use crate::types::{Connection, Packet};

pub type ReadFn = unsafe extern "C" fn(
    buffer: *mut u8,
    size: u32,
    offset: u32,
    data: *mut libc::c_void,
) -> libc::c_int;

pub type WriteFn = unsafe extern "C" fn(
    buffer: *const u8,
    size: u32,
    offset: u32,
    totalsz: u32,
    data: *mut libc::c_void,
) -> libc::c_int;

pub struct SfpReader {
    inner: ffi::csp_sfp_read_t,
}

impl SfpReader {
    pub fn new(read_fn: ReadFn, data: *mut libc::c_void) -> Self {
        Self {
            inner: ffi::csp_sfp_read_t {
                data,
                read: Some(read_fn),
            },
        }
    }
}

pub struct SfpWriter {
    inner: ffi::csp_sfp_recv_t,
}

impl SfpWriter {
    pub fn new(write_fn: WriteFn, data: *mut libc::c_void) -> Self {
        Self {
            inner: ffi::csp_sfp_recv_t {
                data,
                write: Some(write_fn),
            },
        }
    }
}

pub fn send(
    conn: &Connection,
    reader: &SfpReader,
    datasize: u32,
    mtu: u32,
    timeout_ms: u32,
) -> Result<()> {
    check(unsafe { ffi::csp_sfp_send(conn.as_ptr(), &reader.inner, datasize, mtu, timeout_ms) })
}

pub fn recv(
    conn: &Connection,
    writer: &SfpWriter,
    timeout_ms: u32,
    first_packet: Option<Packet>,
) -> Result<()> {
    let pkt_ptr = first_packet.map(|p| p.into_raw()).unwrap_or(core::ptr::null_mut());
    check(unsafe { ffi::csp_sfp_recv_fp(conn.as_ptr(), &writer.inner, timeout_ms, pkt_ptr) })
}

pub fn get_max_mtu_for_opts(opts: crate::types::ConnectOpts) -> u32 {
    unsafe { ffi::csp_sfp_opts_max_mtu(opts.bits()) }
}

pub fn get_max_mtu_for_conn(conn: &Connection) -> u32 {
    unsafe { ffi::csp_sfp_conn_max_mtu(conn.as_ptr()) }
}
