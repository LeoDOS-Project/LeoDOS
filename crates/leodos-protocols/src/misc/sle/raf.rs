//! RAF (Return All Frames) service PDUs.
//!
//! RAF is the SLE service for receiving downlink telemetry frames
//! from a ground station. The client binds to a service instance,
//! starts data transfer, and receives TM frames wrapped in
//! TransferData invocations.

use super::ber::{self, BerReader, BerWriter, Class, tags};
use super::isp1::Credentials;
use super::types::{BindResult, ServiceType, SleError, Time};

/// Maximum length of an initiator/responder identifier string.
const MAX_ID_LEN: usize = 64;

/// Requested frame quality for RAF Start.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RequestedFrameQuality {
    /// Only good (error-free) frames.
    GoodOnly = 0,
    /// Only erred frames.
    ErredOnly = 1,
    /// All frames regardless of quality.
    AllFrames = 2,
}

impl RequestedFrameQuality {
    /// Converts from an integer value.
    pub fn from_i64(v: i64) -> Result<Self, SleError> {
        match v {
            0 => Ok(Self::GoodOnly),
            1 => Ok(Self::ErredOnly),
            2 => Ok(Self::AllFrames),
            _ => Err(SleError::InvalidEnumValue),
        }
    }
}

/// RAF operation identifier tags (context-specific).
/// These are the top-level CHOICE tags in the RAF PDU.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RafOp {
    /// Bind invocation from user to provider.
    Bind = 0,
    /// Bind return from provider to user.
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
    /// Transfer buffer (contains data invocations).
    TransferBuffer = 8,
    /// Status report.
    StatusReport = 9,
}

/// RAF Bind invocation PDU.
#[derive(Clone, Debug)]
pub struct RafBindInvocation {
    /// Identifier of the initiator (client).
    pub initiator_id: [u8; MAX_ID_LEN],
    /// Length of initiator_id.
    pub initiator_id_len: usize,
    /// Identifier of the responder (ground station).
    pub responder_id: [u8; MAX_ID_LEN],
    /// Length of responder_id.
    pub responder_id_len: usize,
    /// Type of RAF service requested.
    pub service_type: ServiceType,
    /// Protocol version number.
    pub version: u16,
    /// Optional authentication credentials.
    pub credentials: Option<Credentials>,
}

impl RafBindInvocation {
    /// Creates a new bind invocation.
    pub fn new(
        initiator_id: &[u8],
        responder_id: &[u8],
        service_type: ServiceType,
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
            service_type,
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

    /// Encodes this PDU into `buf` using BER.
    /// Returns the number of bytes written.
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let mut w = BerWriter::new(buf);
        let outer = w.begin_context(
            RafOp::Bind as u8,
            true,
        )?;
        let seq = w.begin_sequence()?;

        // credentials (CHOICE: NULL or SEQUENCE)
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

    /// Decodes a Bind invocation from BER bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, SleError> {
        let mut r = BerReader::new(buf);

        // context tag [0] constructed
        let (tag, _len) = r.read_context_tag()?;
        if tag != RafOp::Bind as u8 {
            return Err(SleError::UnexpectedTag);
        }

        let _seq_len = r.read_sequence()?;

        // credentials
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
            if time_bytes.len() != 8 {
                return Err(SleError::Truncated);
            }
            time.copy_from_slice(time_bytes);
            let mut hash = [0u8; 20];
            if hash_bytes.len() != 20 {
                return Err(SleError::Truncated);
            }
            hash.copy_from_slice(hash_bytes);
            Some(Credentials { time, random, hash })
        };

        let initiator = r.read_octet_string()?;
        let responder = r.read_octet_string()?;
        let service_type =
            ServiceType::from_u8(r.read_enum()? as u8)?;
        let version = r.read_integer()? as u16;

        Self::new(
            initiator,
            responder,
            service_type,
            version,
            credentials,
        )
    }
}

/// RAF Bind return PDU.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RafBindReturn {
    /// Result of the bind operation.
    pub result: BindResult,
    /// Responder credentials (if authenticating).
    pub credentials: Option<Credentials>,
}

impl RafBindReturn {
    /// Encodes this PDU into `buf`.
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let mut w = BerWriter::new(buf);
        let outer = w.begin_context(
            RafOp::BindReturn as u8,
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

        w.write_enum(self.result as i64)?;

        w.end_sequence(seq)?;
        w.end_sequence(outer)?;
        Ok(w.len())
    }

    /// Decodes a Bind return from BER bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, SleError> {
        let mut r = BerReader::new(buf);

        let (tag, _len) = r.read_context_tag()?;
        if tag != RafOp::BindReturn as u8 {
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

        let result =
            BindResult::from_u8(r.read_enum()? as u8)?;

        Ok(Self {
            result,
            credentials,
        })
    }
}

/// RAF Start invocation PDU.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RafStartInvocation {
    /// Start time — None means "start from now".
    pub start_time: Option<Time>,
    /// Stop time — None means "until stopped".
    pub stop_time: Option<Time>,
    /// Requested frame quality filter.
    pub quality: RequestedFrameQuality,
    /// Optional authentication credentials.
    pub credentials: Option<Credentials>,
}

impl RafStartInvocation {
    /// Encodes this PDU into `buf`.
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let mut w = BerWriter::new(buf);
        let outer = w.begin_context(
            RafOp::Start as u8,
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

        // start time: context [0] or NULL
        match &self.start_time {
            None => w.write_null()?,
            Some(t) => w.write_octet_string(&t.cds)?,
        }

        // stop time: context [1] or NULL
        match &self.stop_time {
            None => w.write_null()?,
            Some(t) => w.write_octet_string(&t.cds)?,
        }

        w.write_enum(self.quality as i64)?;

        w.end_sequence(seq)?;
        w.end_sequence(outer)?;
        Ok(w.len())
    }

    /// Decodes a Start invocation from BER bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, SleError> {
        let mut r = BerReader::new(buf);

        let (tag, _len) = r.read_context_tag()?;
        if tag != RafOp::Start as u8 {
            return Err(SleError::UnexpectedTag);
        }

        let _seq_len = r.read_sequence()?;

        // credentials
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

        // start_time
        let (pt, _, _) = r.peek_tag()?;
        let start_time = if pt == tags::NULL {
            r.read_null()?;
            None
        } else {
            let bytes = r.read_octet_string()?;
            let (t, _) = Time::decode(bytes)?;
            Some(t)
        };

        // stop_time
        let (pt, _, _) = r.peek_tag()?;
        let stop_time = if pt == tags::NULL {
            r.read_null()?;
            None
        } else {
            let bytes = r.read_octet_string()?;
            let (t, _) = Time::decode(bytes)?;
            Some(t)
        };

        let quality =
            RequestedFrameQuality::from_i64(r.read_enum()?)?;

        Ok(Self {
            start_time,
            stop_time,
            quality,
            credentials,
        })
    }
}

/// A single TM frame delivery within a RAF transfer buffer.
#[derive(Clone, Debug)]
pub struct RafTransferDataInvocation {
    /// Earth receive time of the frame.
    pub earth_receive_time: Time,
    /// Data link continuity indicator (-1 if unknown).
    pub data_link_continuity: i16,
    /// The raw TM frame data. Stored inline, max 2048 bytes.
    frame_buf: [u8; Self::MAX_FRAME_LEN],
    /// Actual frame length.
    frame_len: usize,
}

impl RafTransferDataInvocation {
    /// Maximum supported frame size.
    pub const MAX_FRAME_LEN: usize = 2048;

    /// Creates a new transfer data invocation.
    pub fn new(
        earth_receive_time: Time,
        data_link_continuity: i16,
        frame: &[u8],
    ) -> Result<Self, SleError> {
        if frame.len() > Self::MAX_FRAME_LEN {
            return Err(SleError::TooLong);
        }
        let mut frame_buf = [0u8; Self::MAX_FRAME_LEN];
        frame_buf[..frame.len()].copy_from_slice(frame);
        Ok(Self {
            earth_receive_time,
            data_link_continuity,
            frame_buf,
            frame_len: frame.len(),
        })
    }

    /// Returns the TM frame data.
    pub fn frame(&self) -> &[u8] {
        &self.frame_buf[..self.frame_len]
    }

    /// Encodes this invocation into `buf`.
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let mut w = BerWriter::new(buf);
        let seq = w.begin_sequence()?;
        w.write_octet_string(&self.earth_receive_time.cds)?;
        w.write_integer(
            self.data_link_continuity as i64,
        )?;
        w.write_octet_string(self.frame())?;
        w.end_sequence(seq)?;
        Ok(w.len())
    }

    /// Decodes a transfer data invocation from BER bytes.
    pub fn decode(buf: &[u8]) -> Result<Self, SleError> {
        let mut r = BerReader::new(buf);
        let _seq_len = r.read_sequence()?;

        let time_bytes = r.read_octet_string()?;
        let (ert, _) = Time::decode(time_bytes)?;
        let continuity = r.read_integer()? as i16;
        let frame_data = r.read_octet_string()?;

        Self::new(ert, continuity, frame_data)
    }
}

/// A RAF transfer buffer containing one or more data invocations.
///
/// On the wire this is a SEQUENCE OF RafTransferDataInvocation.
/// We provide encode/decode for individual items; the caller
/// iterates the buffer.
pub struct RafTransferBuffer;

impl RafTransferBuffer {
    /// Decodes the outer SEQUENCE header and returns a sub-reader
    /// positioned at the first element. The caller should
    /// repeatedly call `RafTransferDataInvocation::decode` on
    /// sub-slices until the content is exhausted.
    pub fn decode_header(
        buf: &[u8],
    ) -> Result<(usize, usize), SleError> {
        let mut r = BerReader::new(buf);
        // context tag for TransferBuffer
        let (tag, _class, _, _consumed) =
            ber::decode_tag(&buf[r.pos()..])?;
        if tag != RafOp::TransferBuffer as u8 {
            return Err(SleError::UnexpectedTag);
        }
        r.read_tag()?;
        let content_len = r.read_length()?;
        Ok((content_len, r.pos()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_invocation_roundtrip() {
        let bind = RafBindInvocation::new(
            b"user1",
            b"gs-station",
            ServiceType::RafOnline,
            5,
            None,
        )
        .unwrap();

        let mut buf = [0u8; 256];
        let n = bind.encode(&mut buf).unwrap();

        let decoded =
            RafBindInvocation::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.initiator_id(), b"user1");
        assert_eq!(decoded.responder_id(), b"gs-station");
        assert_eq!(
            decoded.service_type,
            ServiceType::RafOnline,
        );
        assert_eq!(decoded.version, 5);
        assert!(decoded.credentials.is_none());
    }

    #[test]
    fn bind_return_roundtrip() {
        let ret = RafBindReturn {
            result: BindResult::Success,
            credentials: None,
        };
        let mut buf = [0u8; 64];
        let n = ret.encode(&mut buf).unwrap();

        let decoded =
            RafBindReturn::decode(&buf[..n]).unwrap();
        assert_eq!(decoded.result, BindResult::Success);
        assert!(decoded.credentials.is_none());
    }

    #[test]
    fn start_invocation_roundtrip() {
        let start = RafStartInvocation {
            start_time: None,
            stop_time: None,
            quality: RequestedFrameQuality::AllFrames,
            credentials: None,
        };
        let mut buf = [0u8; 128];
        let n = start.encode(&mut buf).unwrap();

        let decoded =
            RafStartInvocation::decode(&buf[..n]).unwrap();
        assert!(decoded.start_time.is_none());
        assert!(decoded.stop_time.is_none());
        assert_eq!(
            decoded.quality,
            RequestedFrameQuality::AllFrames,
        );
    }

    #[test]
    fn transfer_data_roundtrip() {
        let frame_data = [0xDE, 0xAD, 0xBE, 0xEF];
        let td = RafTransferDataInvocation::new(
            Time::from_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
            -1,
            &frame_data,
        )
        .unwrap();

        let mut buf = [0u8; 128];
        let n = td.encode(&mut buf).unwrap();

        let decoded =
            RafTransferDataInvocation::decode(&buf[..n])
                .unwrap();
        assert_eq!(decoded.frame(), &frame_data);
        assert_eq!(decoded.data_link_continuity, -1);
        assert_eq!(
            decoded.earth_receive_time,
            Time::from_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
        );
    }
}
