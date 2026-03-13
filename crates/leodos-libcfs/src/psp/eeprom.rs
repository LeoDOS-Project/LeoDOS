//! EEPROM (Electrically Erasable Programmable Read-Only Memory) interface.
//!
//! These functions are highly platform-specific and should only be used by
//! applications that need to directly interact with non-volatile memory hardware.
//! All functions in this module are `unsafe`.

use crate::error::Result;
use crate::ffi;
use crate::status::check;

/// Writes one byte to an EEPROM address.
///
/// May return `TIMEOUT` (write did not complete),
/// `ADDRESS_MISALIGNED`, or `NOT_IMPLEMENTED`.
pub unsafe fn write_u8(address: usize, value: u8) -> Result<()> {
    check(ffi::CFE_PSP_EepromWrite8(address, value))?;
    Ok(())
}

/// Writes two bytes to an EEPROM address.
///
/// May return `TIMEOUT`, `ADDRESS_MISALIGNED`, or
/// `NOT_IMPLEMENTED`.
pub unsafe fn write_u16(address: usize, value: u16) -> Result<()> {
    check(ffi::CFE_PSP_EepromWrite16(address, value))?;
    Ok(())
}

/// Writes four bytes to an EEPROM address.
///
/// May return `TIMEOUT`, `ADDRESS_MISALIGNED`, or
/// `NOT_IMPLEMENTED`.
pub unsafe fn write_u32(address: usize, value: u32) -> Result<()> {
    check(ffi::CFE_PSP_EepromWrite32(address, value))?;
    Ok(())
}

/// Enables write operations for a specific bank of EEPROM.
///
/// May return `NOT_IMPLEMENTED` on platforms without EEPROM.
pub unsafe fn write_enable(bank: u32) -> Result<()> {
    check(ffi::CFE_PSP_EepromWriteEnable(bank))?;
    Ok(())
}

/// Disables write operations for a specific bank of EEPROM.
///
/// May return `NOT_IMPLEMENTED` on platforms without EEPROM.
pub unsafe fn write_disable(bank: u32) -> Result<()> {
    check(ffi::CFE_PSP_EepromWriteDisable(bank))?;
    Ok(())
}

/// Powers up a specific bank of EEPROM.
///
/// May return `NOT_IMPLEMENTED` on platforms without EEPROM.
pub unsafe fn power_up(bank: u32) -> Result<()> {
    check(ffi::CFE_PSP_EepromPowerUp(bank))?;
    Ok(())
}

/// Powers down a specific bank of EEPROM.
///
/// May return `NOT_IMPLEMENTED` on platforms without EEPROM.
pub unsafe fn power_down(bank: u32) -> Result<()> {
    check(ffi::CFE_PSP_EepromPowerDown(bank))?;
    Ok(())
}
