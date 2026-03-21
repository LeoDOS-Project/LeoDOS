//! Shared SLE types used across RAF, CLTU, and ISP1.

/// CCSDS Day Segmented (CDS) time code.
///
/// 8 bytes: 2-byte day count (epoch 1958-01-01) + 4-byte ms of day
/// + 2-byte microseconds of millisecond.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Time {
    /// Raw 8-byte CDS time field.
    pub cds: [u8; 8],
}

impl Time {
    /// Size of a CDS time code in bytes.
    pub const SIZE: usize = 8;

    /// Creates a Time from raw bytes.
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self { cds: bytes }
    }

    /// Returns the day count since 1958-01-01.
    pub fn day(&self) -> u16 {
        u16::from_be_bytes([self.cds[0], self.cds[1]])
    }

    /// Returns the millisecond of the day.
    pub fn ms_of_day(&self) -> u32 {
        u32::from_be_bytes([
            self.cds[2], self.cds[3], self.cds[4], self.cds[5],
        ])
    }

    /// Returns the sub-millisecond microseconds.
    pub fn microseconds(&self) -> u16 {
        u16::from_be_bytes([self.cds[6], self.cds[7]])
    }

    /// Encodes the time into the provided buffer.
    /// Returns the number of bytes written (always 8).
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, SleError> {
        if buf.len() < Self::SIZE {
            return Err(SleError::BufferTooSmall);
        }
        buf[..Self::SIZE].copy_from_slice(&self.cds);
        Ok(Self::SIZE)
    }

    /// Decodes a Time from the provided buffer.
    /// Returns the Time and number of bytes consumed.
    pub fn decode(buf: &[u8]) -> Result<(Self, usize), SleError> {
        if buf.len() < Self::SIZE {
            return Err(SleError::BufferTooSmall);
        }
        let mut cds = [0u8; 8];
        cds.copy_from_slice(&buf[..Self::SIZE]);
        Ok((Self { cds }, Self::SIZE))
    }
}

/// SLE service types.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ServiceType {
    /// Return All Frames — online (real-time) delivery.
    RafOnline = 0,
    /// Return All Frames — offline (deferred) delivery.
    RafOffline = 1,
    /// Forward CLTU service.
    FCltu = 2,
}

impl ServiceType {
    /// Converts from an integer value.
    pub fn from_u8(v: u8) -> Result<Self, SleError> {
        match v {
            0 => Ok(Self::RafOnline),
            1 => Ok(Self::RafOffline),
            2 => Ok(Self::FCltu),
            _ => Err(SleError::InvalidEnumValue),
        }
    }
}

/// Result of a Bind operation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BindResult {
    /// Bind succeeded.
    Success = 0,
    /// Responder denied access (bad credentials).
    AccessDenied = 1,
    /// The requested service type is not available.
    ServiceTypeNotSupported = 2,
    /// The requested protocol version is not supported.
    VersionNotSupported = 3,
}

impl BindResult {
    /// Converts from an integer value.
    pub fn from_u8(v: u8) -> Result<Self, SleError> {
        match v {
            0 => Ok(Self::Success),
            1 => Ok(Self::AccessDenied),
            2 => Ok(Self::ServiceTypeNotSupported),
            3 => Ok(Self::VersionNotSupported),
            _ => Err(SleError::InvalidEnumValue),
        }
    }
}

/// Identifies a specific service instance on the provider.
///
/// In SLE, service instances are identified by an Object Identifier
/// (OID) path like `sagr=1.spack=1.rsl-fg=1.raf=onlt1`.
/// We store the raw bytes of the BER-encoded identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceInstanceId {
    /// Raw BER-encoded service instance identifier.
    /// Max 64 bytes should be enough for any real identifier.
    buf: [u8; 64],
    /// Number of valid bytes in `buf`.
    len: usize,
}

impl ServiceInstanceId {
    /// Maximum encoded length.
    pub const MAX_LEN: usize = 64;

    /// Creates a new ServiceInstanceId from raw bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, SleError> {
        if data.len() > Self::MAX_LEN {
            return Err(SleError::BufferTooSmall);
        }
        let mut buf = [0u8; Self::MAX_LEN];
        buf[..data.len()].copy_from_slice(data);
        Ok(Self {
            buf,
            len: data.len(),
        })
    }

    /// Returns the raw identifier bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Returns the length of the identifier.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the identifier is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Errors from SLE encoding/decoding.
#[derive(Copy, Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SleError {
    /// Output buffer is too small.
    #[error("buffer too small")]
    BufferTooSmall,
    /// Input data is truncated or malformed.
    #[error("truncated or malformed input")]
    Truncated,
    /// Invalid BER tag encountered.
    #[error("unexpected BER tag")]
    UnexpectedTag,
    /// Invalid enum discriminant.
    #[error("invalid enum value")]
    InvalidEnumValue,
    /// Integer value out of range.
    #[error("integer out of range")]
    IntegerOverflow,
    /// String or identifier exceeds maximum length.
    #[error("value too long")]
    TooLong,
    /// Missing required field in a PDU.
    #[error("missing required field")]
    MissingField,
}
