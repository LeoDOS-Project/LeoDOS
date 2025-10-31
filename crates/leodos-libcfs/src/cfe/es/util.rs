//! Miscellaneous utility wrappers for CFE Executive Services APIs.

use crate::ffi;

/// Specifies the CRC algorithm to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CrcType {
    /// 16-bit CRC algorithm (CRC-16/ARC). This is the default for cFE.
    Crc16 = ffi::CFE_ES_CrcType_Enum_CFE_ES_CrcType_16_ARC,
    // Add other types like Crc8 and Crc32 when they are fully supported and needed.
}

/// Calculates a cyclic redundancy check (CRC) on a block of memory.
///
/// This routine can be used to calculate a CRC on contiguous or non-contiguous blocks of memory.
///
/// # Arguments
/// * `data`: A slice of bytes to calculate the CRC over.
/// * `input_crc`: A starting value for the CRC calculation. For non-contiguous blocks,
///   this should be the result of a previous call to this function. For a new calculation,
///   this should typically be 0.
/// * `crc_type`: The CRC algorithm to use.
pub fn calculate_crc(data: &[u8], input_crc: u32, crc_type: CrcType) -> u32 {
    unsafe {
        ffi::CFE_ES_CalculateCRC(
            data.as_ptr() as *const _,
            data.len(),
            input_crc,
            crc_type as ffi::CFE_ES_CrcType_Enum_t,
        )
    }
}
