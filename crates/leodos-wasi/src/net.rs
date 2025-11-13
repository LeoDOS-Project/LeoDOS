// src/net.rs
use heapless::CString;

mod ffi {
    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn host_socket_open(bind_addr: *const core::ffi::c_char, port: u16) -> u32;
        pub fn host_socket_close(handle: u32);
    }
}

/// A handle to a UDP socket managed by the cFS host.
pub struct UdpSocket {
    handle: u32,
}

impl UdpSocket {
    pub fn bind(addr: &str) -> Result<Self, &'static str> {
        let mut c_addr = CString::<64>::new();
        c_addr
            .extend_from_bytes(addr.as_bytes())
            .map_err(|_| "Address too long")?;
        let handle = unsafe { ffi::host_socket_open(c_addr.as_ptr(), 0) };
        if handle == u32::MAX {
            return Err("Failed to open socket");
        }
        Ok(UdpSocket { handle })
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        if self.handle != u32::MAX {
            unsafe { ffi::host_socket_close(self.handle) };
        }
    }
}
