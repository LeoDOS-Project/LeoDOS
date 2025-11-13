pub mod entity_id;
pub mod fault_handler_override;
pub mod filestore_request;
pub mod filestore_response;
pub mod flow_label;
pub mod message_to_user;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

use crate::transport::cfdp::pdu::CfdpError;

/// A single, unified, zero-copy view of a TLV record.
/// It contains the Type, Length, and the variable-length Value.
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, Immutable, KnownLayout)]
pub struct Tlv {
    tlv_type: u8,
    length: u8,
    value: [u8],
}

impl Tlv {
    /// Returns the TLV's type.
    pub fn tlv_type(&self) -> Result<TlvType, CfdpError> {
        TlvType::try_from(self.tlv_type)
    }
    pub fn set_type(&mut self, tlv_type: TlvType) {
        self.tlv_type = tlv_type as u8;
    }

    /// Returns the length of the value field in bytes.
    pub fn length(&self) -> usize {
        self.length as usize
    }
    pub fn set_length(&mut self, length: usize) -> Result<(), CfdpError> {
        if length > u8::MAX as usize {
            return Err(CfdpError::Custom("Length exceeds maximum value for TLV"));
        }
        self.length = length as u8;
        Ok(())
    }

    /// Returns an immutable slice of the value field.
    pub fn value(&self) -> &[u8] {
        &self.value
    }
    pub fn set_value(&mut self, value: &[u8]) -> Result<(), CfdpError> {
        let len = value.len();
        self.value
            .get_mut(..len)
            .ok_or(CfdpError::Custom("Value length exceeds allocated size"))?
            .copy_from_slice(value);
        Ok(())
    }

    /// Calculates the total length of this TLV instance in bytes.
    pub fn total_len(&self) -> usize {
        2 + self.length() // 2 bytes for Type and Length
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TlvType {
    FilestoreRequest = 0x00,
    FilestoreResponse = 0x01,
    MessageToUser = 0x02,
    // 0x03 is unused/reserved
    FaultHandlerOverride = 0x04,
    FlowLabel = 0x05,
    EntityId = 0x06,
}

impl TryFrom<u8> for TlvType {
    type Error = CfdpError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(TlvType::FilestoreRequest),
            0x01 => Ok(TlvType::FilestoreResponse),
            0x02 => Ok(TlvType::MessageToUser),
            0x04 => Ok(TlvType::FaultHandlerOverride),
            0x05 => Ok(TlvType::FlowLabel),
            0x06 => Ok(TlvType::EntityId),
            _ => Err(CfdpError::Custom("Unknown TLV type")),
        }
    }
}

pub struct TlvIterator<'a> {
    pub buffer: &'a [u8],
}

impl<'a> Iterator for TlvIterator<'a> {
    type Item = &'a Tlv;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer.len() < 2 {
            return None;
        }
        let length = self.buffer[1] as usize;
        let total_len = 2 + length;
        if self.buffer.len() < total_len {
            self.buffer = &[];
            return None;
        }
        let (tlv_bytes, rest) = self.buffer.split_at(total_len);
        let tlv = Tlv::ref_from_bytes(tlv_bytes).ok()?;
        self.buffer = rest;

        Some(tlv)
    }
}

/// Action codes for Filestore Request TLV (Table 5-16)
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum FilestoreAction {
    CreateFile = 0x00,
    DeleteFile = 0x01,
    RenameFile = 0x02,
    AppendFile = 0x03,
    ReplaceFile = 0x04,
    CreateDirectory = 0x05,
    RemoveDirectory = 0x06,
    DenyFile = 0x07,
    DenyDirectory = 0x08,
}

impl TryFrom<u8> for FilestoreAction {
    type Error = CfdpError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::CreateFile),
            0x01 => Ok(Self::DeleteFile),
            0x02 => Ok(Self::RenameFile),
            0x03 => Ok(Self::AppendFile),
            0x04 => Ok(Self::ReplaceFile),
            0x05 => Ok(Self::CreateDirectory),
            0x06 => Ok(Self::RemoveDirectory),
            0x07 => Ok(Self::DenyFile),
            0x08 => Ok(Self::DenyDirectory),
            _ => Err(CfdpError::Custom("Invalid FilestoreAction code")),
        }
    }
}
