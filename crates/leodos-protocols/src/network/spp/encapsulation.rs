//! CCSDS Encapsulation Packet Protocol (CCSDS 133.1-B-3)
//!
//! Encapsulation Packets provide a mechanism to carry non-CCSDS
//! protocol data (e.g., IP datagrams) over CCSDS space links. They
//! share the same Packet Version Number (PVN = 7, binary '111') space
//! as Space Packets (PVN = 0) but use a different header format.
//!
//! # Header Layout
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │ Packet Version Number (3 bits) = '111'               │
//! │ Protocol ID (4 bits)                                 │
//! │ Length of Length (2 bits)                             │
//! │ User Defined Field (4 bits)                          │
//! │ Protocol ID Extension (4 bits)                       │
//! │ CCSDS-defined field (1 bit)                          │
//! │ Packet Length (variable: 0, 1, 2, or 4 bytes)        │
//! └──────────────────────────────────────────────────────┘
//! ```
//!
//! The first two bytes are always present. The Packet Length field
//! is 0-4 bytes depending on the Length of Length field.

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian::U16;

use crate::utils::get_bits_u16;
use crate::utils::set_bits_u16;

/// The 2-byte mandatory portion of an Encapsulation Packet header.
#[repr(C)]
#[derive(
    FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable,
    Debug, Copy, Clone,
)]
pub struct EncapsulationHeader {
    /// PVN(3) | Protocol ID(4) | Length of Length(2) |
    /// User Defined(4) | Protocol ID Extension(4) | CCSDS Defined(1).
    fields: U16,
}

/// Bitmasks for the 16-bit encapsulation header.
#[rustfmt::skip]
pub mod bitmask {
    /// Packet Version Number (3 bits) — always 0b111 = 7.
    pub const PVN_MASK: u16          = 0b_1110_0000_0000_0000;
    /// Protocol ID (4 bits).
    pub const PROTOCOL_ID_MASK: u16  = 0b_0001_1110_0000_0000;
    /// Length of Length (2 bits).
    pub const LEN_OF_LEN_MASK: u16   = 0b_0000_0001_1000_0000;
    /// User Defined Field (4 bits).
    pub const USER_DEF_MASK: u16     = 0b_0000_0000_0111_1000;
    /// Protocol ID Extension (4 bits).
    pub const PID_EXT_MASK: u16      = 0b_0000_0000_0000_0111_u16 << 1;
    /// CCSDS Defined Field (1 bit).
    pub const CCSDS_DEF_MASK: u16    = 0b_0000_0000_0000_0001;
}

use bitmask::*;

/// Well-known Protocol ID values (CCSDS 133.1-B-3, Table 4-1).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ProtocolId {
    /// Idle packet (all zeros payload).
    Idle = 0b0000,
    /// Internet Protocol Extension (IPE).
    Ipe = 0b0001,
    /// CCSDS-defined protocol — type in extension field.
    CcsdsDefined = 0b0010,
    /// User-defined protocol (0b0111 = mission-specific).
    UserDefined = 0b0111,
    /// Internet Protocol version 4.
    Ipv4 = 0b1000,
    /// Internet Protocol version 6.
    Ipv6 = 0b1001,
}

/// Errors for Encapsulation Packet operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Error {
    /// Buffer too short for the header.
    BufferTooShort {
        /// Minimum bytes needed.
        required: usize,
        /// Actual bytes available.
        provided: usize,
    },
    /// Invalid Packet Version Number (not 0b111).
    InvalidPvn(u8),
}

/// Encapsulation Packet Version Number (binary '111' = 7).
pub const ENCAP_PVN: u8 = 0b111;

impl EncapsulationHeader {
    /// Minimum header size: the 2-byte mandatory portion.
    pub const MIN_SIZE: usize = 2;

    /// Returns the 3-bit Packet Version Number.
    pub fn pvn(&self) -> u8 {
        get_bits_u16(self.fields, PVN_MASK) as u8
    }
    /// Sets the Packet Version Number.
    pub fn set_pvn(&mut self, pvn: u8) {
        set_bits_u16(&mut self.fields, PVN_MASK, pvn as u16);
    }

    /// Returns the 4-bit Protocol ID.
    pub fn protocol_id(&self) -> u8 {
        get_bits_u16(self.fields, PROTOCOL_ID_MASK) as u8
    }
    /// Sets the 4-bit Protocol ID.
    pub fn set_protocol_id(&mut self, pid: u8) {
        set_bits_u16(
            &mut self.fields,
            PROTOCOL_ID_MASK,
            pid as u16,
        );
    }

    /// Returns the 2-bit Length of Length field.
    ///
    /// This determines how many bytes follow for the packet length:
    /// - 0b00: 0 bytes (length is implicit/undefined)
    /// - 0b01: 1 byte (max 255)
    /// - 0b10: 2 bytes (max 65535)
    /// - 0b11: 4 bytes (max 4294967295)
    pub fn len_of_len(&self) -> u8 {
        get_bits_u16(self.fields, LEN_OF_LEN_MASK) as u8
    }
    /// Sets the 2-bit Length of Length field.
    pub fn set_len_of_len(&mut self, lol: u8) {
        set_bits_u16(
            &mut self.fields,
            LEN_OF_LEN_MASK,
            lol as u16,
        );
    }

    /// Returns the 4-bit User Defined field.
    pub fn user_defined(&self) -> u8 {
        get_bits_u16(self.fields, USER_DEF_MASK) as u8
    }
    /// Sets the 4-bit User Defined field.
    pub fn set_user_defined(&mut self, ud: u8) {
        set_bits_u16(&mut self.fields, USER_DEF_MASK, ud as u16);
    }

    /// Returns the 4-bit Protocol ID Extension.
    pub fn protocol_id_extension(&self) -> u8 {
        get_bits_u16(self.fields, PID_EXT_MASK) as u8
    }
    /// Sets the 4-bit Protocol ID Extension.
    pub fn set_protocol_id_extension(&mut self, ext: u8) {
        set_bits_u16(&mut self.fields, PID_EXT_MASK, ext as u16);
    }

    /// Returns the 1-bit CCSDS Defined field.
    pub fn ccsds_defined(&self) -> bool {
        get_bits_u16(self.fields, CCSDS_DEF_MASK) != 0
    }
    /// Sets the 1-bit CCSDS Defined field.
    pub fn set_ccsds_defined(&mut self, val: bool) {
        set_bits_u16(
            &mut self.fields,
            CCSDS_DEF_MASK,
            u16::from(val),
        );
    }

    /// Returns the number of bytes used for the packet length field.
    pub fn packet_length_bytes(&self) -> usize {
        match self.len_of_len() {
            0b00 => 0,
            0b01 => 1,
            0b10 => 2,
            _ => 4,
        }
    }

    /// Returns the total header size (mandatory 2 bytes + length bytes).
    pub fn total_header_size(&self) -> usize {
        Self::MIN_SIZE + self.packet_length_bytes()
    }

    /// Parses from a byte slice.
    pub fn parse(bytes: &[u8]) -> Result<&Self, Error> {
        if bytes.len() < Self::MIN_SIZE {
            return Err(Error::BufferTooShort {
                required: Self::MIN_SIZE,
                provided: bytes.len(),
            });
        }
        let (hdr, _) = Self::ref_from_prefix(bytes).unwrap();
        if hdr.pvn() != ENCAP_PVN {
            return Err(Error::InvalidPvn(hdr.pvn()));
        }
        Ok(hdr)
    }
}

/// Reads the variable-length packet length from the bytes following
/// the 2-byte mandatory header.
///
/// `len_of_len` is the value from the header's Length of Length field.
/// `bytes` should start at the first byte after the mandatory header.
pub fn read_packet_length(
    len_of_len: u8,
    bytes: &[u8],
) -> Result<Option<u32>, Error> {
    match len_of_len {
        0b00 => Ok(None),
        0b01 => {
            if bytes.is_empty() {
                return Err(Error::BufferTooShort {
                    required: 1,
                    provided: 0,
                });
            }
            Ok(Some(bytes[0] as u32))
        }
        0b10 => {
            if bytes.len() < 2 {
                return Err(Error::BufferTooShort {
                    required: 2,
                    provided: bytes.len(),
                });
            }
            Ok(Some(
                u16::from_be_bytes([bytes[0], bytes[1]]) as u32,
            ))
        }
        _ => {
            if bytes.len() < 4 {
                return Err(Error::BufferTooShort {
                    required: 4,
                    provided: bytes.len(),
                });
            }
            Ok(Some(u32::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
            ])))
        }
    }
}

/// Writes the variable-length packet length after the mandatory header.
///
/// Returns the number of bytes written (0, 1, 2, or 4).
pub fn write_packet_length(
    len_of_len: u8,
    length: u32,
    bytes: &mut [u8],
) -> Result<usize, Error> {
    match len_of_len {
        0b00 => Ok(0),
        0b01 => {
            if bytes.is_empty() {
                return Err(Error::BufferTooShort {
                    required: 1,
                    provided: 0,
                });
            }
            bytes[0] = length as u8;
            Ok(1)
        }
        0b10 => {
            if bytes.len() < 2 {
                return Err(Error::BufferTooShort {
                    required: 2,
                    provided: bytes.len(),
                });
            }
            let b = (length as u16).to_be_bytes();
            bytes[0] = b[0];
            bytes[1] = b[1];
            Ok(2)
        }
        _ => {
            if bytes.len() < 4 {
                return Err(Error::BufferTooShort {
                    required: 4,
                    provided: bytes.len(),
                });
            }
            let b = length.to_be_bytes();
            bytes[..4].copy_from_slice(&b);
            Ok(4)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerocopy::IntoBytes;

    #[test]
    fn header_pvn_is_111() {
        let mut buf = [0u8; 2];
        let hdr =
            EncapsulationHeader::mut_from_bytes(&mut buf).unwrap();
        hdr.set_pvn(ENCAP_PVN);
        assert_eq!(hdr.pvn(), 0b111);
        // Top 3 bits of first byte should be 0b111 = 0xE0
        assert_eq!(buf[0] & 0xE0, 0xE0);
    }

    #[test]
    fn protocol_id_roundtrip() {
        let mut buf = [0u8; 2];
        let hdr =
            EncapsulationHeader::mut_from_bytes(&mut buf).unwrap();
        hdr.set_pvn(ENCAP_PVN);
        hdr.set_protocol_id(ProtocolId::Ipv4 as u8);
        assert_eq!(hdr.protocol_id(), ProtocolId::Ipv4 as u8);
    }

    #[test]
    fn len_of_len_values() {
        for lol in 0..=3u8 {
            let mut buf = [0u8; 2];
            let hdr = EncapsulationHeader::mut_from_bytes(&mut buf)
                .unwrap();
            hdr.set_pvn(ENCAP_PVN);
            hdr.set_len_of_len(lol);
            assert_eq!(hdr.len_of_len(), lol);
            let expected_bytes = match lol {
                0 => 0,
                1 => 1,
                2 => 2,
                _ => 4,
            };
            assert_eq!(hdr.packet_length_bytes(), expected_bytes);
        }
    }

    #[test]
    fn user_defined_field() {
        let mut buf = [0u8; 2];
        let hdr =
            EncapsulationHeader::mut_from_bytes(&mut buf).unwrap();
        hdr.set_pvn(ENCAP_PVN);
        hdr.set_user_defined(0x0F);
        assert_eq!(hdr.user_defined(), 0x0F);
    }

    #[test]
    fn parse_validates_pvn() {
        let buf = [0u8; 2]; // PVN = 0
        let err = EncapsulationHeader::parse(&buf);
        assert!(matches!(err, Err(Error::InvalidPvn(0))));
    }

    #[test]
    fn parse_valid() {
        let mut buf = [0u8; 2];
        let hdr =
            EncapsulationHeader::mut_from_bytes(&mut buf).unwrap();
        hdr.set_pvn(ENCAP_PVN);
        hdr.set_protocol_id(ProtocolId::Ipv6 as u8);
        hdr.set_len_of_len(0b10);

        let parsed = EncapsulationHeader::parse(&buf).unwrap();
        assert_eq!(parsed.pvn(), ENCAP_PVN);
        assert_eq!(parsed.protocol_id(), ProtocolId::Ipv6 as u8);
        assert_eq!(parsed.len_of_len(), 0b10);
    }

    #[test]
    fn read_write_packet_length_1byte() {
        let mut buf = [0u8; 4];
        let n = write_packet_length(0b01, 200, &mut buf).unwrap();
        assert_eq!(n, 1);
        let len = read_packet_length(0b01, &buf).unwrap();
        assert_eq!(len, Some(200));
    }

    #[test]
    fn read_write_packet_length_2byte() {
        let mut buf = [0u8; 4];
        let n =
            write_packet_length(0b10, 50000, &mut buf).unwrap();
        assert_eq!(n, 2);
        let len = read_packet_length(0b10, &buf).unwrap();
        assert_eq!(len, Some(50000));
    }

    #[test]
    fn read_write_packet_length_4byte() {
        let mut buf = [0u8; 4];
        let n = write_packet_length(0b11, 1_000_000, &mut buf)
            .unwrap();
        assert_eq!(n, 4);
        let len = read_packet_length(0b11, &buf).unwrap();
        assert_eq!(len, Some(1_000_000));
    }

    #[test]
    fn packet_length_zero_means_none() {
        let buf = [0u8; 4];
        let len = read_packet_length(0b00, &buf).unwrap();
        assert_eq!(len, None);
    }

    #[test]
    fn total_header_size() {
        let mut buf = [0u8; 2];
        let hdr =
            EncapsulationHeader::mut_from_bytes(&mut buf).unwrap();

        hdr.set_len_of_len(0b00);
        assert_eq!(hdr.total_header_size(), 2);

        hdr.set_len_of_len(0b01);
        assert_eq!(hdr.total_header_size(), 3);

        hdr.set_len_of_len(0b10);
        assert_eq!(hdr.total_header_size(), 4);

        hdr.set_len_of_len(0b11);
        assert_eq!(hdr.total_header_size(), 6);
    }
}
