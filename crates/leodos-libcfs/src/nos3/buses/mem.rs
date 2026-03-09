//! Device memory (memory-mapped I/O).
//!
//! Reads and writes physical memory addresses for direct
//! register access on FPGA or SoC peripherals.

use crate::ffi;

/// Errors from device memory operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum MemError {
    /// Generic OS/driver error (`MEM_ERROR`).
    #[error("DevMem: OS error")]
    OsError,
    /// Unhandled error code.
    #[error("DevMem: unhandled error ({0})")]
    Unhandled(i32),
}

fn check(rc: i32) -> Result<(), MemError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::MEM_ERROR => Err(MemError::OsError),
        other => Err(MemError::Unhandled(other)),
    }
}

/// Writes `data` to the physical address `addr`.
pub fn write(addr: u32, data: &[u8]) -> Result<(), MemError> {
    check(unsafe {
        ffi::devmem_write(
            addr,
            data.as_ptr() as *mut _,
            data.len() as i32,
        )
    })
}

/// Reads `buf.len()` bytes from the physical address `addr`.
pub fn read(addr: u32, buf: &mut [u8]) -> Result<(), MemError> {
    check(unsafe {
        ffi::devmem_read(
            addr,
            buf.as_mut_ptr(),
            buf.len() as i32,
        )
    })
}
