use crate::error::{check, check_with_value, Result, WasiError};
use heapless::String;

mod ffi {
    #[repr(C)]
    pub struct FileStat {
        pub size: u64,
        pub is_dir: u32,
    }

    #[link(wasm_import_module = "env")]
    extern "C" {
        pub fn host_file_open(path: *const core::ffi::c_char, access_mode: i32) -> i32;
        pub fn host_file_create(path: *const core::ffi::c_char) -> i32;
        pub fn host_file_close(handle: u32) -> i32;
        pub fn host_file_read(handle: u32, buf: *mut u8, buf_len: u32) -> i32;
        pub fn host_file_write(handle: u32, data: *const u8, data_len: u32) -> i32;
        pub fn host_file_seek(handle: u32, offset: i64, whence: i32) -> i64;
        pub fn host_file_stat(handle: u32, stat_out: *mut FileStat) -> i32;
        pub fn host_fs_stat(path: *const core::ffi::c_char, stat_out: *mut FileStat) -> i32;
        pub fn host_fs_mkdir(path: *const core::ffi::c_char) -> i32;
        pub fn host_fs_rmdir(path: *const core::ffi::c_char) -> i32;
        pub fn host_fs_remove(path: *const core::ffi::c_char) -> i32;
        pub fn host_fs_rename(
            old_path: *const core::ffi::c_char,
            new_path: *const core::ffi::c_char,
        ) -> i32;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AccessMode {
    ReadOnly = 0,
    WriteOnly = 1,
    ReadWrite = 2,
}

#[derive(Debug, Clone, Copy)]
pub enum SeekFrom {
    Start(u64),
    Current(i64),
    End(i64),
}

#[derive(Debug, Clone, Copy)]
pub struct FileStat {
    pub size: u64,
    pub is_dir: bool,
}

pub struct File {
    handle: u32,
}

impl File {
    pub fn open(path: &str, mode: AccessMode) -> Result<Self> {
        let mut c_path = String::<256>::new();
        c_path.push_str(path).map_err(|_| WasiError::InvalidArgument)?;
        c_path.push('\0').map_err(|_| WasiError::InvalidArgument)?;
        let result = unsafe { ffi::host_file_open(c_path.as_ptr() as *const _, mode as i32) };
        if result < 0 {
            return Err(WasiError::from_code(result));
        }
        Ok(File { handle: result as u32 })
    }

    pub fn create(path: &str) -> Result<Self> {
        let mut c_path = String::<256>::new();
        c_path.push_str(path).map_err(|_| WasiError::InvalidArgument)?;
        c_path.push('\0').map_err(|_| WasiError::InvalidArgument)?;
        let result = unsafe { ffi::host_file_create(c_path.as_ptr() as *const _) };
        if result < 0 {
            return Err(WasiError::from_code(result));
        }
        Ok(File { handle: result as u32 })
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let result = unsafe {
            ffi::host_file_read(self.handle, buf.as_mut_ptr(), buf.len() as u32)
        };
        if result == -12 {
            return Ok(0);
        }
        check_with_value(result).map(|n| n as usize)
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let result = unsafe {
            ffi::host_file_write(self.handle, data.as_ptr(), data.len() as u32)
        };
        check_with_value(result).map(|n| n as usize)
    }

    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let (offset, whence) = match pos {
            SeekFrom::Start(o) => (o as i64, 0),
            SeekFrom::Current(o) => (o, 1),
            SeekFrom::End(o) => (o, 2),
        };
        let result = unsafe { ffi::host_file_seek(self.handle, offset, whence) };
        if result < 0 {
            return Err(WasiError::from_code(result as i32));
        }
        Ok(result as u64)
    }

    pub fn stat(&self) -> Result<FileStat> {
        let mut stat = ffi::FileStat { size: 0, is_dir: 0 };
        let result = unsafe { ffi::host_file_stat(self.handle, &mut stat) };
        check(result)?;
        Ok(FileStat {
            size: stat.size,
            is_dir: stat.is_dir != 0,
        })
    }

    pub fn handle(&self) -> u32 {
        self.handle
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe { ffi::host_file_close(self.handle) };
    }
}

pub fn stat(path: &str) -> Result<FileStat> {
    let mut c_path = String::<256>::new();
    c_path.push_str(path).map_err(|_| WasiError::InvalidArgument)?;
    c_path.push('\0').map_err(|_| WasiError::InvalidArgument)?;
    let mut stat = ffi::FileStat { size: 0, is_dir: 0 };
    let result = unsafe { ffi::host_fs_stat(c_path.as_ptr() as *const _, &mut stat) };
    check(result)?;
    Ok(FileStat {
        size: stat.size,
        is_dir: stat.is_dir != 0,
    })
}

pub fn mkdir(path: &str) -> Result<()> {
    let mut c_path = String::<256>::new();
    c_path.push_str(path).map_err(|_| WasiError::InvalidArgument)?;
    c_path.push('\0').map_err(|_| WasiError::InvalidArgument)?;
    let result = unsafe { ffi::host_fs_mkdir(c_path.as_ptr() as *const _) };
    check(result)
}

pub fn rmdir(path: &str) -> Result<()> {
    let mut c_path = String::<256>::new();
    c_path.push_str(path).map_err(|_| WasiError::InvalidArgument)?;
    c_path.push('\0').map_err(|_| WasiError::InvalidArgument)?;
    let result = unsafe { ffi::host_fs_rmdir(c_path.as_ptr() as *const _) };
    check(result)
}

pub fn remove(path: &str) -> Result<()> {
    let mut c_path = String::<256>::new();
    c_path.push_str(path).map_err(|_| WasiError::InvalidArgument)?;
    c_path.push('\0').map_err(|_| WasiError::InvalidArgument)?;
    let result = unsafe { ffi::host_fs_remove(c_path.as_ptr() as *const _) };
    check(result)
}

pub fn rename(old_path: &str, new_path: &str) -> Result<()> {
    let mut c_old = String::<256>::new();
    c_old.push_str(old_path).map_err(|_| WasiError::InvalidArgument)?;
    c_old.push('\0').map_err(|_| WasiError::InvalidArgument)?;
    let mut c_new = String::<256>::new();
    c_new.push_str(new_path).map_err(|_| WasiError::InvalidArgument)?;
    c_new.push('\0').map_err(|_| WasiError::InvalidArgument)?;
    let result = unsafe {
        ffi::host_fs_rename(c_old.as_ptr() as *const _, c_new.as_ptr() as *const _)
    };
    check(result)
}
