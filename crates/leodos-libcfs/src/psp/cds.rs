//! CDS (Critical Data Store) interface.

use crate::error::Result;
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;

/// Gets the size of the CDS memory area from the PSP.
pub fn get_cds_size() -> Result<usize> {
    let mut size = MaybeUninit::uninit();
    check(unsafe { ffi::CFE_PSP_GetCDSSize(size.as_mut_ptr()) })?;
    Ok(unsafe { size.assume_init() } as usize)
}

/// Writes a block of data to the CDS at a specified offset.
///
/// # Safety
/// This is a raw memory write. The caller must ensure that `offset + data.len()`
/// does not exceed the total size of the CDS.
pub unsafe fn write_to_cds(data: &[u8], offset: usize) -> Result<()> {
    check(ffi::CFE_PSP_WriteToCDS(
        data.as_ptr() as *const _,
        offset as u32,
        data.len() as u32,
    ))?;
    Ok(())
}

/// Reads a block of data from the CDS at a specified offset.
///
/// # Safety
/// This is a raw memory read. The caller must ensure that `offset + buf.len()`
/// does not exceed the total size of the CDS.
pub unsafe fn read_from_cds(buf: &mut [u8], offset: usize) -> Result<()> {
    check(ffi::CFE_PSP_ReadFromCDS(
        buf.as_mut_ptr() as *mut _,
        offset as u32,
        buf.len() as u32,
    ))?;
    Ok(())
}
