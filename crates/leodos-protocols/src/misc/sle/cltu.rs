//! CLTU (Forward Command Link Transmission Unit) service PDUs.
//!
//! CLTU is the SLE service for sending uplink command frames
//! through a ground station. The client binds, starts the
//! service, then sends CLTU data which the ground station
//! radiates to the spacecraft.

use super::ber::{BerReader, BerWriter, Class, tags};
use super::isp1::Credentials;
use super::types::{ServiceType, SleError};

/// Maximum length of an initiator/responder identifier string.
const MAX_ID_LEN: usize = 64;

/// Maximum CLTU data size.
const MAX_CLTU_DATA: usize = 2048;

/// CLTU operation tags (context-specific CHOICE tags).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CltuOp {
    /// Bind invocation.
    Bind = 0,
    /// Bind return.
    BindReturn = 1,
    /// Unbind invocation.
    Unbind = 2,
    /// Unbind return.
    UnbindReturn = 3,
    /// Start invocation.
    Start = 4,
    /// Start return.
    StartReturn = 5,
    /// Stop invocation.
    Stop = 6,
    /// Stop return.
    StopReturn = 7,
    /// Transfer data invocation.
    TransferData = 8,
    /// Transfer data return.
    TransferDataReturn = 9,
}

/// Result of CLTU radiation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CltuStatus {
    /// CLTU was successfully radiated.
    Radiated = 0,
    /// CLTU expired before it could be radiated.
    Expired = 1,
    /// Radiation was interrupted.
    Interrupted = 2,
    /// Production has not been started.
    ProductionNotStarted = 3,
}

impl CltuStatus {
    /// Converts from an integer value.
    pub fn from_i64(v: i64) -> Result<Self, SleError> {
        match v {
            0 => Ok(Self::Radiated),
            1 => Ok(Self::Expired),
            2 => Ok(Self::Interrupted),
            3 => Ok(Self::ProductionNotStarted),
            _ => Err(SleError::InvalidEnumValue),
        }
    }
}

/// CLTU Bind invocation PDU.
#[derive(Clone, Debug)]
pub struct CltuBindInvocation {
    /// Identifier of the initiator (client).
    pub initiator_id: [u8; MAX_ID_LEN],
    /// Length of initiator_id.
    pub initiator_id_len: usize,
    /// Identifier of the responder (ground station).
    pub responder_id: [u8; MAX_ID_LEN],
    /// Length of responder_id.
    pub responder_id_len: usize,
    /// Service type (should be FCltu).
    pub service_type: ServiceType,
    /// Protocol version number.
    pub version: u16,
    /// Optional authentication credentials.
    pub credentials: Option<Credentials>,
}

impl CltuBindInvocation {
    /// Creates a new CLTU bind invocation.
    pub fn new(
        initiator_id: &[u8],
        responder_id: &[u8],
        version: u16,
        credentials: Option<Credentials>,
    ) -> Result<Self, SleError> {
        if initiator_id.len() > MAX_ID_LEN
            || responder_id.len() > MAX_ID_LEN
        {
            return Err(SleError::TooLong);
        }
        let mut init = [0u8; MAX_ID_LEN];
        init[..initiator_id.len()]
            .copy_from_slice(initiator_id);
        let mut resp = [0u8; MAX_ID_LEN];
        resp[..responder_id.len()]
            .copy_from_slice(responder_id);
        Ok(Self {
            initiator_id: init,
            initiator_id_len: initiator_id.len(),
            responder_id: resp,
            responder_id_len: responder_id.len(),
            service_type: ServiceType::FCltu,
            version,
            credentials,
        })
    }

    /// Returns the initiator identifier as a byte slice.
    pub fn initiator_id(&self) -> &[u8] {
        &self.initiator_id[..self.initiator_id_len]
    }

    /// Returns the responder identifier as a byte slice.
    pub fn responder_id(&self) -> &[u8] {
        &self.responder_id[..self.responder_id_len]
    }

    /// Encodes this PDU into `buf`.
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let mut w = BerWriter::new(buf);
        let outer = w.begin_context(
            CltuOp::Bind as u8,
            true,
        )?;
        let seq = w.begin_sequence()?;

        match &self.credentials {
            None => w.write_null()?,
            Some(cred) => {
                let cred_seq = w.begin_sequence()?;
                w.write_octet_string(&cred.time)?;
                w.write_integer(cred.random as i64)?;
                w.write_octet_string(&cred.hash)?;
                w.end_sequence(cred_seq)?;
            }
        }

        w.write_octet_string(self.initiator_id())?;
        w.write_octet_string(self.responder_id())?;
        w.write_enum(self.service_type as i64)?;
        w.write_integer(self.version as i64)?;

        w.end_sequence(seq)?;
        w.end_sequence(outer)?;
        Ok(w.len())
    }

    /// Decodes a CLTU Bind invocation from BER bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, SleError> {
        let mut r = BerReader::new(buf);

        let (tag, _len) = r.read_context_tag()?;
        if tag != CltuOp::Bind as u8 {
            return Err(SleError::UnexpectedTag);
        }

        let _seq_len = r.read_sequence()?;

        let (peek_tag, peek_class, _) = r.peek_tag()?;
        let credentials = if peek_tag == tags::NULL
            && peek_class == Class::Universal
        {
            r.read_null()?;
            None
        } else {
            let _cred_len = r.read_sequence()?;
            let time_bytes = r.read_octet_string()?;
            let random = r.read_integer()? as u32;
            let hash_bytes = r.read_octet_string()?;
            let mut time = [0u8; 8];
            time.copy_from_slice(
                time_bytes.get(..8).ok_or(SleError::Truncated)?,
            );
            let mut hash = [0u8; 20];
            hash.copy_from_slice(
                hash_bytes
                    .get(..20)
                    .ok_or(SleError::Truncated)?,
            );
            Some(Credentials { time, random, hash })
        };

        let initiator = r.read_octet_string()?;
        let responder = r.read_octet_string()?;
        let _service_type =
            ServiceType::from_u8(r.read_enum()? as u8)?;
        let version = r.read_integer()? as u16;

        Self::new(initiator, responder, version, credentials)
    }
}

/// CLTU Start invocation PDU.
///
/// Starts CLTU production at the ground station so CLTUs
/// can be sent for radiation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CltuStartInvocation {
    /// Optional authentication credentials.
    pub credentials: Option<Credentials>,
    /// Invocation identifier (sequence number).
    pub invoke_id: u16,
    /// First CLTU identifier to be accepted.
    pub first_cltu_id: u32,
}

impl CltuStartInvocation {
    /// Encodes this PDU into `buf`.
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let mut w = BerWriter::new(buf);
        let outer = w.begin_context(
            CltuOp::Start as u8,
            true,
        )?;
        let seq = w.begin_sequence()?;

        match &self.credentials {
            None => w.write_null()?,
            Some(cred) => {
                let cred_seq = w.begin_sequence()?;
                w.write_octet_string(&cred.time)?;
                w.write_integer(cred.random as i64)?;
                w.write_octet_string(&cred.hash)?;
                w.end_sequence(cred_seq)?;
            }
        }

        w.write_integer(self.invoke_id as i64)?;
        w.write_integer(self.first_cltu_id as i64)?;

        w.end_sequence(seq)?;
        w.end_sequence(outer)?;
        Ok(w.len())
    }

    /// Decodes a CLTU Start invocation from BER bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, SleError> {
        let mut r = BerReader::new(buf);

        let (tag, _len) = r.read_context_tag()?;
        if tag != CltuOp::Start as u8 {
            return Err(SleError::UnexpectedTag);
        }

        let _seq_len = r.read_sequence()?;

        let (peek_tag, peek_class, _) = r.peek_tag()?;
        let credentials = if peek_tag == tags::NULL
            && peek_class == Class::Universal
        {
            r.read_null()?;
            None
        } else {
            let _cred_len = r.read_sequence()?;
            let time_bytes = r.read_octet_string()?;
            let random = r.read_integer()? as u32;
            let hash_bytes = r.read_octet_string()?;
            let mut time = [0u8; 8];
            time.copy_from_slice(
                time_bytes.get(..8).ok_or(SleError::Truncated)?,
            );
            let mut hash = [0u8; 20];
            hash.copy_from_slice(
                hash_bytes
                    .get(..20)
                    .ok_or(SleError::Truncated)?,
            );
            Some(Credentials { time, random, hash })
        };

        let invoke_id = r.read_integer()? as u16;
        let first_cltu_id = r.read_integer()? as u32;

        Ok(Self {
            credentials,
            invoke_id,
            first_cltu_id,
        })
    }
}

/// CLTU Transfer Data invocation — sends a CLTU for radiation.
#[derive(Clone, Debug)]
pub struct CltuTransferDataInvocation {
    /// CLTU identifier (sequence number).
    pub cltu_id: u32,
    /// The CLTU data to be radiated.
    data_buf: [u8; MAX_CLTU_DATA],
    /// Actual data length.
    data_len: usize,
    /// Optional authentication credentials.
    pub credentials: Option<Credentials>,
}

impl CltuTransferDataInvocation {
    /// Creates a new CLTU transfer data invocation.
    pub fn new(
        cltu_id: u32,
        data: &[u8],
        credentials: Option<Credentials>,
    ) -> Result<Self, SleError> {
        if data.len() > MAX_CLTU_DATA {
            return Err(SleError::TooLong);
        }
        let mut data_buf = [0u8; MAX_CLTU_DATA];
        data_buf[..data.len()].copy_from_slice(data);
        Ok(Self {
            cltu_id,
            data_buf,
            data_len: data.len(),
            credentials,
        })
    }

    /// Returns the CLTU data.
    pub fn data(&self) -> &[u8] {
        &self.data_buf[..self.data_len]
    }

    /// Encodes this PDU into `buf`.
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let mut w = BerWriter::new(buf);
        let outer = w.begin_context(
            CltuOp::TransferData as u8,
            true,
        )?;
        let seq = w.begin_sequence()?;

        match &self.credentials {
            None => w.write_null()?,
            Some(cred) => {
                let cred_seq = w.begin_sequence()?;
                w.write_octet_string(&cred.time)?;
                w.write_integer(cred.random as i64)?;
                w.write_octet_string(&cred.hash)?;
                w.end_sequence(cred_seq)?;
            }
        }

        w.write_integer(self.cltu_id as i64)?;
        w.write_octet_string(self.data())?;

        w.end_sequence(seq)?;
        w.end_sequence(outer)?;
        Ok(w.len())
    }

    /// Decodes a CLTU transfer data invocation from BER bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, SleError> {
        let mut r = BerReader::new(buf);

        let (tag, _len) = r.read_context_tag()?;
        if tag != CltuOp::TransferData as u8 {
            return Err(SleError::UnexpectedTag);
        }

        let _seq_len = r.read_sequence()?;

        let (peek_tag, peek_class, _) = r.peek_tag()?;
        let credentials = if peek_tag == tags::NULL
            && peek_class == Class::Universal
        {
            r.read_null()?;
            None
        } else {
            let _cred_len = r.read_sequence()?;
            let time_bytes = r.read_octet_string()?;
            let random = r.read_integer()? as u32;
            let hash_bytes = r.read_octet_string()?;
            let mut time = [0u8; 8];
            time.copy_from_slice(
                time_bytes.get(..8).ok_or(SleError::Truncated)?,
            );
            let mut hash = [0u8; 20];
            hash.copy_from_slice(
                hash_bytes
                    .get(..20)
                    .ok_or(SleError::Truncated)?,
            );
            Some(Credentials { time, random, hash })
        };

        let cltu_id = r.read_integer()? as u32;
        let data = r.read_octet_string()?;

        Self::new(cltu_id, data, credentials)
    }
}

/// CLTU Transfer Data return — acknowledgement from provider.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CltuTransferDataReturn {
    /// CLTU identifier this return refers to.
    pub cltu_id: u32,
    /// Status of the CLTU.
    pub status: CltuStatus,
    /// Optional authentication credentials.
    pub credentials: Option<Credentials>,
}

impl CltuTransferDataReturn {
    /// Encodes this PDU into `buf`.
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let mut w = BerWriter::new(buf);
        let outer = w.begin_context(
            CltuOp::TransferDataReturn as u8,
            true,
        )?;
        let seq = w.begin_sequence()?;

        match &self.credentials {
            None => w.write_null()?,
            Some(cred) => {
                let cred_seq = w.begin_sequence()?;
                w.write_octet_string(&cred.time)?;
                w.write_integer(cred.random as i64)?;
                w.write_octet_string(&cred.hash)?;
                w.end_sequence(cred_seq)?;
            }
        }

        w.write_integer(self.cltu_id as i64)?;
        w.write_enum(self.status as i64)?;

        w.end_sequence(seq)?;
        w.end_sequence(outer)?;
        Ok(w.len())
    }

    /// Decodes a CLTU transfer data return from BER bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, SleError> {
        let mut r = BerReader::new(buf);

        let (tag, _len) = r.read_context_tag()?;
        if tag != CltuOp::TransferDataReturn as u8 {
            return Err(SleError::UnexpectedTag);
        }

        let _seq_len = r.read_sequence()?;

        let (peek_tag, peek_class, _) = r.peek_tag()?;
        let credentials = if peek_tag == tags::NULL
            && peek_class == Class::Universal
        {
            r.read_null()?;
            None
        } else {
            let _cred_len = r.read_sequence()?;
            let time_bytes = r.read_octet_string()?;
            let random = r.read_integer()? as u32;
            let hash_bytes = r.read_octet_string()?;
            let mut time = [0u8; 8];
            time.copy_from_slice(
                time_bytes.get(..8).ok_or(SleError::Truncated)?,
            );
            let mut hash = [0u8; 20];
            hash.copy_from_slice(
                hash_bytes
                    .get(..20)
                    .ok_or(SleError::Truncated)?,
            );
            Some(Credentials { time, random, hash })
        };

        let cltu_id = r.read_integer()? as u32;
        let status =
            CltuStatus::from_i64(r.read_enum()?)?;

        Ok(Self {
            cltu_id,
            status,
            credentials,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cltu_bind_roundtrip() {
        let bind = CltuBindInvocation::new(
            b"operator",
            b"antenna-1",
            3,
            None,
        )
        .unwrap();

        let mut buf = [0u8; 256];
        let n = bind.encode(&mut buf).unwrap();

        let decoded =
            CltuBindInvocation::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.initiator_id(), b"operator");
        assert_eq!(decoded.responder_id(), b"antenna-1");
        assert_eq!(decoded.service_type, ServiceType::FCltu);
        assert_eq!(decoded.version, 3);
        assert!(decoded.credentials.is_none());
    }

    #[test]
    fn cltu_start_roundtrip() {
        let start = CltuStartInvocation {
            credentials: None,
            invoke_id: 1,
            first_cltu_id: 100,
        };

        let mut buf = [0u8; 128];
        let n = start.encode(&mut buf).unwrap();

        let decoded =
            CltuStartInvocation::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.invoke_id, 1);
        assert_eq!(decoded.first_cltu_id, 100);
        assert!(decoded.credentials.is_none());
    }

    #[test]
    fn cltu_transfer_data_roundtrip() {
        let cltu_data = [0x01, 0x02, 0x03, 0x04, 0x05];
        let td = CltuTransferDataInvocation::new(
            42,
            &cltu_data,
            None,
        )
        .unwrap();

        let mut buf = [0u8; 128];
        let n = td.encode(&mut buf).unwrap();

        let decoded =
            CltuTransferDataInvocation::decode(&buf[..n])
                .unwrap();
        assert_eq!(decoded.cltu_id, 42);
        assert_eq!(decoded.data(), &cltu_data);
        assert!(decoded.credentials.is_none());
    }

    #[test]
    fn cltu_transfer_data_return_roundtrip() {
        let ret = CltuTransferDataReturn {
            cltu_id: 42,
            status: CltuStatus::Radiated,
            credentials: None,
        };

        let mut buf = [0u8; 64];
        let n = ret.encode(&mut buf).unwrap();

        let decoded =
            CltuTransferDataReturn::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.cltu_id, 42);
        assert_eq!(decoded.status, CltuStatus::Radiated);
        assert!(decoded.credentials.is_none());
    }
}
