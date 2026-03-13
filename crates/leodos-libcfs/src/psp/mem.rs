//! Low-level PSP wrappers for memory information and access.

use crate::error::Result;
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;

/// Type of memory in the PSP memory table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MemoryType {
    /// Volatile random-access memory.
    Ram = ffi::CFE_PSP_MEM_RAM,
    /// Non-volatile electrically erasable memory.
    Eeprom = ffi::CFE_PSP_MEM_EEPROM,
    /// Matches any memory type during validation.
    Any = ffi::CFE_PSP_MEM_ANY,
    /// Invalid / unrecognized memory type.
    Invalid = ffi::CFE_PSP_MEM_INVALID,
}

impl From<u32> for MemoryType {
    fn from(val: u32) -> Self {
        match val {
            ffi::CFE_PSP_MEM_RAM => Self::Ram,
            ffi::CFE_PSP_MEM_EEPROM => Self::Eeprom,
            ffi::CFE_PSP_MEM_ANY => Self::Any,
            _ => Self::Invalid,
        }
    }
}

impl From<MemoryType> for u32 {
    fn from(val: MemoryType) -> u32 {
        val as u32
    }
}

/// Information about a specific memory range defined in the PSP memory table.
#[derive(Debug, Clone, Copy)]
pub struct MemRangeInfo {
    /// The type of memory (e.g., RAM, EEPROM).
    pub memory_type: MemoryType,
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
///
/// This area is preserved across processor resets. It stores the
/// ER Log, System Log, and reset-related variables.
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
///
/// May return `INVALID_MEM_ADDR` (bad start address),
/// `INVALID_MEM_TYPE` (type mismatch), or `INVALID_MEM_RANGE`
/// (range too small for address + size).
pub fn mem_validate_range(address: usize, size: usize, memory_type: MemoryType) -> Result<()> {
    check(unsafe { ffi::CFE_PSP_MemValidateRange(address, size, memory_type as u32) })?;
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
        memory_type: MemoryType::from(unsafe { mem_type.assume_init() }),
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
///
/// This may not be implemented on all platforms.
pub fn get_cfe_text_segment_info() -> Result<(usize, usize)> {
    let mut ptr = MaybeUninit::uninit();
    let mut size = MaybeUninit::uninit();
    check(unsafe { ffi::CFE_PSP_GetCFETextSegmentInfo(ptr.as_mut_ptr(), size.as_mut_ptr()) })?;
    Ok((unsafe { ptr.assume_init() }, unsafe { size.assume_init() }
        as usize))
}
