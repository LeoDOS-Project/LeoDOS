use core::ffi::CStr;
use leodos_libwamr as wamr;
use wamr::ffi as wamr_ffi;

use crate::WamrHost;

pub const INVALID_HANDLE: i32 = -1;
pub const ERR_NO_CAPACITY: i32 = -3;
pub const ERR_IO_ERROR: i32 = -4;
pub const ERR_NOT_FOUND: i32 = -6;
pub const ERR_PERMISSION_DENIED: i32 = -7;
pub const ERR_ALREADY_EXISTS: i32 = -8;
pub const ERR_EOF: i32 = -12;
pub const ERR_END_OF_DIR: i32 = -13;

pub unsafe fn get_host_state<'a>(exec_env: *mut wamr_ffi::WASMExecEnv) -> Option<&'a mut WamrHost<'a>> {
    let instance_ptr = wamr_ffi::wasm_runtime_get_module_inst(exec_env);
    if instance_ptr.is_null() {
        return None;
    }

    let instance = wamr::Instance::from_raw(instance_ptr);
    let host_ptr = instance.get_custom_data() as *mut WamrHost;
    core::mem::forget(instance);

    if host_ptr.is_null() {
        None
    } else {
        Some(&mut *host_ptr)
    }
}

pub fn allocate_handle<T, const N: usize>(
    pool: &mut heapless::Vec<Option<T>, N>,
    resource: T,
) -> Result<u32, i32> {
    for (i, slot) in pool.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(resource);
            return Ok(i as u32);
        }
    }

    if pool.len() < N {
        let handle = pool.len() as u32;
        pool.push(Some(resource)).map_err(|_| ERR_NO_CAPACITY)?;
        return Ok(handle);
    }

    Err(ERR_NO_CAPACITY)
}

pub fn release_handle<T, const N: usize>(
    pool: &mut heapless::Vec<Option<T>, N>,
    handle: u32,
) -> Option<T> {
    pool.get_mut(handle as usize).and_then(|slot| slot.take())
}

pub fn get_handle<T, const N: usize>(
    pool: &heapless::Vec<Option<T>, N>,
    handle: u32,
) -> Option<&T> {
    pool.get(handle as usize).and_then(|slot| slot.as_ref())
}

pub fn get_handle_mut<T, const N: usize>(
    pool: &mut heapless::Vec<Option<T>, N>,
    handle: u32,
) -> Option<&mut T> {
    pool.get_mut(handle as usize).and_then(|slot| slot.as_mut())
}

pub unsafe fn read_cstring<'a>(
    instance: &wamr::Instance,
    offset: u32,
) -> Result<&'a CStr, i32> {
    let memory = instance.default_memory().map_err(|_| INVALID_HANDLE)?;
    let native_ptr = memory.offset_to_native(offset as u64).map_err(|_| INVALID_HANDLE)?;
    Ok(CStr::from_ptr(native_ptr as *const _))
}

pub unsafe fn write_to_guest(
    instance: &wamr::Instance,
    offset: u32,
    data: &[u8],
) -> Result<(), i32> {
    let memory = instance.default_memory().map_err(|_| INVALID_HANDLE)?;
    let native_ptr = memory.offset_to_native(offset as u64).map_err(|_| INVALID_HANDLE)?;
    let dest = core::slice::from_raw_parts_mut(native_ptr as *mut u8, data.len());
    dest.copy_from_slice(data);
    Ok(())
}

pub unsafe fn get_guest_slice<'a>(
    instance: &wamr::Instance,
    offset: u32,
    len: u32,
) -> Result<&'a [u8], i32> {
    let memory = instance.default_memory().map_err(|_| INVALID_HANDLE)?;
    let native_ptr = memory.offset_to_native(offset as u64).map_err(|_| INVALID_HANDLE)?;
    Ok(core::slice::from_raw_parts(native_ptr as *const u8, len as usize))
}

pub unsafe fn get_guest_slice_mut<'a>(
    instance: &wamr::Instance,
    offset: u32,
    len: u32,
) -> Result<&'a mut [u8], i32> {
    let memory = instance.default_memory().map_err(|_| INVALID_HANDLE)?;
    let native_ptr = memory.offset_to_native(offset as u64).map_err(|_| INVALID_HANDLE)?;
    Ok(core::slice::from_raw_parts_mut(native_ptr as *mut u8, len as usize))
}
