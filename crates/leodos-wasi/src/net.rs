use crate::error::{check_with_value, Result, WasiError};
use heapless::String;

mod ffi {
    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn host_udp_open(bind_addr: *const core::ffi::c_char, port: u16) -> i32;
        pub fn host_udp_close(handle: u32) -> i32;
        pub fn host_udp_send_to(
            handle: u32,
            data: *const u8,
            data_len: u32,
            target_addr: *const core::ffi::c_char,
            target_port: u16,
        ) -> i32;
        pub fn host_udp_recv_from(
            handle: u32,
            buf: *mut u8,
            buf_len: u32,
            addr_out: *mut u8,
            timeout_ms: i32,
        ) -> i32;
    }
}

pub struct UdpSocket {
    handle: u32,
}

impl UdpSocket {
    pub fn bind(addr: &str, port: u16) -> Result<Self> {
        let mut c_addr = String::<64>::new();
        c_addr.push_str(addr).map_err(|_| WasiError::InvalidArgument)?;
        c_addr.push('\0').map_err(|_| WasiError::InvalidArgument)?;
        let result = unsafe { ffi::host_udp_open(c_addr.as_ptr() as *const _, port) };
        if result < 0 {
            return Err(WasiError::from_code(result));
        }
        Ok(UdpSocket { handle: result as u32 })
    }

    pub fn send_to(&self, data: &[u8], addr: &str, port: u16) -> Result<usize> {
        let mut c_addr = String::<64>::new();
        c_addr.push_str(addr).map_err(|_| WasiError::InvalidArgument)?;
        c_addr.push('\0').map_err(|_| WasiError::InvalidArgument)?;
        let result = unsafe {
            ffi::host_udp_send_to(
                self.handle,
                data.as_ptr(),
                data.len() as u32,
                c_addr.as_ptr() as *const _,
                port,
            )
        };
        check_with_value(result).map(|n| n as usize)
    }

    pub fn recv_from(&self, buf: &mut [u8], timeout_ms: i32) -> Result<(usize, String<64>)> {
        let mut addr_buf = [0u8; 64];
        let result = unsafe {
            ffi::host_udp_recv_from(
                self.handle,
                buf.as_mut_ptr(),
                buf.len() as u32,
                addr_buf.as_mut_ptr(),
                timeout_ms,
            )
        };
        let n = check_with_value(result)? as usize;
        let addr_len = addr_buf.iter().position(|&b| b == 0).unwrap_or(64);
        let mut addr = String::new();
        if let Ok(s) = core::str::from_utf8(&addr_buf[..addr_len]) {
            let _ = addr.push_str(s);
        }
        Ok((n, addr))
    }

    pub fn recv(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        let (n, _) = self.recv_from(buf, timeout_ms)?;
        Ok(n)
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for UdpSocket {
    fn drop(&mut self) {
        unsafe { ffi::host_udp_close(self.handle) };
    }
}
