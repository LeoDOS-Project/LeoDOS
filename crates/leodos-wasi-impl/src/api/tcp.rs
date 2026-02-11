use crate::{cfs, wamr, MAX_TCP_LISTENERS, MAX_TCP_STREAMS};
use wamr::{ffi as wamr_ffi, NativeSymbol};

use super::common::{
    allocate_handle, get_handle_mut, get_host_state, release_handle, ERR_IO_ERROR, INVALID_HANDLE,
};

pub(crate) fn tcp_listen() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_tcp_listen\0".as_ptr() as *const _,
        func_ptr: host_tcp_listen as *mut _,
        signature: "($i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn tcp_accept() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_tcp_accept\0".as_ptr() as *const _,
        func_ptr: host_tcp_accept as *mut _,
        signature: "(i*i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn tcp_listener_close() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_tcp_listener_close\0".as_ptr() as *const _,
        func_ptr: host_tcp_listener_close as *mut _,
        signature: "(i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn tcp_connect() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_tcp_connect\0".as_ptr() as *const _,
        func_ptr: host_tcp_connect as *mut _,
        signature: "($i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn tcp_read() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_tcp_read\0".as_ptr() as *const _,
        func_ptr: host_tcp_read as *mut _,
        signature: "(i*~)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn tcp_write() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_tcp_write\0".as_ptr() as *const _,
        func_ptr: host_tcp_write as *mut _,
        signature: "(i*~)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn tcp_close() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_tcp_close\0".as_ptr() as *const _,
        func_ptr: host_tcp_close as *mut _,
        signature: "(i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

unsafe extern "C" fn host_tcp_listen(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    bind_addr: *const libc::c_char,
    port: u16,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let addr_str = match core::ffi::CStr::from_ptr(bind_addr).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    let sock_addr = match cfs::os::net::SocketAddr::new_ipv4(addr_str, port) {
        Ok(a) => a,
        Err(_) => return ERR_IO_ERROR,
    };

    let listener = match cfs::os::net::TcpListener::bind(sock_addr) {
        Ok(l) => l,
        Err(_) => return ERR_IO_ERROR,
    };

    if listener.listen().is_err() {
        return ERR_IO_ERROR;
    }

    match allocate_handle(&mut host.tcp_listeners, listener) {
        Ok(h) => h as i32,
        Err(e) => e,
    }
}

unsafe extern "C" fn host_tcp_accept(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    listener_handle: u32,
    addr_out: *mut u8,
    timeout_ms: i32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let listener = match host.tcp_listeners.get(listener_handle as usize) {
        Some(Some(l)) => l,
        _ => return INVALID_HANDLE,
    };

    let timeout = if timeout_ms < 0 {
        cfs::os::net::Timeout::Pend
    } else if timeout_ms == 0 {
        cfs::os::net::Timeout::Poll
    } else {
        cfs::os::net::Timeout::Milliseconds(timeout_ms)
    };

    let (stream, remote_addr) = match listener.accept(timeout) {
        Ok(r) => r,
        Err(_) => return ERR_IO_ERROR,
    };

    if !addr_out.is_null() {
        if let Ok(addr_str) = remote_addr.to_string() {
            let port = remote_addr.port().unwrap_or(0);
            let addr_buf = core::slice::from_raw_parts_mut(addr_out, 64);
            let bytes = addr_str.as_bytes();
            let copy_len = bytes.len().min(60);
            addr_buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
            addr_buf[copy_len] = b':';
            let port_str_buf = &mut addr_buf[copy_len + 1..];
            let mut p = port;
            let mut digits = [0u8; 5];
            let mut digit_count = 0;
            if p == 0 {
                digits[0] = b'0';
                digit_count = 1;
            } else {
                while p > 0 {
                    digits[digit_count] = b'0' + (p % 10) as u8;
                    p /= 10;
                    digit_count += 1;
                }
            }
            for i in 0..digit_count {
                port_str_buf[i] = digits[digit_count - 1 - i];
            }
            port_str_buf[digit_count] = 0;
        }
    }

    match allocate_handle(&mut host.tcp_streams, stream) {
        Ok(h) => h as i32,
        Err(e) => e,
    }
}

unsafe extern "C" fn host_tcp_listener_close(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    match release_handle(&mut host.tcp_listeners, handle) {
        Some(_) => 0,
        None => INVALID_HANDLE,
    }
}

unsafe extern "C" fn host_tcp_connect(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    addr: *const libc::c_char,
    port: u16,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let addr_str = match core::ffi::CStr::from_ptr(addr).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    let sock_addr = match cfs::os::net::SocketAddr::new_ipv4(addr_str, port) {
        Ok(a) => a,
        Err(_) => return ERR_IO_ERROR,
    };

    let stream = match cfs::os::net::TcpStream::connect(sock_addr, cfs::os::net::SocketDomain::IPv4)
    {
        Ok(s) => s,
        Err(_) => return ERR_IO_ERROR,
    };

    match allocate_handle(&mut host.tcp_streams, stream) {
        Ok(h) => h as i32,
        Err(e) => e,
    }
}

unsafe extern "C" fn host_tcp_read(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    buf: *mut u8,
    buf_len: u32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(stream) = get_handle_mut(&mut host.tcp_streams, handle) else {
        return INVALID_HANDLE;
    };

    let read_buf = core::slice::from_raw_parts_mut(buf, buf_len as usize);

    match stream.read(read_buf) {
        Ok(n) => n as i32,
        Err(_) => ERR_IO_ERROR,
    }
}

unsafe extern "C" fn host_tcp_write(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    data: *const u8,
    data_len: u32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(stream) = get_handle_mut(&mut host.tcp_streams, handle) else {
        return INVALID_HANDLE;
    };

    let write_buf = core::slice::from_raw_parts(data, data_len as usize);

    match stream.write(write_buf) {
        Ok(n) => n as i32,
        Err(_) => ERR_IO_ERROR,
    }
}

unsafe extern "C" fn host_tcp_close(exec_env: *mut wamr_ffi::WASMExecEnv, handle: u32) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    match release_handle(&mut host.tcp_streams, handle) {
        Some(_) => 0,
        None => INVALID_HANDLE,
    }
}
