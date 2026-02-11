use crate::{cfs, wamr, MAX_DIRECTORIES};
use wamr::{ffi as wamr_ffi, NativeSymbol};

use super::common::{
    allocate_handle, get_handle_mut, get_host_state, release_handle, ERR_END_OF_DIR, ERR_IO_ERROR,
    ERR_NOT_FOUND, INVALID_HANDLE,
};

pub(crate) fn dir_open() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_dir_open\0".as_ptr() as *const _,
        func_ptr: host_dir_open as *mut _,
        signature: "($)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn dir_close() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_dir_close\0".as_ptr() as *const _,
        func_ptr: host_dir_close as *mut _,
        signature: "(i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn dir_read() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_dir_read\0".as_ptr() as *const _,
        func_ptr: host_dir_read as *mut _,
        signature: "(i*i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn dir_rewind() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_dir_rewind\0".as_ptr() as *const _,
        func_ptr: host_dir_rewind as *mut _,
        signature: "(i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

unsafe extern "C" fn host_dir_open(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    path: *const libc::c_char,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let path_str = match core::ffi::CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    let dir = match cfs::os::fs::Directory::open(path_str) {
        Ok(d) => d,
        Err(_) => return ERR_NOT_FOUND,
    };

    match allocate_handle(&mut host.directories, dir) {
        Ok(h) => h as i32,
        Err(e) => e,
    }
}

unsafe extern "C" fn host_dir_close(exec_env: *mut wamr_ffi::WASMExecEnv, handle: u32) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    match release_handle(&mut host.directories, handle) {
        Some(_) => 0,
        None => INVALID_HANDLE,
    }
}

unsafe extern "C" fn host_dir_read(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    buf: *mut u8,
    buf_len: i32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(dir) = get_handle_mut(&mut host.directories, handle) else {
        return INVALID_HANDLE;
    };

    match dir.next() {
        Some(Ok(entry)) => {
            let name_bytes = entry.as_bytes();
            let copy_len = name_bytes.len().min(buf_len as usize - 1);
            let dest = core::slice::from_raw_parts_mut(buf, buf_len as usize);
            dest[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
            dest[copy_len] = 0;
            copy_len as i32
        }
        Some(Err(_)) => ERR_IO_ERROR,
        None => ERR_END_OF_DIR,
    }
}

unsafe extern "C" fn host_dir_rewind(exec_env: *mut wamr_ffi::WASMExecEnv, handle: u32) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(dir) = get_handle_mut(&mut host.directories, handle) else {
        return INVALID_HANDLE;
    };

    match dir.rewind() {
        Ok(_) => 0,
        Err(_) => ERR_IO_ERROR,
    }
}
