use bon::bon;
use core::mem::size_of;
use core::ops::Deref;
use core::ops::DerefMut;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U32;

use crate::network::cfe::tc::Telecommand;
use crate::network::cfe::tc::TelecommandError;
use crate::network::cfe::tc::TelecommandSecondaryHeader;
use crate::network::spp::Apid;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SequenceCount;
use crate::network::spp::SpacePacket;
use crate::utils::checksum_u8;
use crate::utils::validate_checksum_u8;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ColoniesOpCode {
    /// Executor -> Server. Request a job.
    AssignRequest = 0x00,
    /// Server -> Executor. Assign a job.
    AssignResponse = 0x01,
    /// Executor -> Server. Close a job.
    Close = 0x02,
    /// Executor -> Server. Indicate job failure.
    Fail = 0x03,
    /// Bidirectional. Keep-alive message.
    KeepAlive = 0x04,
}

impl TryFrom<u8> for ColoniesOpCode {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::AssignRequest),
            0x01 => Ok(Self::AssignResponse),
            0x02 => Ok(Self::Close),
            0x03 => Ok(Self::Fail),
            0x04 => Ok(Self::KeepAlive),
            _ => Err(()),
        }
    }
}

#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct ColoniesPacket {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub colonies: ColoniesHeader,
    pub payload: [u8],
}

#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct ColoniesHeader {
    pub op_code: u8,
    pub _reserved: [u8; 3],
    pub msg_id: U32,
}

impl ColoniesHeader {
    pub fn op_code(&self) -> Result<ColoniesOpCode, ()> {
        self.op_code.try_into()
    }
    pub fn set_op_code(&mut self, op_code: ColoniesOpCode) {
        self.op_code = op_code as u8;
    }

    pub fn msg_id(&self) -> u32 {
        self.msg_id.get()
    }
    pub fn set_msg_id(&mut self, msg_id: u32) {
        self.msg_id = U32::new(msg_id);
    }
}

impl Deref for ColoniesPacket {
    type Target = SpacePacket;
    fn deref(&self) -> &Self::Target {
        SpacePacket::ref_from_bytes(self.as_bytes())
            .expect("ColoniesPacket layout is a superset of SpacePacket")
    }
}

impl DerefMut for ColoniesPacket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        SpacePacket::mut_from_bytes(self.as_mut_bytes())
            .expect("ColoniesPacket layout is a superset of SpacePacket")
    }
}

#[derive(Debug)]
pub enum ColoniesMessageError {
    Telecommand(TelecommandError),
    PayloadTooLarge,
    PayloadTooSmall,
    Parse,
}

#[bon]
impl ColoniesPacket {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: SequenceCount,
        op_code: ColoniesOpCode,
        msg_id: u32,
        payload_len: usize,
    ) -> Result<&'a mut Self, ColoniesMessageError> {
        if payload_len > u16::MAX as usize {
            return Err(ColoniesMessageError::PayloadTooLarge);
        }

        let tc = Telecommand::builder()
            .buffer(buffer)
            .apid(apid)
            .sequence_count(sequence_count)
            .function_code(0) // Unused in Colonies
            .payload_len(size_of::<ColoniesHeader>() + payload_len)
            .build()
            .map_err(ColoniesMessageError::Telecommand)?;

        let required_len = size_of::<PrimaryHeader>()
            + size_of::<TelecommandSecondaryHeader>()
            + size_of::<ColoniesHeader>()
            + payload_len;

        let buffer = tc.as_mut_bytes();
        let provided_len = buffer.len();

        let packet = Self::mut_from_bytes_with_elems(buffer, required_len).map_err(|_| {
            ColoniesMessageError::Telecommand(TelecommandError::BufferTooSmall {
                required_len,
                provided_len,
            })
        })?;

        packet.colonies.set_op_code(op_code);
        packet.colonies.set_msg_id(msg_id);

        Ok(packet)
    }

    pub fn set_cfe_checksum(&mut self) {
        self.secondary.checksum = 0;
        self.secondary.checksum = checksum_u8(self.as_bytes());
    }
    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }
}

impl ColoniesPacket {
    pub fn parse(bytes: &[u8]) -> Result<&Self, ColoniesMessageError> {
        let tc = Telecommand::parse(bytes).map_err(ColoniesMessageError::Telecommand)?;
        <&ColoniesPacket>::try_from(tc)
    }
}

impl<'a> TryFrom<&'a Telecommand> for &'a ColoniesPacket {
    type Error = ColoniesMessageError;

    fn try_from(tc: &'a Telecommand) -> Result<Self, Self::Error> {
        if tc.payload().len() < size_of::<ColoniesHeader>() {
            return Err(ColoniesMessageError::PayloadTooSmall);
        }
        Ok(ColoniesPacket::ref_from_bytes(tc.as_bytes()).unwrap())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ArgIterator<'a> {
    buffer: &'a [u8],
}

impl<'a> ArgIterator<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }
}

impl<'a> Iterator for ArgIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer.len() < 2 {
            return None;
        }
        let len_bytes: [u8; 2] = self.buffer[0..2].try_into().ok()?;
        let len = u16::from_be_bytes(len_bytes) as usize;

        let total_len = 2 + len;
        if self.buffer.len() < total_len {
            return None;
        }

        let value = &self.buffer[2..total_len];
        self.buffer = &self.buffer[total_len..];
        Some(value)
    }
}

/// Helper to write Length-Value (LV) encoded bytes into the packet buffer.
pub struct PayloadWriter<'a> {
    buffer: &'a mut [u8],
    cursor: usize,
}

impl<'a> PayloadWriter<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, cursor: 0 }
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), ()> {
        let len = data.len();

        if self.cursor + 2 + len > self.buffer.len() {
            return Err(());
        }

        let len_bytes = (len as u16).to_be_bytes();
        self.buffer[self.cursor] = len_bytes[0];
        self.buffer[self.cursor + 1] = len_bytes[1];
        self.cursor += 2;

        self.buffer[self.cursor..self.cursor + len].copy_from_slice(data);
        self.cursor += len;

        Ok(())
    }

    pub fn write_str(&mut self, s: &str) -> Result<(), ()> {
        self.write_bytes(s.as_bytes())
    }

    pub fn len(&self) -> usize {
        self.cursor
    }
    
    pub fn remaining_capacity(&self) -> usize {
        self.buffer.len() - self.cursor
    }
}
