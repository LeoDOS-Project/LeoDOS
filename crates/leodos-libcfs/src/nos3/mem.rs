//! Safe device memory access wrappers.
//!
//! Wraps the hwlib `devmem_*` functions for reading and writing
//! physical memory-mapped registers.

use super::{check_mem, MemError};
use crate::ffi;

/// Writes `data` to the physical address `addr`.
pub fn write(addr: u32, data: &[u8]) -> Result<(), MemError> {
    check_mem(unsafe {
        ffi::devmem_write(
            addr,
            data.as_ptr() as *mut _,
            data.len() as i32,
        )
    })
}

/// Reads `buf.len()` bytes from the physical address `addr`.
pub fn read(addr: u32, buf: &mut [u8]) -> Result<(), MemError> {
    check_mem(unsafe {
        ffi::devmem_read(
            addr,
            buf.as_mut_ptr(),
            buf.len() as i32,
        )
    })
}
