//! A zero-copy view and builder for CCSDS Telecommand (TC) Transfer Frames.
//!
//! The TC Transfer Frame is the "envelope" used to package `SpacePacket`s for
//! uplink (sending commands from the ground to a satellite).

use crate::{FromBytes, IntoBytes, KnownLayout, Unaligned};
use zerocopy::byteorder::network_endian::U16;

use crate::builder::Vacant;
/// A typestate builder for constructing a `TCTransferFrame`.
pub use builder::TCFrameBuilder;

/// A zero-copy view over a TC Transfer Frame in a raw byte buffer.
///
/// This struct can be created via `TCTransferFrame::builder()` and provides
/// access to the frame header and its data field, which typically contains
/// one or more `SpacePacket`s.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout)]
pub struct TCTransferFrame {
    header: TCHeader,
    data_field: [u8],
}

/// An error that can occur during TC Transfer Frame construction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BuildError {
    /// The provided Spacecraft ID is outside the valid 10-bit range (0-1023).
    InvalidScid(u16),
    /// The provided Virtual Channel ID is outside the valid 6-bit range (0-63).
    InvalidVcid(u8),
    /// The provided data length exceeds the maximum of 1019 bytes.
    DataTooLong(usize),
    /// The provided buffer is too small to hold the requested frame.
    BufferTooSmall { required: usize, provided: usize },
}

/// The Bypass Flag, controlling the type of frame acceptance checks performed
/// by the receiving spacecraft.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum BypassFlag {
    /// The normal acceptance checks shall be performed (Type-A).
    TypeA = 0,
    /// The acceptance checks are bypassed (Type-B).
    TypeB = 1,
}

/// The Control Command Flag, indicating whether the frame contains user data or
/// control information for the receiver.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ControlFlag {
    /// The frame contains user data (e.g., a `SpacePacket`).
    TypeD = 0,
    /// The frame contains control information (Type-C).
    TypeC = 1,
}

/// The 5-byte header of a TC Transfer Frame.
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Debug, Copy, Clone)]
pub struct TCHeader {
    word0: U16,
    word1: U16,
    seq: u8,
}

impl TCTransferFrame {
    /// The size of the TC Transfer Frame header in bytes.
    pub const HEADER_SIZE: usize = 5;
    /// The maximum allowed size of the data field in bytes.
    pub const MAX_DATA_FIELD_LEN: usize = 1019;

    /// Creates a new builder to begin constructing a `TCTransferFrame`.
    pub fn builder() -> TCFrameBuilder<Vacant, Vacant, Vacant, Vacant> {
        TCFrameBuilder::new()
    }

    /// Returns a reference to the frame's header.
    pub fn header(&self) -> &TCHeader {
        &self.header
    }

    /// Returns a mutable reference to the frame's data field.
    ///
    /// This is typically used to copy a serialized `SpacePacket` into the frame.
    pub fn data_field_mut(&mut self) -> &mut [u8] {
        &mut self.data_field
    }

    /// Returns the total length of the frame (header + data field) in bytes.
    pub fn frame_len(&self) -> usize {
        Self::HEADER_SIZE + self.data_field.len()
    }
}

pub mod builder {
    use super::*;

    /// A typestate builder for `TCTransferFrame`.
    #[derive(Clone)]
    pub struct TCFrameBuilder<A, B, C, D> {
        scid: A,
        vcid: B,
        buffer: C,
        bypass_flag: BypassFlag,
        control_flag: ControlFlag,
        seq: u8,
        data_field_len: D,
    }

    impl TCFrameBuilder<Vacant, Vacant, Vacant, Vacant> {
        pub(crate) fn new() -> Self {
            Self {
                scid: Vacant,
                vcid: Vacant,
                buffer: Vacant,
                bypass_flag: BypassFlag::TypeA,
                control_flag: ControlFlag::TypeD,
                seq: 0,
                data_field_len: Vacant,
            }
        }
    }

    impl<A, B, C, D> TCFrameBuilder<A, B, C, D> {
        /// Sets the Bypass Flag (optional). Defaults to `TypeA`.
        pub fn bypass_flag(mut self, flag: BypassFlag) -> Self {
            self.bypass_flag = flag;
            self
        }
        /// Sets the Control Flag (optional). Defaults to `TypeD`.
        pub fn control_flag(mut self, flag: ControlFlag) -> Self {
            self.control_flag = flag;
            self
        }
        /// Sets the Frame Sequence Number (optional). Defaults to `0`.
        pub fn sequence_num(mut self, seq: u8) -> Self {
            self.seq = seq;
            self
        }
    }

    impl<B, C, D> TCFrameBuilder<Vacant, B, C, D> {
        /// Sets the Spacecraft ID (SCID), a required field.
        pub fn scid(self, scid: u16) -> TCFrameBuilder<u16, B, C, D> {
            TCFrameBuilder {
                scid,
                vcid: self.vcid,
                buffer: self.buffer,
                bypass_flag: self.bypass_flag,
                control_flag: self.control_flag,
                seq: self.seq,
                data_field_len: self.data_field_len,
            }
        }
    }

    impl<A, C, D> TCFrameBuilder<A, Vacant, C, D> {
        /// Sets the Virtual Channel ID (VCID), a required field.
        pub fn vcid(self, vcid: u8) -> TCFrameBuilder<A, u8, C, D> {
            TCFrameBuilder {
                vcid,
                scid: self.scid,
                buffer: self.buffer,
                bypass_flag: self.bypass_flag,
                control_flag: self.control_flag,
                seq: self.seq,
                data_field_len: self.data_field_len,
            }
        }
    }

    impl<A, B, D> TCFrameBuilder<A, B, Vacant, D> {
        /// Sets the memory buffer where the frame will be written, a required field.
        pub fn buffer<'a>(self, buffer: &'a mut [u8]) -> TCFrameBuilder<A, B, &'a mut [u8], D> {
            TCFrameBuilder {
                buffer,
                scid: self.scid,
                vcid: self.vcid,
                bypass_flag: self.bypass_flag,
                control_flag: self.control_flag,
                seq: self.seq,
                data_field_len: self.data_field_len,
            }
        }
    }

    impl<A, B, C> TCFrameBuilder<A, B, C, Vacant> {
        /// Sets the length of the frame's data field in bytes, a required field.
        pub fn data_field_len(self, data_field_len: usize) -> TCFrameBuilder<A, B, C, usize> {
            TCFrameBuilder {
                data_field_len,
                scid: self.scid,
                vcid: self.vcid,
                buffer: self.buffer,
                bypass_flag: self.bypass_flag,
                control_flag: self.control_flag,
                seq: self.seq,
            }
        }
    }

    impl<'a> TCFrameBuilder<u16, u8, &'a mut [u8], usize> {
        /// Consumes the builder and constructs the `TCTransferFrame` header.
        ///
        /// This method is only available after all required fields (`scid`, `vcid`,
        /// `buffer`, and `data_field_len`) have been set.
        ///
        /// Returns a mutable view of the frame, ready for the user to write data
        /// into its data field.
        pub fn build(self) -> Result<&'a mut TCTransferFrame, BuildError> {
            if self.scid > 0x3FF {
                return Err(BuildError::InvalidScid(self.scid));
            }
            if self.vcid > 0x3F {
                return Err(BuildError::InvalidVcid(self.vcid));
            }
            if self.data_field_len > TCTransferFrame::MAX_DATA_FIELD_LEN {
                return Err(BuildError::DataTooLong(self.data_field_len));
            }

            let total_len = TCTransferFrame::HEADER_SIZE + self.data_field_len;
            if self.buffer.len() < total_len {
                return Err(BuildError::BufferTooSmall {
                    required: total_len,
                    provided: self.buffer.len(),
                });
            }

            let frame_buf = &mut self.buffer[..total_len];
            let frame = TCTransferFrame::mut_from_bytes(frame_buf).unwrap();

            let tfvn = 0u16; // Version 1 is '00'
            let word0 = (tfvn << 14)
                | ((self.bypass_flag as u16) << 13)
                | ((self.control_flag as u16) << 12)
                | (self.scid & 0x3FF);

            // Frame length field is Total Length - 1
            let frame_len_field = (total_len - 1) as u16;
            let word1 = ((self.vcid as u16) << 10) | (frame_len_field & 0x3FF);

            frame.header.word0.set(word0);
            frame.header.word1.set(word1);
            frame.header.seq = self.seq;

            Ok(frame)
        }
    }
}
