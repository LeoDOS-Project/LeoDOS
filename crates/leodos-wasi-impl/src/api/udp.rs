use crate::{cfs, wamr, MAX_UDP_SOCKETS};
use wamr::{ffi as wamr_ffi, NativeSymbol};

use super::common::{
    allocate_handle, get_guest_slice, get_guest_slice_mut, get_handle, get_host_state,
    read_cstring, release_handle, write_to_guest, ERR_IO_ERROR, ERR_NO_CAPACITY, INVALID_HANDLE,
};

pub(crate) fn socket_open() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_udp_open\0".as_ptr() as *const _,
        func_ptr: host_udp_open as *mut _,
        signature: "($i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn socket_close() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_udp_close\0".as_ptr() as *const _,
        func_ptr: host_udp_close as *mut _,
        signature: "(i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn socket_sendto() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_udp_send_to\0".as_ptr() as *const _,
        func_ptr: host_udp_send_to as *mut _,
        signature: "(i*~$i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn socket_recvfrom() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_udp_recv_from\0".as_ptr() as *const _,
        func_ptr: host_udp_recv_from as *mut _,
        signature: "(i*~*i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

unsafe extern "C" fn host_udp_open(
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

    let socket = match cfs::os::net::UdpSocket::bind(sock_addr) {
        Ok(s) => s,
        Err(_) => return ERR_IO_ERROR,
    };

    match allocate_handle(&mut host.udp_sockets, socket) {
        Ok(h) => h as i32,
        Err(e) => e,
    }
}

unsafe extern "C" fn host_udp_close(exec_env: *mut wamr_ffi::WASMExecEnv, handle: u32) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    match release_handle(&mut host.udp_sockets, handle) {
        Some(_) => 0,
        None => INVALID_HANDLE,
    }
}

unsafe extern "C" fn host_udp_send_to(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    data: *const u8,
    data_len: u32,
    target_addr: *const libc::c_char,
    target_port: u16,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(socket) = get_handle(&host.udp_sockets, handle) else {
        return INVALID_HANDLE;
    };

    let addr_str = match core::ffi::CStr::from_ptr(target_addr).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    let target = match cfs::os::net::SocketAddr::new_ipv4(addr_str, target_port) {
        Ok(a) => a,
        Err(_) => return ERR_IO_ERROR,
    };

    let buf = core::slice::from_raw_parts(data, data_len as usize);

    match socket.send_to(buf, &target) {
        Ok(n) => n as i32,
        Err(_) => ERR_IO_ERROR,
    }
}

unsafe extern "C" fn host_udp_recv_from(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    buf: *mut u8,
    buf_len: u32,
    addr_out: *mut u8,
    timeout_ms: i32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(socket) = get_handle(&host.udp_sockets, handle) else {
        return INVALID_HANDLE;
    };

    let recv_buf = core::slice::from_raw_parts_mut(buf, buf_len as usize);

    let timeout = if timeout_ms < 0 {
        cfs::os::net::Timeout::Pend
    } else if timeout_ms == 0 {
        cfs::os::net::Timeout::Poll
    } else {
        cfs::os::net::Timeout::Milliseconds(timeout_ms)
    };

    match socket.recv_from(recv_buf, timeout) {
        Ok((n, remote_addr)) => {
            if !addr_out.is_null() {
                if let Ok(addr_str) = remote_addr.to_string() {
                    let port = remote_addr.port().unwrap_or(0);
                    let addr_buf = core::slice::from_raw_parts_mut(addr_out, 64);
                    let bytes = addr_str.as_bytes();
                    let copy_len = bytes.len().min(60);
                    addr_buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    addr_buf[copy_len] = b':';
                    let port_str_buf = &mut addr_buf[copy_len + 1..];
                    let mut port_len = 0;
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
            n as i32
        }
        Err(_) => ERR_IO_ERROR,
    }
}
