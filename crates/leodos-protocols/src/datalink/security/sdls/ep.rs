//! SDLS Extended Procedures PDU format (CCSDS 355.1-B-1).
//!
//! Extended Procedures are administrative commands carried in
//! TC frames on a dedicated MAP/VCID. Each PDU uses a TLV header
//! (8-bit tag + 16-bit length) that identifies the service group
//! and procedure.

use super::Error;

/// Service groups (2 bits) defined by SDLS-EP (Section 5.3.2.2.3).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ServiceGroup {
    /// Key Management (OTAR, activation, verification, etc.).
    KeyManagement = 0b00,
    /// SA Management: Initiator → Recipient direction.
    SaManagementIr = 0b01,
    /// SA Management: Recipient → Initiator direction.
    SaManagementRi = 0b10,
    /// Security Monitoring & Control.
    MonitoringControl = 0b11,
}

impl TryFrom<u8> for ServiceGroup {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0b00 => Ok(Self::KeyManagement),
            0b01 => Ok(Self::SaManagementIr),
            0b10 => Ok(Self::SaManagementRi),
            0b11 => Ok(Self::MonitoringControl),
            _ => Err(Error::InvalidProcedure),
        }
    }
}

/// Key Management procedures (Table 5-1, Service Group 00).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum KeyProcedure {
    /// Deliver encrypted session keys via master key.
    Otar = 0b0001,
    /// Transition key from PREACTIVE to ACTIVE.
    Activate = 0b0010,
    /// Deactivate a key.
    Deactivate = 0b0011,
    /// Challenge-response key verification.
    Verify = 0b0100,
    /// Cryptographically destroy a key.
    Destroy = 0b0110,
    /// Query available keys.
    Inventory = 0b0111,
}

impl TryFrom<u8> for KeyProcedure {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0b0001 => Ok(Self::Otar),
            0b0010 => Ok(Self::Activate),
            0b0011 => Ok(Self::Deactivate),
            0b0100 => Ok(Self::Verify),
            0b0110 => Ok(Self::Destroy),
            0b0111 => Ok(Self::Inventory),
            _ => Err(Error::InvalidProcedure),
        }
    }
}

/// SA Management procedures (Table 5-1, Service Group 01 or 10).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum SaProcedure {
    /// Create a new Security Association.
    Create = 0b0001,
    /// Delete an existing SA.
    Delete = 0b0100,
    /// Assign keys to an SA (rekey).
    Rekey = 0b0110,
    /// Activate an SA for operational use.
    Start = 0b1011,
    /// Deactivate an SA.
    Stop = 0b1110,
    /// Mark an SA for retirement.
    Expire = 0b1001,
    /// Set the Anti-Replay Sequence Number.
    SetArsn = 0b1010,
    /// Set the Anti-Replay Sequence Number Window.
    SetArsnWindow = 0b0101,
    /// Read the current ARSN.
    ReadArsn = 0b0000,
    /// Read SA status.
    Status = 0b1111,
}

impl TryFrom<u8> for SaProcedure {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0b0001 => Ok(Self::Create),
            0b0100 => Ok(Self::Delete),
            0b0110 => Ok(Self::Rekey),
            0b1011 => Ok(Self::Start),
            0b1110 => Ok(Self::Stop),
            0b1001 => Ok(Self::Expire),
            0b1010 => Ok(Self::SetArsn),
            0b0101 => Ok(Self::SetArsnWindow),
            0b0000 => Ok(Self::ReadArsn),
            0b1111 => Ok(Self::Status),
            _ => Err(Error::InvalidProcedure),
        }
    }
}

/// Monitoring & Control procedures (Table 5-1, Service Group 11).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum McProcedure {
    /// Ping the security function.
    Ping = 0b0001,
    /// Query log status.
    LogStatus = 0b0010,
    /// Dump the security log.
    DumpLog = 0b0011,
    /// Erase the security log.
    EraseLog = 0b0100,
    /// Trigger a self-test.
    SelfTest = 0b0101,
    /// Reset the alarm flag in the FSR.
    ResetAlarmFlag = 0b0111,
}

impl TryFrom<u8> for McProcedure {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0b0001 => Ok(Self::Ping),
            0b0010 => Ok(Self::LogStatus),
            0b0011 => Ok(Self::DumpLog),
            0b0100 => Ok(Self::EraseLog),
            0b0101 => Ok(Self::SelfTest),
            0b0111 => Ok(Self::ResetAlarmFlag),
            _ => Err(Error::InvalidProcedure),
        }
    }
}

/// PDU type: command (Initiator → Recipient) or reply.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PduType {
    /// Command from Initiator to Recipient.
    Command,
    /// Reply from Recipient to Initiator.
    Reply,
}

use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

#[rustfmt::skip]
mod bitmask {
    /// Procedure Type flag (1 bit): 0=command, 1=reply.
    pub const TYPE_MASK: u8 =          0b_1000_0000;
    /// User Flag (1 bit): 0=CCSDS, 1=user-defined.
    pub const USER_FLAG_MASK: u8 =     0b_0100_0000;
    /// Service Group (2 bits).
    pub const SERVICE_GROUP_MASK: u8 = 0b_0011_0000;
    /// Procedure Identification (4 bits).
    pub const PROCEDURE_ID_MASK: u8 =  0b_0000_1111;
}

/// PDU header size: 1-byte tag + 2-byte length = 3 bytes.
pub const EP_HEADER_SIZE: usize = 3;

/// Parsed SDLS-EP PDU header.
///
/// Wire format (3 bytes, per Figure 5-2):
///   byte 0 (Tag): type(1) | user_flag(1) | service_group(2) | procedure_id(4)
///   byte 1-2: data field length in bits (16-bit BE, octet-aligned)
#[derive(Debug, Copy, Clone)]
pub struct EpHeader {
    /// Command or reply.
    pub pdu_type: PduType,
    /// User-defined extension flag.
    pub user_flag: bool,
    /// Service group (2 bits).
    pub service_group: u8,
    /// Procedure identifier within the service group (4 bits).
    pub procedure_id: u8,
}

impl EpHeader {
    /// Parse an EP header from a byte buffer.
    ///
    /// Returns the header and the data field length in bytes.
    pub fn parse(bytes: &[u8]) -> Result<(Self, u16), Error> {
        if bytes.len() < EP_HEADER_SIZE {
            return Err(Error::FrameTooShort);
        }

        let tag = bytes[0];
        let pdu_type = match get_bits_u8(tag, bitmask::TYPE_MASK) {
            0 => PduType::Command,
            _ => PduType::Reply,
        };
        let user_flag = get_bits_u8(tag, bitmask::USER_FLAG_MASK) != 0;
        let service_group = get_bits_u8(tag, bitmask::SERVICE_GROUP_MASK);
        let procedure_id = get_bits_u8(tag, bitmask::PROCEDURE_ID_MASK);

        let length_bits = u16::from_be_bytes([bytes[1], bytes[2]]);
        let length_bytes = length_bits / 8;

        Ok((
            Self {
                pdu_type,
                user_flag,
                service_group,
                procedure_id,
            },
            length_bytes,
        ))
    }

    /// Encode the EP header into a buffer.
    ///
    /// `data_len_bytes` is the data field length in bytes; it is
    /// encoded as bits in the wire format.
    pub fn encode(&self, data_len_bytes: u16, out: &mut [u8]) -> Result<usize, Error> {
        if out.len() < EP_HEADER_SIZE {
            return Err(Error::BufferTooSmall {
                required: EP_HEADER_SIZE,
                available: out.len(),
            });
        }

        let mut tag = 0u8;
        let type_val = match self.pdu_type {
            PduType::Command => 0u8,
            PduType::Reply => 1u8,
        };
        set_bits_u8(&mut tag, bitmask::TYPE_MASK, type_val);
        set_bits_u8(&mut tag, bitmask::USER_FLAG_MASK, self.user_flag as u8);
        set_bits_u8(&mut tag, bitmask::SERVICE_GROUP_MASK, self.service_group);
        set_bits_u8(&mut tag, bitmask::PROCEDURE_ID_MASK, self.procedure_id);
        out[0] = tag;

        let length_bits = data_len_bytes * 8;
        let len_bytes = length_bits.to_be_bytes();
        out[1] = len_bytes[0];
        out[2] = len_bytes[1];

        Ok(EP_HEADER_SIZE)
    }
}

/// Frame Security Report (FSR) — 32-bit OCF carried in TM/AOS/USLP
/// frames to report security status (Section 4.2.2).
#[derive(Debug, Copy, Clone)]
pub struct FrameSecurityReport {
    /// Last SPI used by the receiving security function.
    pub last_spi: u16,
    /// 8 LSBs of the ARSN from the last received frame.
    pub arsn_lsb: u8,
    /// Alarm flag: at least one frame was rejected since last reset.
    pub alarm: bool,
    /// Bad Sequence Number flag for the last received frame.
    pub bad_sn: bool,
    /// Bad MAC flag for the last received frame.
    pub bad_mac: bool,
    /// Bad SA flag for the last received frame.
    pub bad_sa: bool,
}

impl FrameSecurityReport {
    /// FSR version number (3 bits, value `100` = version 1).
    pub const VERSION: u8 = 0b100;

    /// Encode the FSR into a 4-byte buffer.
    pub fn encode(&self, out: &mut [u8; 4]) {
        let mut byte0: u8 = 0;
        set_bits_u8(&mut byte0, 0b_1000_0000, 1);
        set_bits_u8(&mut byte0, 0b_0111_0000, Self::VERSION);
        set_bits_u8(&mut byte0, 0b_0000_1000, self.alarm as u8);
        set_bits_u8(&mut byte0, 0b_0000_0100, self.bad_sn as u8);
        set_bits_u8(&mut byte0, 0b_0000_0010, self.bad_mac as u8);
        set_bits_u8(&mut byte0, 0b_0000_0001, self.bad_sa as u8);
        out[0] = byte0;
        let spi_bytes = self.last_spi.to_be_bytes();
        out[1] = spi_bytes[0];
        out[2] = spi_bytes[1];
        out[3] = self.arsn_lsb;
    }

    /// Parse an FSR from a 4-byte buffer.
    pub fn parse(bytes: &[u8; 4]) -> Self {
        Self {
            alarm: get_bits_u8(bytes[0], 0b_0000_1000) != 0,
            bad_sn: get_bits_u8(bytes[0], 0b_0000_0100) != 0,
            bad_mac: get_bits_u8(bytes[0], 0b_0000_0010) != 0,
            bad_sa: get_bits_u8(bytes[0], 0b_0000_0001) != 0,
            last_spi: u16::from_be_bytes([bytes[1], bytes[2]]),
            arsn_lsb: bytes[3],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip_key_otar() {
        let hdr = EpHeader {
            pdu_type: PduType::Command,
            user_flag: false,
            service_group: ServiceGroup::KeyManagement as u8,
            procedure_id: KeyProcedure::Otar as u8,
        };
        let mut buf = [0u8; 8];
        hdr.encode(10, &mut buf).unwrap();

        let (parsed, length) = EpHeader::parse(&buf).unwrap();
        assert_eq!(parsed.pdu_type, PduType::Command);
        assert!(!parsed.user_flag);
        assert_eq!(parsed.service_group, ServiceGroup::KeyManagement as u8);
        assert_eq!(parsed.procedure_id, KeyProcedure::Otar as u8);
        assert_eq!(length, 10);
    }

    #[test]
    fn header_roundtrip_sa_start() {
        let hdr = EpHeader {
            pdu_type: PduType::Reply,
            user_flag: true,
            service_group: ServiceGroup::SaManagementIr as u8,
            procedure_id: SaProcedure::Start as u8,
        };
        let mut buf = [0u8; 8];
        hdr.encode(32, &mut buf).unwrap();

        let (parsed, length) = EpHeader::parse(&buf).unwrap();
        assert_eq!(parsed.pdu_type, PduType::Reply);
        assert!(parsed.user_flag);
        assert_eq!(parsed.service_group, ServiceGroup::SaManagementIr as u8);
        assert_eq!(parsed.procedure_id, SaProcedure::Start as u8);
        assert_eq!(length, 32);
    }

    #[test]
    fn header_size_is_3_bytes() {
        assert_eq!(EP_HEADER_SIZE, 3);
    }

    #[test]
    fn service_group_try_from() {
        assert_eq!(
            ServiceGroup::try_from(0b00).unwrap(),
            ServiceGroup::KeyManagement
        );
        assert_eq!(
            ServiceGroup::try_from(0b01).unwrap(),
            ServiceGroup::SaManagementIr
        );
        assert_eq!(
            ServiceGroup::try_from(0b10).unwrap(),
            ServiceGroup::SaManagementRi
        );
        assert_eq!(
            ServiceGroup::try_from(0b11).unwrap(),
            ServiceGroup::MonitoringControl
        );
    }

    #[test]
    fn mc_procedures() {
        assert_eq!(McProcedure::try_from(0b0001).unwrap(), McProcedure::Ping);
        assert_eq!(
            McProcedure::try_from(0b0111).unwrap(),
            McProcedure::ResetAlarmFlag
        );
        assert!(McProcedure::try_from(0b1111).is_err());
    }

    #[test]
    fn buffer_too_short() {
        assert!(EpHeader::parse(&[0u8; 2]).is_err());
    }

    #[test]
    fn fsr_roundtrip() {
        let fsr = FrameSecurityReport {
            last_spi: 42,
            arsn_lsb: 0xAB,
            alarm: true,
            bad_sn: false,
            bad_mac: true,
            bad_sa: false,
        };
        let mut buf = [0u8; 4];
        fsr.encode(&mut buf);

        let parsed = FrameSecurityReport::parse(&buf);
        assert_eq!(parsed.last_spi, 42);
        assert_eq!(parsed.arsn_lsb, 0xAB);
        assert!(parsed.alarm);
        assert!(!parsed.bad_sn);
        assert!(parsed.bad_mac);
        assert!(!parsed.bad_sa);
    }

    #[test]
    fn fsr_control_word_type_is_1() {
        let fsr = FrameSecurityReport {
            last_spi: 0,
            arsn_lsb: 0,
            alarm: false,
            bad_sn: false,
            bad_mac: false,
            bad_sa: false,
        };
        let mut buf = [0u8; 4];
        fsr.encode(&mut buf);
        assert_eq!(buf[0] & 0x80, 0x80);
    }
}
