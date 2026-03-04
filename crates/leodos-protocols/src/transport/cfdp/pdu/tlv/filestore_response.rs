use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::filestore::FileId;
use crate::transport::cfdp::pdu::tlv::FilestoreAction;

/// A zero-copy view of the Value of a Filestore Response TLV.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct TlvFilestoreResponse {
    /// Packed byte with the 4-bit action code and 4-bit status code.
    action_and_status_code: u8,
    /// Contains LV-encoded file names and an LV-encoded message.
    rest: [u8],
}

/// A parsed filestore response containing the action and file names.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FilestoreResponse {
    /// The filestore action that was requested.
    pub action: FilestoreAction,
    /// The primary file name from the original request.
    pub first_file_name: FileId,
    /// The secondary file name, used by rename/append/replace actions.
    pub second_file_name: FileId,
}

impl TlvFilestoreResponse {
    /// Returns the filestore action code from the packed byte.
    pub fn action(&self) -> Result<FilestoreAction, CfdpError> {
        (self.action_and_status_code >> 4).try_into()
    }

    /// Returns the 4-bit status code from the packed byte.
    pub fn status_code(&self) -> u8 {
        self.action_and_status_code & 0x0F
    }

    // NOTE: The parsing logic here can get complex. This implementation assumes a fixed order.
    // The spec is slightly ambiguous on whether all fields are always present.
    // This implementation follows the implied structure from Table 5-17.

    /// Parses and returns the first file name from the response.
    pub fn first_file_name(&self) -> Result<&[u8], CfdpError> {
        let len = *self
            .rest
            .first()
            .ok_or(CfdpError::Custom("Missing first file name length"))? as usize;
        self.rest
            .get(1..1 + len)
            .ok_or(CfdpError::Custom("Invalid first file name slice"))
    }

    /// Parses the rest of the TLV value to find the second file name and the message.
    /// Returns `(second_file_name, message, remainder)`
    /// Parses the second file name and status message from after the first LV name.
    fn parse_after_first_name(&self) -> Result<(Option<&[u8]>, &[u8]), CfdpError> {
        let first_lv_len = 1 + self.first_file_name()?.len();
        let mut remainder = self
            .rest
            .get(first_lv_len..)
            .ok_or(CfdpError::Custom("Invalid slice after first file name"))?;

        let has_second = matches!(
            self.action()?,
            FilestoreAction::RenameFile
                | FilestoreAction::AppendFile
                | FilestoreAction::ReplaceFile
        );

        let second_file_name = if has_second {
            let len = *remainder
                .first()
                .ok_or(CfdpError::Custom("Missing second name len"))?
                as usize;
            remainder = remainder
                .get(1..)
                .ok_or(CfdpError::Custom("Invalid FS Response"))?;
            let (name, rest) = remainder.split_at(len);
            remainder = rest;
            Some(name)
        } else {
            None
        };

        let msg_len = *remainder
            .first()
            .ok_or(CfdpError::Custom("Missing message len"))? as usize;
        let message = remainder
            .get(1..1 + msg_len)
            .ok_or(CfdpError::Custom("Invalid message slice"))?;

        Ok((second_file_name, message))
    }

    /// Returns the second file name, if present for the action type.
    pub fn second_file_name(&self) -> Result<Option<&[u8]>, CfdpError> {
        self.parse_after_first_name().map(|(name, _)| name)
    }

    /// Returns the filestore status message bytes.
    pub fn message(&self) -> Result<&[u8], CfdpError> {
        self.parse_after_first_name().map(|(_, msg)| msg)
    }
}
