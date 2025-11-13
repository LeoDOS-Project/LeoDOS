use crate::error::{Error, Result};
use crate::ffi;
use crate::types::{ConnectOpts, Connection, Packet, Priority};

pub fn init() {
    unsafe {
        ffi::csp_init();
    }
}

pub fn buffer_get() -> Result<Packet> {
    let ptr = unsafe { ffi::csp_buffer_get(0) };
    Packet::from_raw(ptr).ok_or(Error::NoBuffers)
}

pub fn buffer_remaining() -> i32 {
    unsafe { ffi::csp_buffer_remaining() }
}

pub fn connect(
    prio: Priority,
    dst: u16,
    dst_port: u8,
    timeout_ms: u32,
    opts: ConnectOpts,
) -> Result<Connection> {
    let ptr = unsafe { ffi::csp_connect(prio as u8, dst, dst_port, timeout_ms, opts.bits()) };
    Connection::from_raw(ptr).ok_or(Error::Timeout)
}

pub fn ping(node: u16, timeout_ms: u32, size: u32, opts: ConnectOpts) -> Result<i32> {
    let result = unsafe { ffi::csp_ping(node, timeout_ms, size, opts.bits() as u8) };
    if result < 0 {
        Err(Error::Timeout)
    } else {
        Ok(result)
    }
}

pub fn ping_noreply(node: u16) {
    unsafe {
        ffi::csp_ping_noreply(node);
    }
}

pub fn reboot(node: u16) {
    unsafe {
        ffi::csp_reboot(node);
    }
}

pub fn shutdown(node: u16) {
    unsafe {
        ffi::csp_shutdown(node);
    }
}

pub fn service_handler(packet: Packet) {
    unsafe {
        ffi::csp_service_handler(packet.into_raw());
    }
}

pub fn route_work() -> Result<()> {
    let result = unsafe { ffi::csp_route_work() };
    if result < 0 {
        Err(Error::from_csp(result).unwrap_or(Error::Unknown(result)))
    } else {
        Ok(())
    }
}

pub fn sendto(
    prio: Priority,
    dst: u16,
    dst_port: u8,
    src_port: u8,
    opts: ConnectOpts,
    packet: Packet,
) {
    unsafe {
        ffi::csp_sendto(
            prio as u8,
            dst,
            dst_port,
            src_port,
            opts.bits(),
            packet.into_raw(),
        );
    }
}

pub fn sendto_reply(request: &Packet, reply: Packet, opts: ConnectOpts) {
    unsafe {
        ffi::csp_sendto_reply(request.as_ptr(), reply.into_raw(), opts.bits());
    }
}

pub fn transaction(
    prio: Priority,
    dst: u16,
    dst_port: u8,
    timeout_ms: u32,
    outbuf: &[u8],
    inbuf: &mut [u8],
    opts: ConnectOpts,
) -> Result<usize> {
    let result = unsafe {
        ffi::csp_transaction_w_opts(
            prio as u8,
            dst,
            dst_port,
            timeout_ms,
            outbuf.as_ptr() as *const libc::c_void,
            outbuf.len() as i32,
            inbuf.as_mut_ptr() as *mut libc::c_void,
            inbuf.len() as i32,
            opts.bits(),
        )
    };
    if result <= 0 {
        Err(Error::Timeout)
    } else {
        Ok(result as usize)
    }
}

pub fn transaction_no_reply(
    prio: Priority,
    dst: u16,
    dst_port: u8,
    timeout_ms: u32,
    outbuf: &[u8],
    opts: ConnectOpts,
) -> Result<()> {
    let result = unsafe {
        ffi::csp_transaction_w_opts(
            prio as u8,
            dst,
            dst_port,
            timeout_ms,
            outbuf.as_ptr() as *const libc::c_void,
            outbuf.len() as i32,
            core::ptr::null_mut(),
            0,
            opts.bits(),
        )
    };
    if result <= 0 {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}

pub fn transaction_persistent(
    conn: &Connection,
    timeout_ms: u32,
    outbuf: &[u8],
    inbuf: &mut [u8],
) -> Result<usize> {
    let result = unsafe {
        ffi::csp_transaction_persistent(
            conn.as_ptr(),
            timeout_ms,
            outbuf.as_ptr() as *const libc::c_void,
            outbuf.len() as i32,
            inbuf.as_mut_ptr() as *mut libc::c_void,
            inbuf.len() as i32,
        )
    };
    if result <= 0 {
        Err(Error::Timeout)
    } else {
        Ok(result as usize)
    }
}

pub fn get_uptime(node: u16, timeout_ms: u32) -> Result<u32> {
    let mut uptime: u32 = 0;
    let result = unsafe { ffi::csp_get_uptime(node, timeout_ms, &mut uptime) };
    if result < 0 {
        Err(Error::from_csp(result).unwrap_or(Error::Timeout))
    } else {
        Ok(uptime)
    }
}

pub fn get_memfree(node: u16, timeout_ms: u32) -> Result<u32> {
    let mut size: u32 = 0;
    let result = unsafe { ffi::csp_get_memfree(node, timeout_ms, &mut size) };
    if result < 0 {
        Err(Error::from_csp(result).unwrap_or(Error::Timeout))
    } else {
        Ok(size)
    }
}

pub fn get_buf_free(node: u16, timeout_ms: u32) -> Result<u32> {
    let mut size: u32 = 0;
    let result = unsafe { ffi::csp_get_buf_free(node, timeout_ms, &mut size) };
    if result < 0 {
        Err(Error::from_csp(result).unwrap_or(Error::Timeout))
    } else {
        Ok(size)
    }
}

pub fn print_connections() {
    unsafe { ffi::csp_conn_print_table() }
}

pub fn hex_dump(desc: &str, data: &[u8]) {
    let mut buf = [0u8; 64];
    let len = desc.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&desc.as_bytes()[..len]);
    unsafe {
        ffi::csp_hex_dump(
            buf.as_ptr() as *const libc::c_char,
            data.as_ptr() as *const libc::c_void,
            data.len() as i32,
        )
    }
}

pub fn print_uptime(node: u16, timeout_ms: u32) {
    unsafe { ffi::csp_uptime(node, timeout_ms) }
}

pub fn print_memfree(node: u16, timeout_ms: u32) {
    unsafe { ffi::csp_memfree(node, timeout_ms) }
}

pub fn print_buf_free(node: u16, timeout_ms: u32) {
    unsafe { ffi::csp_buf_free(node, timeout_ms) }
}

pub fn print_ps(node: u16, timeout_ms: u32) {
    unsafe { ffi::csp_ps(node, timeout_ms) }
}

pub fn bytesize(bytes: u64) -> (u64, &'static str) {
    let mut postfix = [0u8; 8];
    let value = unsafe { ffi::csp_bytesize(bytes, postfix.as_mut_ptr() as *mut libc::c_char) };
    let suffix = match postfix[0] {
        b'B' => "B",
        b'K' => "K",
        b'M' => "M",
        b'G' => "G",
        _ => "",
    };
    (value, suffix)
}

pub fn bind_callback(
    callback: unsafe extern "C" fn(packet: *mut libc::c_void),
    port: u8,
) -> Result<()> {
    let cb: ffi::csp_callback_t = Some(unsafe {
        core::mem::transmute::<
            unsafe extern "C" fn(*mut libc::c_void),
            unsafe extern "C" fn(*mut ffi::csp_packet_t),
        >(callback)
    });
    crate::error::check(unsafe { ffi::csp_bind_callback(cb, port) })
}

