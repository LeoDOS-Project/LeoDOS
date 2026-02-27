use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

use crate::transport::cfdp::pdu::file_directive::ConditionCode;
use crate::transport::cfdp::CfdpError;
use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

/// A zero-copy view of the Value field of a Fault Handler Override TLV.
#[repr(C)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct TlvFaultHandlerOverride {
    packed: u8,
}

#[rustfmt::ignore]
mod bitmasks {
    pub const CONDITION_CODE_MASK: u8 = 0b_11110000;
    pub const HANDLER_CODE_MASK: u8 = 0b_00001111;
}

use bitmasks::*;

/// Handler codes for Fault Handler Override TLV (Table 5-19)
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum HandlerCode {
    /// Cancel the transaction (default).
    #[default]
    Cancel = 0x01,
    /// Suspend the transaction.
    Suspend = 0x02,
    /// Ignore the fault and continue.
    Ignore = 0x03,
    /// Abandon the transaction without further PDUs.
    Abandon = 0x04,
}

impl TryFrom<u8> for HandlerCode {
    type Error = CfdpError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Cancel),
            0x02 => Ok(Self::Suspend),
            0x03 => Ok(Self::Ignore),
            0x04 => Ok(Self::Abandon),
            _ => Err(CfdpError::Custom("Invalid HandlerCode")),
        }
    }
}

impl TlvFaultHandlerOverride {
    /// The fault condition to be handled (Table 5-5).
    pub fn condition_code(&self) -> Result<ConditionCode, CfdpError> {
        get_bits_u8(self.packed, CONDITION_CODE_MASK).try_into()
    }
    /// Sets the fault condition code.
    pub fn set_condition_code(&mut self, code: ConditionCode) {
        set_bits_u8(&mut self.packed, CONDITION_CODE_MASK, code as u8);
    }

    /// The action to be taken (Table 5-19).
    pub fn handler_code(&self) -> Result<HandlerCode, CfdpError> {
        get_bits_u8(self.packed, HANDLER_CODE_MASK).try_into()
    }
    /// Sets the handler action code.
    pub fn set_handler_code(&mut self, code: HandlerCode) {
        set_bits_u8(&mut self.packed, HANDLER_CODE_MASK, code as u8);
    }
}

/// A bit-packed structure to store the fault handler action for all 16
/// possible CFDP condition codes in a single `u32`.
///
/// Each handler requires 2 bits to represent the 4 possible actions.
/// `16 conditions * 2 bits/condition = 32 bits`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FaultHandlerSet(u32);

impl Default for FaultHandlerSet {
    /// Creates a new `FaultHandlerSet` with the default handler
    /// (`HandlerCode::default()`, which is `Cancel`) for all conditions.
    fn default() -> Self {
        let default_handler_bits = HandlerCode::default() as u32;
        let mut packed_value = 0u32;
        // Pre-fill all 16 slots with the default handler.
        for i in 0..16 {
            let shift = i * 2;
            packed_value |= default_handler_bits << shift;
        }
        Self(packed_value)
    }
}

impl FaultHandlerSet {
    /// Creates a new `FaultHandlerSet` where all handlers are set to the
    /// specified default action.
    pub fn new(default_handler: HandlerCode) -> Self {
        let handler_bits = handler_to_bits(default_handler);
        let mut packed_value = 0u32;
        for i in 0..16 {
            let shift = i * 2;
            packed_value |= handler_bits << shift;
        }
        Self(packed_value)
    }

    /// Sets the handler for a specific condition code.
    pub fn set_handler(&mut self, condition: ConditionCode, handler: HandlerCode) {
        // ConditionCode enum values are their integer representations.
        let condition_index = condition as u8;
        let shift = condition_index * 2;
        let handler_bits = handler_to_bits(handler);

        // 1. Create a mask to clear the 2 bits for this condition.
        // e.g., for index 3, shift=6, mask = !(0b11 << 6) = !(11000000) = 00111111
        let mask = !(0b11 << shift);

        // 2. Clear the existing bits.
        self.0 &= mask;

        // 3. Set the new bits.
        self.0 |= handler_bits << shift;
    }

    /// Gets the handler for a specific condition code.
    pub fn get_handler(&self, condition: ConditionCode) -> HandlerCode {
        let condition_index = condition as u8;
        let shift = condition_index * 2;

        // 1. Shift the relevant bits to the LSB position.
        let bits = (self.0 >> shift) & 0b11;

        // 2. Convert the 2-bit value back to a HandlerCode.
        bits_to_handler(bits)
    }
}

// Private helper functions to map between HandlerCode and its 2-bit representation.
// Using this mapping:
// 00 -> Cancel (as it's more common to default to a safe state)
// 01 -> Suspend
// 10 -> Ignore
// 11 -> Abandon
// NOTE: `HandlerCode` enum values are 1, 2, 3, 4. We map them to 0, 1, 2, 3.
const fn handler_to_bits(handler: HandlerCode) -> u32 {
    match handler {
        HandlerCode::Cancel => 0b00,
        HandlerCode::Suspend => 0b01,
        HandlerCode::Ignore => 0b10,
        HandlerCode::Abandon => 0b11,
    }
}

const fn bits_to_handler(bits: u32) -> HandlerCode {
    match bits {
        0b00 => HandlerCode::Cancel,
        0b01 => HandlerCode::Suspend,
        0b10 => HandlerCode::Ignore,
        0b11 => HandlerCode::Abandon,
        // This case is unreachable if the mask is correct, but defensive.
        _ => HandlerCode::Cancel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fault_handler_set_default() {
        let set = FaultHandlerSet::default();
        // Check a few condition codes to ensure they are all the default.
        assert_eq!(
            set.get_handler(ConditionCode::NoError),
            HandlerCode::default()
        );
        assert_eq!(
            set.get_handler(ConditionCode::FileChecksumFailure),
            HandlerCode::default()
        );
        assert_eq!(
            set.get_handler(ConditionCode::CancelReceived),
            HandlerCode::default()
        );
    }

    #[test]
    fn test_fault_handler_set_and_get() {
        let mut set = FaultHandlerSet::default();

        // Ensure default is Cancel
        assert_eq!(
            set.get_handler(ConditionCode::FileChecksumFailure),
            HandlerCode::Cancel
        );

        // Set a new handler
        set.set_handler(ConditionCode::FileChecksumFailure, HandlerCode::Ignore);
        assert_eq!(
            set.get_handler(ConditionCode::FileChecksumFailure),
            HandlerCode::Ignore
        );

        // Ensure other handlers are unaffected
        assert_eq!(
            set.get_handler(ConditionCode::NakLimitReached),
            HandlerCode::Cancel
        );

        // Set another handler and verify both
        set.set_handler(ConditionCode::NakLimitReached, HandlerCode::Suspend);
        assert_eq!(
            set.get_handler(ConditionCode::FileChecksumFailure),
            HandlerCode::Ignore
        );
        assert_eq!(
            set.get_handler(ConditionCode::NakLimitReached),
            HandlerCode::Suspend
        );

        // Overwrite a handler
        set.set_handler(ConditionCode::FileChecksumFailure, HandlerCode::Abandon);
        assert_eq!(
            set.get_handler(ConditionCode::FileChecksumFailure),
            HandlerCode::Abandon
        );
    }
}
