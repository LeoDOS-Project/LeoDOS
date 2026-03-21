//! CCSDS Unsegmented Code (CUC) Time Format (CCSDS 301.0-B-4, §3.2)
//!
//! CUC encodes time as a binary count of whole seconds (coarse) and
//! fractional seconds (fine) since a reference epoch. The number of
//! coarse and fine bytes is configurable (1-4 each), making it
//! flexible for different precision requirements.
//!
//! # P-Field (Preamble)
//!
//! The P-field is a 1-byte or 2-byte preamble that describes the
//! time code format. When the P-field is implicit (agreed upon by
//! sender and receiver), it may be omitted from the encoded bytes.
//!
//! ```text
//! Byte 1:
//!   [7]     Extension flag (0 = 1-byte P-field, 1 = 2-byte)
//!   [6:4]   Time code ID: 001 = CUC with agency-defined epoch
//!                          010 = CUC with CCSDS epoch (TAI)
//!   [3:2]   Number of coarse time octets - 1 (0-3 → 1-4 bytes)
//!   [1:0]   Number of fine time octets (0-3 bytes)
//!
//! Byte 2 (if extension flag = 1):
//!   [7]     Must be 0
//!   [6:5]   Additional coarse octets (0-3)
//!   [4:3]   Additional fine octets (0-3)
//!   [2:0]   Reserved
//! ```
//!
//! # CCSDS Epoch
//!
//! The standard CCSDS epoch is **1958-01-01T00:00:00 TAI**.

use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

/// Maximum number of coarse (whole-second) bytes.
pub const MAX_COARSE_BYTES: u8 = 4;

/// Maximum number of fine (fractional-second) bytes.
pub const MAX_FINE_BYTES: u8 = 3;

/// The CCSDS epoch: 1958-01-01T00:00:00 TAI.
/// Offset from the Unix epoch (1970-01-01) in seconds.
/// Unix epoch = CCSDS epoch + 378_691_200 seconds.
pub const CCSDS_EPOCH_UNIX_OFFSET: i64 = -378_691_200;

/// Time code IDs for the P-field.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum TimeCodeId {
    /// CUC with agency-defined epoch (ID = 0b001).
    AgencyEpoch = 0b001,
    /// CUC with CCSDS epoch (1958-01-01 TAI) (ID = 0b010).
    CcsdsEpoch = 0b010,
}

/// Bitmasks for CUC P-field byte fields.
#[rustfmt::skip]
mod bitmask {
    /// Bitmask for the 3-bit time code ID field [6:4].
    pub const TIME_CODE_ID_MASK: u8 = 0b_0111_0000;
    /// Bitmask for the 2-bit coarse length code field [3:2].
    pub const COARSE_LEN_MASK: u8 =   0b_0000_1100;
    /// Bitmask for the 2-bit fine length code field [1:0].
    pub const FINE_LEN_MASK: u8 =      0b_0000_0011;
}

use bitmask::*;

/// Configuration for a CUC time code.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CucConfig {
    /// Time code identifier.
    pub time_code_id: TimeCodeId,
    /// Number of coarse (whole-second) bytes (1-4).
    pub coarse_len: u8,
    /// Number of fine (fractional-second) bytes (0-3).
    pub fine_len: u8,
}

impl CucConfig {
    /// Creates a new CUC configuration.
    ///
    /// Panics if `coarse_len` is 0 or > 4, or `fine_len` > 3.
    pub const fn new(time_code_id: TimeCodeId, coarse_len: u8, fine_len: u8) -> Self {
        assert!(coarse_len >= 1 && coarse_len <= MAX_COARSE_BYTES);
        assert!(fine_len <= MAX_FINE_BYTES);
        Self {
            time_code_id,
            coarse_len,
            fine_len,
        }
    }

    /// Standard 4+2 configuration: 4 coarse bytes (seconds since
    /// CCSDS epoch) + 2 fine bytes (~15 µs resolution).
    pub const CCSDS_4_2: Self = Self::new(TimeCodeId::CcsdsEpoch, 4, 2);

    /// Standard 4+0 configuration: 4 coarse bytes, no fractional.
    pub const CCSDS_4_0: Self = Self::new(TimeCodeId::CcsdsEpoch, 4, 0);

    /// Returns the P-field byte for this configuration.
    ///
    /// This encodes the time code ID, coarse length, and fine
    /// length into a single byte. The extension flag is 0.
    pub const fn p_field(&self) -> u8 {
        let id = self.time_code_id as u8;
        let coarse_code = self.coarse_len - 1;
        let fine_code = self.fine_len;
        let mut pf = 0u8;
        set_bits_u8(&mut pf, TIME_CODE_ID_MASK, id);
        set_bits_u8(&mut pf, COARSE_LEN_MASK, coarse_code);
        set_bits_u8(&mut pf, FINE_LEN_MASK, fine_code);
        pf
    }

    /// Parses a P-field byte into a CUC configuration.
    pub const fn from_p_field(pf: u8) -> Result<Self, Error> {
        let id_bits = get_bits_u8(pf, TIME_CODE_ID_MASK);
        let coarse_code = get_bits_u8(pf, COARSE_LEN_MASK);
        let fine_code = get_bits_u8(pf, FINE_LEN_MASK);

        let time_code_id = match id_bits {
            0b001 => TimeCodeId::AgencyEpoch,
            0b010 => TimeCodeId::CcsdsEpoch,
            _ => return Err(Error::InvalidTimeCodeId(id_bits)),
        };

        Ok(Self {
            time_code_id,
            coarse_len: coarse_code + 1,
            fine_len: fine_code,
        })
    }

    /// Total T-field size in bytes (coarse + fine).
    pub const fn t_field_len(&self) -> usize {
        self.coarse_len as usize + self.fine_len as usize
    }

    /// Total encoded size including the P-field (1 + T-field).
    pub const fn encoded_len(&self) -> usize {
        1 + self.t_field_len()
    }
}

/// A CUC timestamp value.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CucTime {
    /// Configuration describing the encoding format.
    pub config: CucConfig,
    /// Coarse time: whole seconds since epoch.
    pub coarse: u32,
    /// Fine time: fractional seconds (interpretation depends on
    /// `config.fine_len`).
    pub fine: u32,
}

/// Errors for CUC time operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    /// The P-field contains an unrecognized time code ID.
    #[error("Invalid time code ID in P-field: {0:#03b}")]
    InvalidTimeCodeId(u8),
    /// The buffer is too short for the expected encoding.
    #[error("Buffer too short: required {required} bytes, but provided {provided} bytes")]
    BufferTooShort {
        /// Minimum bytes needed.
        required: usize,
        /// Actual bytes available.
        provided: usize,
    },
}

impl CucTime {
    /// Creates a new CUC timestamp from coarse and fine values.
    pub const fn new(config: CucConfig, coarse: u32, fine: u32) -> Self {
        Self {
            config,
            coarse,
            fine,
        }
    }

    /// Creates a CUC timestamp from a floating-point seconds value.
    ///
    /// The integer part becomes the coarse time; the fractional part
    /// is quantized into `config.fine_len` bytes of resolution.
    pub fn from_seconds(config: CucConfig, seconds: f64) -> Self {
        let coarse = seconds as u32;
        let frac = seconds - coarse as f64;
        let fine_bits = config.fine_len as u32 * 8;
        let fine = if fine_bits > 0 {
            (frac * (1u64 << fine_bits) as f64) as u32
        } else {
            0
        };
        Self {
            config,
            coarse,
            fine,
        }
    }

    /// Converts this timestamp to a floating-point seconds value.
    pub fn to_seconds(&self) -> f64 {
        let fine_bits = self.config.fine_len as u32 * 8;
        let frac = if fine_bits > 0 {
            self.fine as f64 / (1u64 << fine_bits) as f64
        } else {
            0.0
        };
        self.coarse as f64 + frac
    }

    /// Returns the fractional-second resolution in seconds.
    pub fn resolution(&self) -> f64 {
        let fine_bits = self.config.fine_len as u32 * 8;
        if fine_bits > 0 {
            1.0 / (1u64 << fine_bits) as f64
        } else {
            1.0
        }
    }

    /// Encodes this timestamp into a byte buffer (P-field + T-field).
    ///
    /// Returns the number of bytes written.
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let total = self.config.encoded_len();
        if buf.len() < total {
            return Err(Error::BufferTooShort {
                required: total,
                provided: buf.len(),
            });
        }

        buf[0] = self.config.p_field();
        let mut pos = 1;

        // Write coarse bytes (big-endian, only the low N bytes)
        let coarse_bytes = self.coarse.to_be_bytes();
        let coarse_len = self.config.coarse_len as usize;
        let coarse_start = 4 - coarse_len;
        buf[pos..pos + coarse_len].copy_from_slice(&coarse_bytes[coarse_start..]);
        pos += coarse_len;

        // Write fine bytes (big-endian, only the high N bytes)
        let fine_len = self.config.fine_len as usize;
        if fine_len > 0 {
            let fine_bytes = self.fine.to_be_bytes();
            buf[pos..pos + fine_len].copy_from_slice(&fine_bytes[..fine_len]);
            pos += fine_len;
        }

        Ok(pos)
    }

    /// Encodes only the T-field (no P-field) into a buffer.
    ///
    /// Useful when the P-field is implicit.
    pub fn encode_t_field(&self, buf: &mut [u8]) -> Result<usize, Error> {
        let t_len = self.config.t_field_len();
        if buf.len() < t_len {
            return Err(Error::BufferTooShort {
                required: t_len,
                provided: buf.len(),
            });
        }

        let mut pos = 0;

        let coarse_bytes = self.coarse.to_be_bytes();
        let coarse_len = self.config.coarse_len as usize;
        let coarse_start = 4 - coarse_len;
        buf[pos..pos + coarse_len].copy_from_slice(&coarse_bytes[coarse_start..]);
        pos += coarse_len;

        let fine_len = self.config.fine_len as usize;
        if fine_len > 0 {
            let fine_bytes = self.fine.to_be_bytes();
            buf[pos..pos + fine_len].copy_from_slice(&fine_bytes[..fine_len]);
            pos += fine_len;
        }

        Ok(pos)
    }

    /// Decodes a CUC timestamp from bytes (P-field + T-field).
    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.is_empty() {
            return Err(Error::BufferTooShort {
                required: 1,
                provided: 0,
            });
        }

        let config = CucConfig::from_p_field(buf[0])?;
        let total = config.encoded_len();
        if buf.len() < total {
            return Err(Error::BufferTooShort {
                required: total,
                provided: buf.len(),
            });
        }

        Self::decode_t_field(&config, &buf[1..])
    }

    /// Decodes a CUC timestamp from T-field bytes only.
    ///
    /// The caller provides the configuration (implicit P-field).
    pub fn decode_t_field(config: &CucConfig, buf: &[u8]) -> Result<Self, Error> {
        let t_len = config.t_field_len();
        if buf.len() < t_len {
            return Err(Error::BufferTooShort {
                required: t_len,
                provided: buf.len(),
            });
        }

        let mut pos = 0;
        let coarse_len = config.coarse_len as usize;
        let mut coarse_buf = [0u8; 4];
        let coarse_start = 4 - coarse_len;
        coarse_buf[coarse_start..].copy_from_slice(&buf[pos..pos + coarse_len]);
        let coarse = u32::from_be_bytes(coarse_buf);
        pos += coarse_len;

        let fine_len = config.fine_len as usize;
        let fine = if fine_len > 0 {
            let mut fine_buf = [0u8; 4];
            fine_buf[..fine_len].copy_from_slice(&buf[pos..pos + fine_len]);
            u32::from_be_bytes(fine_buf)
        } else {
            0
        };

        Ok(Self {
            config: *config,
            coarse,
            fine,
        })
    }
}

impl core::fmt::Display for CucTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CUC({:.6}s)", self.to_seconds())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_field_roundtrip() {
        let config = CucConfig::CCSDS_4_2;
        let pf = config.p_field();
        let parsed = CucConfig::from_p_field(pf).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn p_field_values() {
        // CcsdsEpoch (0b010), coarse=4 (code=3), fine=2
        // = 0b0_010_11_10 = 0x2E
        let config = CucConfig::CCSDS_4_2;
        assert_eq!(config.p_field(), 0x2E);

        // CcsdsEpoch (0b010), coarse=4 (code=3), fine=0
        // = 0b0_010_11_00 = 0x2C
        let config = CucConfig::CCSDS_4_0;
        assert_eq!(config.p_field(), 0x2C);
    }

    #[test]
    fn encode_decode_roundtrip_4_2() {
        let config = CucConfig::CCSDS_4_2;
        let t = CucTime::new(config, 1_000_000, 0x8000_0000);

        let mut buf = [0u8; 16];
        let len = t.encode(&mut buf).unwrap();
        assert_eq!(len, 7); // 1 P-field + 4 coarse + 2 fine

        let decoded = CucTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.coarse, 1_000_000);
        // Fine: we wrote 0x8000_0000, but only 2 bytes → 0x8000
        // decoded back into u32: 0x8000_0000
        assert_eq!(decoded.fine, 0x8000_0000);
    }

    #[test]
    fn encode_decode_roundtrip_4_0() {
        let config = CucConfig::CCSDS_4_0;
        let t = CucTime::new(config, 42, 0);

        let mut buf = [0u8; 8];
        let len = t.encode(&mut buf).unwrap();
        assert_eq!(len, 5); // 1 + 4

        let decoded = CucTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.coarse, 42);
        assert_eq!(decoded.fine, 0);
    }

    #[test]
    fn t_field_only() {
        let config = CucConfig::CCSDS_4_2;
        let t = CucTime::new(config, 12345, 0xABCD_0000);

        let mut buf = [0u8; 8];
        let len = t.encode_t_field(&mut buf).unwrap();
        assert_eq!(len, 6); // 4 coarse + 2 fine

        let decoded = CucTime::decode_t_field(&config, &buf[..len]).unwrap();
        assert_eq!(decoded.coarse, 12345);
        assert_eq!(decoded.fine, 0xABCD_0000);
    }

    #[test]
    fn from_seconds_and_back() {
        let config = CucConfig::CCSDS_4_2;
        let t = CucTime::from_seconds(config, 100.5);

        assert_eq!(t.coarse, 100);
        // 0.5 * 2^16 = 32768 = 0x8000 → stored as 0x8000_0000
        assert_eq!(t.fine, 0x8000);

        let secs = t.to_seconds();
        let diff = (secs - 100.5).abs();
        assert!(diff < 0.001);
    }

    #[test]
    fn resolution_values() {
        let c0 = CucConfig::new(TimeCodeId::CcsdsEpoch, 4, 0);
        assert_eq!(c0.fine_len, 0);
        assert_eq!(CucTime::new(c0, 0, 0).resolution(), 1.0);

        let c1 = CucConfig::new(TimeCodeId::CcsdsEpoch, 4, 1);
        let r1 = CucTime::new(c1, 0, 0).resolution();
        let diff1 = (r1 - 1.0 / 256.0).abs();
        assert!(diff1 < 1e-10);

        let c2 = CucConfig::CCSDS_4_2;
        let r2 = CucTime::new(c2, 0, 0).resolution();
        let diff2 = (r2 - 1.0 / 65536.0).abs();
        assert!(diff2 < 1e-12);

        let c3 = CucConfig::new(TimeCodeId::CcsdsEpoch, 4, 3);
        let r3 = CucTime::new(c3, 0, 0).resolution();
        let diff3 = (r3 - 1.0 / 16777216.0).abs();
        assert!(diff3 < 1e-15);
    }

    #[test]
    fn agency_epoch() {
        let config = CucConfig::new(TimeCodeId::AgencyEpoch, 2, 1);
        let t = CucTime::new(config, 300, 0x8000_0000);

        let mut buf = [0u8; 8];
        let len = t.encode(&mut buf).unwrap();
        assert_eq!(len, 4); // 1 P-field + 2 coarse + 1 fine

        let decoded = CucTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.config.time_code_id, TimeCodeId::AgencyEpoch);
        assert_eq!(decoded.coarse, 300);
        assert_eq!(decoded.fine, 0x8000_0000);
    }

    #[test]
    fn buffer_too_short() {
        let t = CucTime::new(CucConfig::CCSDS_4_2, 0, 0);
        let mut buf = [0u8; 3]; // need 7
        let err = t.encode(&mut buf);
        assert!(matches!(
            err,
            Err(Error::BufferTooShort {
                required: 7,
                provided: 3,
            })
        ));
    }

    #[test]
    fn invalid_time_code_id() {
        let err = CucConfig::from_p_field(0x00); // id = 0b000
        assert!(matches!(err, Err(Error::InvalidTimeCodeId(0))));
    }

    #[test]
    fn small_coarse_field() {
        // 1-byte coarse, 0 fine — can represent 0..255 seconds
        let config = CucConfig::new(TimeCodeId::CcsdsEpoch, 1, 0);
        let t = CucTime::new(config, 200, 0);

        let mut buf = [0u8; 4];
        let len = t.encode(&mut buf).unwrap();
        assert_eq!(len, 2); // 1 P-field + 1 coarse

        let decoded = CucTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.coarse, 200);
    }
}
