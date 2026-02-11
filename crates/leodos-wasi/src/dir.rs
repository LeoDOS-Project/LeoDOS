use crate::error::{check, Result, WasiError};
use heapless::String;

mod ffi {
    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn host_dir_open(path: *const core::ffi::c_char) -> i32;
        pub fn host_dir_close(handle: u32) -> i32;
        pub fn host_dir_read(handle: u32, buf: *mut u8, buf_len: i32) -> i32;
        pub fn host_dir_rewind(handle: u32) -> i32;
    }
}

pub struct Directory {
    handle: u32,
}

impl Directory {
    pub fn open(path: &str) -> Result<Self> {
        let mut c_path = String::<256>::new();
        c_path.push_str(path).map_err(|_| WasiError::InvalidArgument)?;
        c_path.push('\0').map_err(|_| WasiError::InvalidArgument)?;
        let result = unsafe { ffi::host_dir_open(c_path.as_ptr() as *const _) };
        if result < 0 {
            return Err(WasiError::from_code(result));
        }
        Ok(Directory { handle: result as u32 })
    }

    pub fn read_next(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        let result = unsafe {
            ffi::host_dir_read(self.handle, buf.as_mut_ptr(), buf.len() as i32)
        };
        if result == -13 {
            return Ok(None);
        }
        if result < 0 {
            return Err(WasiError::from_code(result));
        }
        Ok(Some(result as usize))
    }

    pub fn rewind(&mut self) -> Result<()> {
        let result = unsafe { ffi::host_dir_rewind(self.handle) };
        check(result)
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        unsafe { ffi::host_dir_close(self.handle) };
    }
}

impl Iterator for Directory {
    type Item = Result<String<256>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0u8; 256];
        match self.read_next(&mut buf) {
            Ok(Some(len)) => {
                let mut s = String::new();
                if let Ok(str_slice) = core::str::from_utf8(&buf[..len]) {
                    if s.push_str(str_slice).is_err() {
                        return Some(Err(WasiError::InvalidArgument));
                    }
                }
                Some(Ok(s))
            }
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}
