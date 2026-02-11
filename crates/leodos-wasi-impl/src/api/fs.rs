use crate::{cfs, wamr, MAX_FILES};
use wamr::{ffi as wamr_ffi, NativeSymbol};

use super::common::{
    allocate_handle, get_handle_mut, get_host_state, release_handle, write_to_guest,
    ERR_ALREADY_EXISTS, ERR_EOF, ERR_IO_ERROR, ERR_NOT_FOUND, ERR_PERMISSION_DENIED, INVALID_HANDLE,
};

pub(crate) fn file_open() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_file_open\0".as_ptr() as *const _,
        func_ptr: host_file_open as *mut _,
        signature: "($i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn file_create() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_file_create\0".as_ptr() as *const _,
        func_ptr: host_file_create as *mut _,
        signature: "($)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn file_close() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_file_close\0".as_ptr() as *const _,
        func_ptr: host_file_close as *mut _,
        signature: "(i)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn file_read() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_file_read\0".as_ptr() as *const _,
        func_ptr: host_file_read as *mut _,
        signature: "(i*~)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn file_write() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_file_write\0".as_ptr() as *const _,
        func_ptr: host_file_write as *mut _,
        signature: "(i*~)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn file_seek() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_file_seek\0".as_ptr() as *const _,
        func_ptr: host_file_seek as *mut _,
        signature: "(iIi)I\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn file_stat() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_file_stat\0".as_ptr() as *const _,
        func_ptr: host_file_stat as *mut _,
        signature: "(i*)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn fs_stat() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_fs_stat\0".as_ptr() as *const _,
        func_ptr: host_fs_stat as *mut _,
        signature: "($*)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn fs_mkdir() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_fs_mkdir\0".as_ptr() as *const _,
        func_ptr: host_fs_mkdir as *mut _,
        signature: "($)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn fs_rmdir() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_fs_rmdir\0".as_ptr() as *const _,
        func_ptr: host_fs_rmdir as *mut _,
        signature: "($)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn fs_remove() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_fs_remove\0".as_ptr() as *const _,
        func_ptr: host_fs_remove as *mut _,
        signature: "($)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

pub(crate) fn fs_rename() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_fs_rename\0".as_ptr() as *const _,
        func_ptr: host_fs_rename as *mut _,
        signature: "($$)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

unsafe extern "C" fn host_file_open(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    path: *const libc::c_char,
    access_mode: i32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let path_str = match core::ffi::CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    let mode = match access_mode {
        0 => cfs::os::fs::AccessMode::ReadOnly,
        1 => cfs::os::fs::AccessMode::WriteOnly,
        2 => cfs::os::fs::AccessMode::ReadWrite,
        _ => return INVALID_HANDLE,
    };

    let file = match cfs::os::fs::File::open(path_str, mode) {
        Ok(f) => f,
        Err(_) => return ERR_NOT_FOUND,
    };

    match allocate_handle(&mut host.files, file) {
        Ok(h) => h as i32,
        Err(e) => e,
    }
}

unsafe extern "C" fn host_file_create(
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

    let file = match cfs::os::fs::File::create(path_str) {
        Ok(f) => f,
        Err(_) => return ERR_IO_ERROR,
    };

    match allocate_handle(&mut host.files, file) {
        Ok(h) => h as i32,
        Err(e) => e,
    }
}

unsafe extern "C" fn host_file_close(exec_env: *mut wamr_ffi::WASMExecEnv, handle: u32) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    match release_handle(&mut host.files, handle) {
        Some(_) => 0,
        None => INVALID_HANDLE,
    }
}

unsafe extern "C" fn host_file_read(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    buf: *mut u8,
    buf_len: u32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(file) = get_handle_mut(&mut host.files, handle) else {
        return INVALID_HANDLE;
    };

    let read_buf = core::slice::from_raw_parts_mut(buf, buf_len as usize);

    match file.sync_read(read_buf) {
        Ok(0) => ERR_EOF,
        Ok(n) => n as i32,
        Err(_) => ERR_IO_ERROR,
    }
}

unsafe extern "C" fn host_file_write(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    data: *const u8,
    data_len: u32,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(file) = get_handle_mut(&mut host.files, handle) else {
        return INVALID_HANDLE;
    };

    let write_buf = core::slice::from_raw_parts(data, data_len as usize);

    match file.sync_write(write_buf) {
        Ok(n) => n as i32,
        Err(_) => ERR_IO_ERROR,
    }
}

unsafe extern "C" fn host_file_seek(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    offset: i64,
    whence: i32,
) -> i64 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE as i64;
    };

    let Some(file) = get_handle_mut(&mut host.files, handle) else {
        return INVALID_HANDLE as i64;
    };

    let seek_from = match whence {
        0 => cfs::os::fs::SeekFrom::Start(offset as u32),
        1 => cfs::os::fs::SeekFrom::Current(offset as i32),
        2 => cfs::os::fs::SeekFrom::End(offset as i32),
        _ => return INVALID_HANDLE as i64,
    };

    match file.seek(seek_from) {
        Ok(pos) => pos as i64,
        Err(_) => ERR_IO_ERROR as i64,
    }
}

#[repr(C)]
pub struct GuestFileStat {
    pub size: u64,
    pub is_dir: u32,
}

unsafe extern "C" fn host_file_stat(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    handle: u32,
    stat_out: *mut GuestFileStat,
) -> i32 {
    let Some(host) = get_host_state(exec_env) else {
        return INVALID_HANDLE;
    };

    let Some(file) = get_handle_mut(&mut host.files, handle) else {
        return INVALID_HANDLE;
    };

    let info = match file.info() {
        Ok(i) => i,
        Err(_) => return ERR_IO_ERROR,
    };

    (*stat_out).size = 0;
    (*stat_out).is_dir = 0;

    0
}

unsafe extern "C" fn host_fs_stat(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    path: *const libc::c_char,
    stat_out: *mut GuestFileStat,
) -> i32 {
    let path_str = match core::ffi::CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    let stat = match cfs::os::fs::stat(path_str) {
        Ok(s) => s,
        Err(_) => return ERR_NOT_FOUND,
    };

    (*stat_out).size = stat.size() as u64;
    (*stat_out).is_dir = if stat.is_dir() { 1 } else { 0 };

    0
}

unsafe extern "C" fn host_fs_mkdir(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    path: *const libc::c_char,
) -> i32 {
    let path_str = match core::ffi::CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    match cfs::os::fs::mkdir(path_str) {
        Ok(_) => 0,
        Err(_) => ERR_IO_ERROR,
    }
}

unsafe extern "C" fn host_fs_rmdir(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    path: *const libc::c_char,
) -> i32 {
    let path_str = match core::ffi::CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    match cfs::os::fs::rmdir(path_str) {
        Ok(_) => 0,
        Err(_) => ERR_IO_ERROR,
    }
}

unsafe extern "C" fn host_fs_remove(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    path: *const libc::c_char,
) -> i32 {
    let path_str = match core::ffi::CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    match cfs::os::fs::remove(path_str) {
        Ok(_) => 0,
        Err(_) => ERR_NOT_FOUND,
    }
}

unsafe extern "C" fn host_fs_rename(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    old_path: *const libc::c_char,
    new_path: *const libc::c_char,
) -> i32 {
    let old_str = match core::ffi::CStr::from_ptr(old_path).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    let new_str = match core::ffi::CStr::from_ptr(new_path).to_str() {
        Ok(s) => s,
        Err(_) => return INVALID_HANDLE,
    };

    match cfs::os::fs::rename(old_str, new_str) {
        Ok(_) => 0,
        Err(_) => ERR_IO_ERROR,
    }
}
