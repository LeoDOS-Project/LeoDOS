//! Low-level PSP wrappers for memory information and access.

use crate::error::Result;
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;

/// Information about a specific memory range defined in the PSP memory table.
#[derive(Debug, Clone, Copy)]
pub struct MemRangeInfo {
    /// The type of memory (e.g., RAM, EEPROM).
    pub memory_type: u32,
    /// The starting address of the memory range.
    pub start_addr: usize,
    /// The size of the memory range in bytes.
    pub size: usize,
    /// The word size of the memory (e.g., 1, 2, 4 bytes).
    pub word_size: usize,
    /// The attributes of the memory (e.g., read, write).
    pub attributes: u32,
}

/// Returns the location and size of the ES Reset information area.
pub fn get_reset_area() -> Result<(usize, usize)> {
    let mut ptr = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();
    check(unsafe { ffi::CFE_PSP_GetResetArea(ptr.as_mut_ptr(), size.as_mut_ptr()) })?;
    Ok((unsafe { ptr.assume_init() }, unsafe { size.assume_init() }
        as usize))
}

/// Returns the location and size of the user-reserved memory area.
pub fn get_user_reserved_area() -> Result<(usize, usize)> {
    let mut ptr = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();
    check(unsafe { ffi::CFE_PSP_GetUserReservedArea(ptr.as_mut_ptr(), size.as_mut_ptr()) })?;
    Ok((unsafe { ptr.assume_init() }, unsafe { size.assume_init() }
        as usize))
}

/// Returns the location and size of the memory used for the cFE volatile disk.
pub fn get_volatile_disk_mem() -> Result<(usize, usize)> {
    let mut ptr = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();
    check(unsafe { ffi::CFE_PSP_GetVolatileDiskMem(ptr.as_mut_ptr(), size.as_mut_ptr()) })?;
    Ok((unsafe { ptr.assume_init() }, unsafe { size.assume_init() }
        as usize))
}

/// Validates a memory range against the PSP's memory map.
pub fn mem_validate_range(address: usize, size: usize, memory_type: u32) -> Result<()> {
    check(unsafe { ffi::CFE_PSP_MemValidateRange(address, size, memory_type) })?;
    Ok(())
}

/// Returns the number of memory ranges defined in the PSP's memory table.
pub fn get_mem_ranges() -> u32 {
    unsafe { ffi::CFE_PSP_MemRanges() }
}

/// Retrieves one of the records from the PSP's memory table.
pub fn get_mem_range(range_num: u32) -> Result<MemRangeInfo> {
    let mut mem_type = MaybeUninit::uninit();
    let mut start_addr = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();
    let mut word_size = MaybeUninit::uninit();
    let mut attributes = MaybeUninit::uninit();
    check(unsafe {
        ffi::CFE_PSP_MemRangeGet(
            range_num,
            mem_type.as_mut_ptr(),
            start_addr.as_mut_ptr(),
            size.as_mut_ptr(),
            word_size.as_mut_ptr(),
            attributes.as_mut_ptr(),
        )
    })?;
    Ok(MemRangeInfo {
        memory_type: unsafe { mem_type.assume_init() },
        start_addr: unsafe { start_addr.assume_init() },
        size: unsafe { size.assume_init() },
        word_size: unsafe { word_size.assume_init() },
        attributes: unsafe { attributes.assume_init() },
    })
}

/// Returns the location and size of the kernel text segment.
///
/// This may not be implemented on all platforms.
pub fn get_kernel_text_segment_info() -> Result<(usize, usize)> {
    let mut ptr = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();
    check(unsafe { ffi::CFE_PSP_GetKernelTextSegmentInfo(ptr.as_mut_ptr(), size.as_mut_ptr()) })?;
    Ok((unsafe { ptr.assume_init() }, unsafe { size.assume_init() }
        as usize))
}

/// Returns the location and size of the CFE text segment.
pub fn get_cfe_text_segment_info() -> Result<(usize, usize)> {
    let mut ptr = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();
    check(unsafe { ffi::CFE_PSP_GetCFETextSegmentInfo(ptr.as_mut_ptr(), size.as_mut_ptr()) })?;
    Ok((unsafe { ptr.assume_init() }, unsafe { size.assume_init() }
        as usize))
}
