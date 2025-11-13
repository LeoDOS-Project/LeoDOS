use crate::{cfs, wamr, WamrHost, MAX_WASM_SOCKETS};
use core::ffi::{c_void, CStr};
use wamr::{ffi as wamr_ffi, NativeSymbol};

// Helper to get mutable host state from the execution environment
unsafe fn get_host_state<'a>(exec_env: *mut wamr_ffi::WASMExecEnv) -> Option<&'a mut WamrHost<'a>> {
    let instance_ptr = wamr_ffi::wasm_runtime_get_module_inst(exec_env);
    if instance_ptr.is_null() {
        return None;
    }

    let instance = wamr::Instance::from_raw(instance_ptr);
    let host_ptr = instance.get_custom_data() as *mut WamrHost;
    core::mem::forget(instance); // Avoid dropping the temporary wrapper

    if host_ptr.is_null() {
        None
    } else {
        Some(&mut *host_ptr)
    }
}

/// Native symbol definition for opening a socket.
pub(crate) fn socket_open() -> NativeSymbol {
    NativeSymbol {
        symbol: "host_socket_open\0".as_ptr() as *const _,
        func_ptr: host_socket_open as *mut _,
        signature: "(ii)i\0".as_ptr() as *const _,
        attachment: core::ptr::null_mut(),
    }
}

/// Bridge function to open a socket.
unsafe extern "C" fn host_socket_open(
    exec_env: *mut wamr_ffi::WASMExecEnv,
    bind_addr_offset: u32,
    port: u32,
) -> u32 {
    let Some(host) = get_host_state(exec_env) else {
        return u32::MAX;
    };
    let instance = wamr::Instance::from_raw(wamr_ffi::wasm_runtime_get_module_inst(exec_env));

    let result = (|| {
        // Read address string from Wasm memory
        let memory = instance.default_memory()?;
        let native_ptr = memory.offset_to_native(bind_addr_offset as u64)?;
        let cstr = CStr::from_ptr(native_ptr as *const _);
        let addr_str = cstr
            .to_str()
            .map_err(|_| wamr::WamrError::InvalidUtf8(core::str::Utf8Error::default()))?;

        // Create cFS socket
        let sock_addr = cfs::os::net::SocketAddr::new_ipv4(addr_str, port as u16)?;
        let socket = cfs::os::net::UdpSocket::bind(sock_addr)?;

        // Find a slot in our pool
        for (i, slot) in host.sockets.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(socket);
                return Ok(i as u32);
            }
        }

        // Or add a new one if there is capacity
        if host.sockets.len() < MAX_WASM_SOCKETS {
            let handle = host.sockets.len() as u32;
            host.sockets.push(Some(socket)).unwrap(); // Should not fail due to capacity check
            return Ok(handle);
        }

        Err(wamr::WamrError::CapacityExceeded)
    })();

    core::mem::forget(instance);
    result.unwrap_or(u32::MAX)
}
