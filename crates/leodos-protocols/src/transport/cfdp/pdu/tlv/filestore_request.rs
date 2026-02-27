use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::filestore::FileId;
use crate::transport::cfdp::pdu::tlv::FilestoreAction;
use crate::transport::cfdp::pdu::tlv::Tlv;
use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

/// A zero-copy view of the Value of a Filestore Request TLV.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct TlvFilestoreRequest {
    action_code_and_spare: u8,
    /// Contains LV-encoded file names.
    rest: [u8],
}

/// A parsed filestore request containing the action and file names.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FilestoreRequest {
    /// The filestore action to perform.
    pub action: FilestoreAction,
    /// The primary file name for the action.
    pub first_file_name: FileId,
    /// The secondary file name, required for rename/append/replace actions.
    pub second_file_name: Option<FileId>,
}

#[rustfmt::skip]
mod bitmasks {
    pub const ACTION_CODE_MASK: u8 = 0b1111_0000;
    pub const _SPARE_MASK: u8 =      0b0000_1111;
}

impl TlvFilestoreRequest {
    /// Parses a `TlvFilestoreRequest` from a generic `Tlv` reference.
    pub fn from_tlv(tlv: &Tlv) -> Result<&Self, CfdpError> {
        if tlv.length() < 1 {
            return Err(CfdpError::Custom("Filestore Request TLV too short"));
        }
        TlvFilestoreRequest::ref_from_bytes(tlv.value())
            .map_err(|_| CfdpError::Custom("Failed to parse Filestore Request TLV"))
    }

    /// Returns the filestore action code from the packed byte.
    pub fn action(&self) -> Result<FilestoreAction, CfdpError> {
        get_bits_u8(self.action_code_and_spare, bitmasks::ACTION_CODE_MASK).try_into()
    }

    /// Sets the filestore action code.
    pub fn set_action(&mut self, action: FilestoreAction) {
        set_bits_u8(
            &mut self.action_code_and_spare,
            bitmasks::ACTION_CODE_MASK,
            action as u8,
        );
    }

    /// Parses and returns the first file name.
    pub fn first_file_name(&self) -> Result<&[u8], CfdpError> {
        let len = *self
            .rest
            .first()
            .ok_or(CfdpError::Custom("Missing first file name length"))? as usize;
        self.rest
            .get(1..1 + len)
            .ok_or(CfdpError::Custom("Invalid first file name slice"))
    }

    /// Parses and returns the second file name, if present for the action type.
    pub fn second_file_name(&self) -> Result<Option<&[u8]>, CfdpError> {
        let has_second = matches!(
            self.action()?,
            FilestoreAction::RenameFile
                | FilestoreAction::AppendFile
                | FilestoreAction::ReplaceFile
        );
        if !has_second {
            return Ok(None);
        }

        let first_lv_len = 1 + self.first_file_name()?.len();
        let remainder = self
            .rest
            .get(first_lv_len..)
            .ok_or(CfdpError::Custom("Invalid slice after first file name"))?;

        let len = *remainder
            .first()
            .ok_or(CfdpError::Custom("Missing second file name length"))?
            as usize;
        let name = remainder
            .get(1..1 + len)
            .ok_or(CfdpError::Custom("Invalid second file name slice"))?;
        Ok(Some(name))
    }
}
