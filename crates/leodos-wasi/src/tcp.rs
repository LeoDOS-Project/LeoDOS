use crate::error::{check_with_value, Result, WasiError};
use heapless::String;

mod ffi {
    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn host_tcp_listen(bind_addr: *const core::ffi::c_char, port: u16) -> i32;
        pub fn host_tcp_accept(listener_handle: u32, addr_out: *mut u8, timeout_ms: i32) -> i32;
        pub fn host_tcp_listener_close(handle: u32) -> i32;
        pub fn host_tcp_connect(addr: *const core::ffi::c_char, port: u16) -> i32;
        pub fn host_tcp_read(handle: u32, buf: *mut u8, buf_len: u32) -> i32;
        pub fn host_tcp_write(handle: u32, data: *const u8, data_len: u32) -> i32;
        pub fn host_tcp_close(handle: u32) -> i32;
    }
}

pub struct TcpListener {
    handle: u32,
}

impl TcpListener {
    pub fn bind(addr: &str, port: u16) -> Result<Self> {
        let mut c_addr = String::<64>::new();
        c_addr.push_str(addr).map_err(|_| WasiError::InvalidArgument)?;
        c_addr.push('\0').map_err(|_| WasiError::InvalidArgument)?;
        let result = unsafe { ffi::host_tcp_listen(c_addr.as_ptr() as *const _, port) };
        if result < 0 {
            return Err(WasiError::from_code(result));
        }
        Ok(TcpListener { handle: result as u32 })
    }

    pub fn accept(&self, timeout_ms: i32) -> Result<(TcpStream, String<64>)> {
        let mut addr_buf = [0u8; 64];
        let result = unsafe {
            ffi::host_tcp_accept(self.handle, addr_buf.as_mut_ptr(), timeout_ms)
        };
        let stream_handle = check_with_value(result)? as u32;
        let addr_len = addr_buf.iter().position(|&b| b == 0).unwrap_or(64);
        let mut addr = String::new();
        if let Ok(s) = core::str::from_utf8(&addr_buf[..addr_len]) {
            let _ = addr.push_str(s);
        }
        Ok((TcpStream { handle: stream_handle }, addr))
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        unsafe { ffi::host_tcp_listener_close(self.handle) };
    }
}

pub struct TcpStream {
    handle: u32,
}

impl TcpStream {
    pub fn connect(addr: &str, port: u16) -> Result<Self> {
        let mut c_addr = String::<64>::new();
        c_addr.push_str(addr).map_err(|_| WasiError::InvalidArgument)?;
        c_addr.push('\0').map_err(|_| WasiError::InvalidArgument)?;
        let result = unsafe { ffi::host_tcp_connect(c_addr.as_ptr() as *const _, port) };
        if result < 0 {
            return Err(WasiError::from_code(result));
        }
        Ok(TcpStream { handle: result as u32 })
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let result = unsafe {
            ffi::host_tcp_read(self.handle, buf.as_mut_ptr(), buf.len() as u32)
        };
        check_with_value(result).map(|n| n as usize)
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let result = unsafe {
            ffi::host_tcp_write(self.handle, data.as_ptr(), data.len() as u32)
        };
        check_with_value(result).map(|n| n as usize)
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        unsafe { ffi::host_tcp_close(self.handle) };
    }
}
