//! Advanced Orbiting Systems Space Datalink Protocol (AOS-SDLP)
//!
//! Spec: https://ccsds.org/Pubs/732x0b4.pdf
//!
//! AOS Frames are fixed-length frames optimized for high-speed, mixed-media data
//! (e.g., audio, video, and packets interleaved). They are the standard for
//! Space Station (ISS) and modern constellation Inter-Satellite Links.

use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian::U16;

use crate::coding::randomizer::Randomizer;
use crate::utils::get_bits_u8;
use crate::utils::get_bits_u16;
use crate::utils::set_bits_u8;
use crate::utils::set_bits_u16;

/// A zero-copy view over a CCSDS AOS Transfer Frame.
///
/// # Layout
///
/// ```text
/// +------------------------------------+----------+
/// | Field Name                         | Size     |
/// +------------------------------------+----------+
/// | -- Primary Header (6 bytes) ------ |          |
/// | Version Number (01)                | 2 bits   |
/// | Spacecraft ID                      | 8 bits   |
/// | Virtual Channel ID                 | 6 bits   |
/// | Virtual Channel Frame Count        | 24 bits  |
/// | Replay Flag                        | 1 bit    |
/// | Usage Flag (Spare)                 | 1 bit    |
/// | Spare                              | 6 bits   |
/// |                                    |          |
/// | -- Insert Zone (Optional) -------- | Variable |
/// | -- Data Field -------------------- | Variable |
/// | -- Trailer (Optional) ------------ |          |
/// | Frame Error Control (CRC)          | 2 bytes  |
/// +------------------------------------+----------+
/// ```
#[repr(C, packed)]
#[derive(IntoBytes, FromBytes, Unaligned, KnownLayout, Immutable)]
pub struct AosTransferFrame {
    pub header: AosPrimaryHeader,
    pub data_field: [u8],
}

/// The 6-byte Primary Header of an AOS Frame.
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Debug, Copy, Clone)]
pub struct AosPrimaryHeader {
    version_scid_vcid_field: U16,
    vc_frame_count: [u8; 3],
    replay_usage_spare_field: u8,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BuildError {
    InvalidScid(u16),
    InvalidVcid(u8),
    BufferTooSmall { required: usize, provided: usize },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParseError {
    TooShortForHeader,
    InvalidVersion(u8),
}

#[rustfmt::skip]
pub mod bitmasks {
    pub const VERSION_MASK: u16    = 0b_11000000_00000000;
    pub const SCID_MASK: u16       = 0b_00111111_11000000;
    pub const VCID_MASK: u16       = 0b_00000000_00111111;

    pub const REPLAY_FLAG_MASK: u8 = 0b_10000000;
    pub const USAGE_FLAG_MASK: u8  = 0b_01000000;
    pub const _SPARE_MASK: u8      = 0b_00111111;
}

use bitmasks::*;

#[bon]
impl AosTransferFrame {
    pub const AOS_VERSION: u8 = 0b01;

    /// Parses a raw byte slice into a zero-copy AOS Frame view.
    ///
    /// Optionally applies de-randomization if a randomizer is provided.
    pub fn parse<'a>(
        bytes: &[u8],
        output_buffer: &'a mut [u8],
        randomizer: Option<&impl Randomizer>,
    ) -> Result<&'a AosTransferFrame, ParseError> {
        if bytes.len() < size_of::<AosPrimaryHeader>() {
            return Err(ParseError::TooShortForHeader);
        }
        if output_buffer.len() < bytes.len() {
            // In a real implementation, handle this gracefully
            return Err(ParseError::TooShortForHeader);
        }

        // Copy input to output
        let frame_buf = &mut output_buffer[..bytes.len()];
        frame_buf.copy_from_slice(bytes);

        // De-randomize in place if needed
        if let Some(r) = randomizer {
            r.apply(frame_buf);
        }

        // Cast
        let frame = AosTransferFrame::ref_from_bytes(frame_buf)
            .map_err(|_| ParseError::TooShortForHeader)?;

        // Validate Version
        if frame.header.version() != Self::AOS_VERSION {
            return Err(ParseError::InvalidVersion(frame.header.version()));
        }

        Ok(frame)
    }

    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        scid: u8,
        vcid: u8,
        vc_frame_count: u32,
        replay_flag: bool,
        usage_flag: bool,
        payload: &'a [u8],
    ) -> Result<&'a mut Self, BuildError> {
        let total_len = size_of::<AosPrimaryHeader>() + payload.len();
        if buffer.len() < total_len {
            return Err(BuildError::BufferTooSmall {
                required: total_len,
                provided: buffer.len(),
            });
        }
        if vcid > 0x3F {
            return Err(BuildError::InvalidVcid(vcid));
        }

        let frame = AosTransferFrame::mut_from_bytes(&mut buffer[..total_len]).unwrap();

        frame.header.set_version(Self::AOS_VERSION);
        frame.header.set_scid(scid);
        frame.header.set_vcid(vcid);
        frame.header.set_vc_frame_count(vc_frame_count);
        frame.header.set_replay(replay_flag);
        frame.header.set_usage_flag(usage_flag);

        Ok(frame)
    }

    pub fn header(&self) -> &AosPrimaryHeader {
        &self.header
    }

    pub fn data(&self) -> &[u8] {
        &self.data_field
    }
}

impl AosPrimaryHeader {
    pub fn version(&self) -> u8 {
        get_bits_u16(self.version_scid_vcid_field, VERSION_MASK) as u8
    }
    pub fn set_version(&mut self, version: u8) {
        set_bits_u16(
            &mut self.version_scid_vcid_field,
            VERSION_MASK,
            version as u16,
        );
    }

    pub fn scid(&self) -> u8 {
        get_bits_u16(self.version_scid_vcid_field, SCID_MASK) as u8
    }
    pub fn set_scid(&mut self, scid: u8) {
        set_bits_u16(&mut self.version_scid_vcid_field, SCID_MASK, scid as u16);
    }

    pub fn vcid(&self) -> u8 {
        get_bits_u16(self.version_scid_vcid_field, VCID_MASK) as u8
    }
    pub fn set_vcid(&mut self, vcid: u8) {
        set_bits_u16(&mut self.version_scid_vcid_field, VCID_MASK, vcid as u16);
    }

    pub fn vc_frame_count(&self) -> u32 {
        let b = self.vc_frame_count;
        u32::from_be_bytes([0, b[0], b[1], b[2]])
    }
    pub fn set_vc_frame_count(&mut self, count: u32) {
        let bytes = count.to_be_bytes();
        self.vc_frame_count.copy_from_slice(&bytes[1..4]);
    }

    pub fn is_replay(&self) -> bool {
        get_bits_u8(self.replay_usage_spare_field, REPLAY_FLAG_MASK) != 0
    }
    pub fn set_replay(&mut self, replay: bool) {
        set_bits_u8(
            &mut self.replay_usage_spare_field,
            REPLAY_FLAG_MASK,
            if replay { 1 } else { 0 },
        );
    }

    pub fn usage_flag(&self) -> bool {
        get_bits_u8(self.replay_usage_spare_field, USAGE_FLAG_MASK) != 0
    }
    pub fn set_usage_flag(&mut self, usage: bool) {
        set_bits_u8(
            &mut self.replay_usage_spare_field,
            USAGE_FLAG_MASK,
            if usage { 1 } else { 0 },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aos_builder_and_parser() {
        let mut buf = [0u8; 1024];
        let payload = [0xAA, 0xBB, 0xCC];

        let frame = AosTransferFrame::builder()
            .buffer(&mut buf)
            .scid(0x12)
            .vcid(0x3F) // Max VCID
            .vc_frame_count(0x123456)
            .replay_flag(true)
            .usage_flag(false)
            .payload(&payload)
            .build()
            .unwrap();

        assert_eq!(frame.header.scid(), 0x12);
        assert_eq!(frame.header.vcid(), 0x3F);
        assert_eq!(frame.header.vc_frame_count(), 0x123456);
        assert!(frame.header.is_replay());
        assert_eq!(frame.data(), &payload);
    }
}
