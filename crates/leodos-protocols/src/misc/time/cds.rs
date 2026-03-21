//! CCSDS Day Segmented (CDS) Time Format (CCSDS 301.0-B-4, §3.3)
//!
//! CDS encodes time as a day count since a reference epoch plus
//! milliseconds within the day, with optional sub-millisecond
//! resolution (microseconds or picoseconds).
//!
//! # P-Field
//!
//! ```text
//! Byte 1:
//!   [7]     Extension flag (0 = 1-byte P-field)
//!   [6:4]   Time code ID: 100 = CDS
//!   [3]     Epoch ID: 0 = CCSDS epoch, 1 = agency-defined
//!   [2]     Day segment length: 0 = 16-bit, 1 = 24-bit
//!   [1:0]   Sub-millisecond resolution:
//!           00 = none, 01 = µs (16-bit), 10 = ps (32-bit)
//! ```
//!
//! # T-Field
//!
//! ```text
//! ┌──────────┬────────────┬──────────────────┐
//! │ Day      │ ms of day  │ sub-ms (optional) │
//! │ 16/24 b  │ 32 bits    │ 16 or 32 bits     │
//! └──────────┴────────────┴──────────────────┘
//! ```
//!
//! # CCSDS Epoch
//!
//! Same as CUC: **1958-01-01T00:00:00 TAI**.

use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

/// Milliseconds in one day.
pub const MS_PER_DAY: u32 = 86_400_000;

/// Time code ID for CDS in the P-field.
const TIME_CODE_ID: u8 = 0b100;

/// Bitmasks for CDS P-field byte fields.
#[rustfmt::skip]
mod bitmask {
    /// Bitmask for the 3-bit time code ID field [6:4].
    pub const TIME_CODE_ID_MASK: u8 = 0b_0111_0000;
    /// Bitmask for the 1-bit epoch ID field [3].
    pub const EPOCH_ID_MASK: u8 =     0b_0000_1000;
    /// Bitmask for the 1-bit day segment length field [2].
    pub const DAY_SEG_MASK: u8 =      0b_0000_0100;
    /// Bitmask for the 2-bit sub-millisecond resolution field [1:0].
    pub const SUB_MILLIS_MASK: u8 =   0b_0000_0011;
}

use bitmask::*;

/// Sub-millisecond resolution options.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SubMillis {
    /// No sub-millisecond field.
    None,
    /// 16-bit microseconds within the millisecond (0..999).
    Microseconds,
    /// 32-bit picoseconds within the millisecond (0..999_999_999).
    Picoseconds,
}

impl SubMillis {
    /// Size of the sub-millisecond field in bytes.
    pub const fn field_len(self) -> usize {
        match self {
            Self::None => 0,
            Self::Microseconds => 2,
            Self::Picoseconds => 4,
        }
    }

    /// P-field code for this resolution.
    const fn code(self) -> u8 {
        match self {
            Self::None => 0b00,
            Self::Microseconds => 0b01,
            Self::Picoseconds => 0b10,
        }
    }

    /// Parses the sub-ms code from the P-field.
    const fn from_code(code: u8) -> Result<Self, CdsError> {
        match code {
            0b00 => Ok(Self::None),
            0b01 => Ok(Self::Microseconds),
            0b10 => Ok(Self::Picoseconds),
            _ => Err(CdsError::InvalidSubMillisCode(code)),
        }
    }
}

/// Epoch identifier.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum EpochId {
    /// CCSDS epoch: 1958-01-01T00:00:00 TAI.
    Ccsds,
    /// Agency-defined epoch.
    Agency,
}

/// CDS time code configuration.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CdsConfig {
    /// Epoch identifier.
    pub epoch: EpochId,
    /// Day segment uses 24 bits (true) or 16 bits (false).
    pub day_24bit: bool,
    /// Sub-millisecond resolution.
    pub sub_millis: SubMillis,
}

impl CdsConfig {
    /// Standard 16-bit day, no sub-ms, CCSDS epoch.
    pub const CCSDS_16: Self = Self {
        epoch: EpochId::Ccsds,
        day_24bit: false,
        sub_millis: SubMillis::None,
    };

    /// 16-bit day with microsecond resolution, CCSDS epoch.
    pub const CCSDS_16_US: Self = Self {
        epoch: EpochId::Ccsds,
        day_24bit: false,
        sub_millis: SubMillis::Microseconds,
    };

    /// Day segment size in bytes.
    pub const fn day_len(&self) -> usize {
        if self.day_24bit { 3 } else { 2 }
    }

    /// Total T-field size in bytes.
    pub const fn t_field_len(&self) -> usize {
        self.day_len() + 4 + self.sub_millis.field_len()
    }

    /// Total encoded size (P-field + T-field).
    pub const fn encoded_len(&self) -> usize {
        1 + self.t_field_len()
    }

    /// Encodes the P-field byte.
    pub const fn p_field(&self) -> u8 {
        let epoch_bit = match self.epoch {
            EpochId::Ccsds => 0,
            EpochId::Agency => 1,
        };
        let day_bit = if self.day_24bit { 1 } else { 0 };
        let mut pf = 0u8;
        set_bits_u8(&mut pf, TIME_CODE_ID_MASK, TIME_CODE_ID);
        set_bits_u8(&mut pf, EPOCH_ID_MASK, epoch_bit);
        set_bits_u8(&mut pf, DAY_SEG_MASK, day_bit);
        set_bits_u8(&mut pf, SUB_MILLIS_MASK, self.sub_millis.code());
        pf
    }

    /// Parses a P-field byte into a CDS configuration.
    pub const fn from_p_field(pf: u8) -> Result<Self, CdsError> {
        let id = get_bits_u8(pf, TIME_CODE_ID_MASK);
        if id != TIME_CODE_ID {
            return Err(CdsError::NotCds(id));
        }
        let epoch_bit = get_bits_u8(pf, EPOCH_ID_MASK);
        let day_bit = get_bits_u8(pf, DAY_SEG_MASK);
        let sub_code = get_bits_u8(pf, SUB_MILLIS_MASK);

        let epoch = if epoch_bit == 0 {
            EpochId::Ccsds
        } else {
            EpochId::Agency
        };

        let sub_millis = match SubMillis::from_code(sub_code) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        Ok(Self {
            epoch,
            day_24bit: day_bit == 1,
            sub_millis,
        })
    }
}

/// A CDS timestamp.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CdsTime {
    /// Encoding configuration.
    pub config: CdsConfig,
    /// Day count since epoch.
    pub day: u32,
    /// Milliseconds within the day (0..86_399_999).
    pub ms_of_day: u32,
    /// Sub-millisecond value (µs 0..999 or ps 0..999_999_999).
    pub sub_ms: u32,
}

/// Errors from CDS time operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CdsError {
    /// P-field time code ID is not CDS (100).
    NotCds(u8),
    /// Invalid sub-millisecond code in P-field.
    InvalidSubMillisCode(u8),
    /// Buffer too short.
    BufferTooShort {
        /// Minimum bytes needed.
        required: usize,
        /// Bytes available.
        provided: usize,
    },
    /// Milliseconds value exceeds 86_399_999.
    MsOutOfRange(u32),
}

impl CdsTime {
    /// Creates a new CDS timestamp.
    pub const fn new(
        config: CdsConfig,
        day: u32,
        ms_of_day: u32,
        sub_ms: u32,
    ) -> Self {
        Self { config, day, ms_of_day, sub_ms }
    }

    /// Creates a CDS timestamp from total seconds since epoch.
    pub fn from_seconds(config: CdsConfig, seconds: f64) -> Self {
        let total_ms = (seconds * 1000.0) as u64;
        let day = (total_ms / MS_PER_DAY as u64) as u32;
        let ms_of_day = (total_ms % MS_PER_DAY as u64) as u32;

        let frac_ms = seconds * 1000.0 - (total_ms as f64);
        let sub_ms = match config.sub_millis {
            SubMillis::None => 0,
            SubMillis::Microseconds => {
                (frac_ms * 1000.0) as u32
            }
            SubMillis::Picoseconds => {
                (frac_ms * 1_000_000_000.0) as u32
            }
        };

        Self { config, day, ms_of_day, sub_ms }
    }

    /// Converts to total seconds since epoch.
    pub fn to_seconds(&self) -> f64 {
        let day_secs = self.day as f64 * 86_400.0;
        let ms_secs = self.ms_of_day as f64 / 1000.0;
        let sub_secs = match self.config.sub_millis {
            SubMillis::None => 0.0,
            SubMillis::Microseconds => {
                self.sub_ms as f64 / 1_000_000.0
            }
            SubMillis::Picoseconds => {
                self.sub_ms as f64 / 1_000_000_000_000.0
            }
        };
        day_secs + ms_secs + sub_secs
    }

    /// Encodes this timestamp (P-field + T-field).
    pub fn encode(&self, buf: &mut [u8]) -> Result<usize, CdsError> {
        let total = self.config.encoded_len();
        if buf.len() < total {
            return Err(CdsError::BufferTooShort {
                required: total,
                provided: buf.len(),
            });
        }

        buf[0] = self.config.p_field();
        self.write_t_field(&mut buf[1..])?;
        Ok(total)
    }

    /// Encodes only the T-field (no P-field).
    pub fn encode_t_field(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, CdsError> {
        let t_len = self.config.t_field_len();
        if buf.len() < t_len {
            return Err(CdsError::BufferTooShort {
                required: t_len,
                provided: buf.len(),
            });
        }
        self.write_t_field(buf)?;
        Ok(t_len)
    }

    fn write_t_field(&self, buf: &mut [u8]) -> Result<(), CdsError> {
        let mut pos = 0;

        // Day segment
        let day_bytes = self.day.to_be_bytes();
        if self.config.day_24bit {
            buf[pos..pos + 3].copy_from_slice(&day_bytes[1..4]);
            pos += 3;
        } else {
            buf[pos..pos + 2].copy_from_slice(&day_bytes[2..4]);
            pos += 2;
        }

        // Milliseconds of day
        let ms_bytes = self.ms_of_day.to_be_bytes();
        buf[pos..pos + 4].copy_from_slice(&ms_bytes);
        pos += 4;

        // Sub-millisecond
        match self.config.sub_millis {
            SubMillis::None => {}
            SubMillis::Microseconds => {
                let us_bytes = (self.sub_ms as u16).to_be_bytes();
                buf[pos..pos + 2].copy_from_slice(&us_bytes);
            }
            SubMillis::Picoseconds => {
                let ps_bytes = self.sub_ms.to_be_bytes();
                buf[pos..pos + 4].copy_from_slice(&ps_bytes);
            }
        }

        Ok(())
    }

    /// Decodes a CDS timestamp from bytes (P-field + T-field).
    pub fn decode(buf: &[u8]) -> Result<Self, CdsError> {
        if buf.is_empty() {
            return Err(CdsError::BufferTooShort {
                required: 1,
                provided: 0,
            });
        }

        let config = CdsConfig::from_p_field(buf[0])?;
        let total = config.encoded_len();
        if buf.len() < total {
            return Err(CdsError::BufferTooShort {
                required: total,
                provided: buf.len(),
            });
        }

        Self::decode_t_field(&config, &buf[1..])
    }

    /// Decodes from T-field bytes with an implicit configuration.
    pub fn decode_t_field(
        config: &CdsConfig,
        buf: &[u8],
    ) -> Result<Self, CdsError> {
        let t_len = config.t_field_len();
        if buf.len() < t_len {
            return Err(CdsError::BufferTooShort {
                required: t_len,
                provided: buf.len(),
            });
        }

        let mut pos = 0;

        // Day
        let day = if config.day_24bit {
            let mut d = [0u8; 4];
            d[1..4].copy_from_slice(&buf[pos..pos + 3]);
            pos += 3;
            u32::from_be_bytes(d)
        } else {
            let mut d = [0u8; 4];
            d[2..4].copy_from_slice(&buf[pos..pos + 2]);
            pos += 2;
            u32::from_be_bytes(d)
        };

        // Milliseconds of day
        let ms_of_day = u32::from_be_bytes([
            buf[pos],
            buf[pos + 1],
            buf[pos + 2],
            buf[pos + 3],
        ]);
        pos += 4;

        // Sub-millisecond
        let sub_ms = match config.sub_millis {
            SubMillis::None => 0,
            SubMillis::Microseconds => {
                let v = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
                v as u32
            }
            SubMillis::Picoseconds => {
                u32::from_be_bytes([
                    buf[pos],
                    buf[pos + 1],
                    buf[pos + 2],
                    buf[pos + 3],
                ])
            }
        };

        Ok(Self {
            config: *config,
            day,
            ms_of_day,
            sub_ms,
        })
    }
}

impl core::fmt::Display for CdsTime {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        let h = self.ms_of_day / 3_600_000;
        let m = (self.ms_of_day % 3_600_000) / 60_000;
        let s = (self.ms_of_day % 60_000) / 1000;
        let ms = self.ms_of_day % 1000;
        write!(
            f,
            "CDS(day={}, {:02}:{:02}:{:02}.{:03}",
            self.day, h, m, s, ms
        )?;
        match self.config.sub_millis {
            SubMillis::None => {}
            SubMillis::Microseconds => {
                write!(f, ".{:03}µs", self.sub_ms)?;
            }
            SubMillis::Picoseconds => {
                write!(f, ".{:09}ps", self.sub_ms)?;
            }
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_field_roundtrip() {
        let config = CdsConfig::CCSDS_16;
        let pf = config.p_field();
        let parsed = CdsConfig::from_p_field(pf).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn p_field_with_us() {
        let config = CdsConfig::CCSDS_16_US;
        let pf = config.p_field();
        let parsed = CdsConfig::from_p_field(pf).unwrap();
        assert_eq!(parsed, config);
        assert_eq!(parsed.sub_millis, SubMillis::Microseconds);
    }

    #[test]
    fn p_field_24bit_day() {
        let config = CdsConfig {
            epoch: EpochId::Ccsds,
            day_24bit: true,
            sub_millis: SubMillis::None,
        };
        let pf = config.p_field();
        let parsed = CdsConfig::from_p_field(pf).unwrap();
        assert!(parsed.day_24bit);
    }

    #[test]
    fn p_field_agency_epoch() {
        let config = CdsConfig {
            epoch: EpochId::Agency,
            day_24bit: false,
            sub_millis: SubMillis::None,
        };
        let pf = config.p_field();
        assert_eq!(get_bits_u8(pf, EPOCH_ID_MASK), 1);
        let parsed = CdsConfig::from_p_field(pf).unwrap();
        assert_eq!(parsed.epoch, EpochId::Agency);
    }

    #[test]
    fn p_field_value() {
        // CDS (100), CCSDS epoch (0), 16-bit day (0), no sub-ms (00)
        // = 0b0_100_0_0_00 = 0x40
        assert_eq!(CdsConfig::CCSDS_16.p_field(), 0x40);
        // CDS (100), CCSDS epoch (0), 16-bit day (0), µs (01)
        // = 0b0_100_0_0_01 = 0x41
        assert_eq!(CdsConfig::CCSDS_16_US.p_field(), 0x41);
    }

    #[test]
    fn encode_decode_16bit_no_subms() {
        let t = CdsTime::new(CdsConfig::CCSDS_16, 1000, 43_200_000, 0);
        let mut buf = [0u8; 16];
        let len = t.encode(&mut buf).unwrap();
        // 1 P + 2 day + 4 ms = 7
        assert_eq!(len, 7);

        let decoded = CdsTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.day, 1000);
        assert_eq!(decoded.ms_of_day, 43_200_000);
        assert_eq!(decoded.sub_ms, 0);
    }

    #[test]
    fn encode_decode_16bit_us() {
        let t = CdsTime::new(CdsConfig::CCSDS_16_US, 500, 1000, 750);
        let mut buf = [0u8; 16];
        let len = t.encode(&mut buf).unwrap();
        // 1 P + 2 day + 4 ms + 2 µs = 9
        assert_eq!(len, 9);

        let decoded = CdsTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.day, 500);
        assert_eq!(decoded.ms_of_day, 1000);
        assert_eq!(decoded.sub_ms, 750);
    }

    #[test]
    fn encode_decode_24bit_ps() {
        let config = CdsConfig {
            epoch: EpochId::Ccsds,
            day_24bit: true,
            sub_millis: SubMillis::Picoseconds,
        };
        let t = CdsTime::new(config, 100_000, 50_000_000, 123_456_789);
        let mut buf = [0u8; 16];
        let len = t.encode(&mut buf).unwrap();
        // 1 P + 3 day + 4 ms + 4 ps = 12
        assert_eq!(len, 12);

        let decoded = CdsTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.day, 100_000);
        assert_eq!(decoded.ms_of_day, 50_000_000);
        assert_eq!(decoded.sub_ms, 123_456_789);
    }

    #[test]
    fn t_field_only() {
        let config = CdsConfig::CCSDS_16;
        let t = CdsTime::new(config, 365, 72_000_000, 0);
        let mut buf = [0u8; 8];
        let len = t.encode_t_field(&mut buf).unwrap();
        assert_eq!(len, 6); // 2 day + 4 ms

        let decoded =
            CdsTime::decode_t_field(&config, &buf[..len]).unwrap();
        assert_eq!(decoded.day, 365);
        assert_eq!(decoded.ms_of_day, 72_000_000);
    }

    #[test]
    fn from_seconds_and_back() {
        let config = CdsConfig::CCSDS_16;
        // 1.5 days = 129600 seconds
        let t = CdsTime::from_seconds(config, 129_600.0);
        assert_eq!(t.day, 1);
        assert_eq!(t.ms_of_day, 43_200_000); // 12 hours in ms

        let secs = t.to_seconds();
        assert!((secs - 129_600.0).abs() < 0.001);
    }

    #[test]
    fn from_seconds_with_us() {
        let config = CdsConfig::CCSDS_16_US;
        // 0.0015005 seconds = 1 ms + 500 µs + 0.5 µs
        let t = CdsTime::from_seconds(config, 0.001_500_5);
        assert_eq!(t.day, 0);
        assert_eq!(t.ms_of_day, 1);
        assert_eq!(t.sub_ms, 500);
    }

    #[test]
    fn buffer_too_short() {
        let t = CdsTime::new(CdsConfig::CCSDS_16, 0, 0, 0);
        let mut buf = [0u8; 3];
        assert!(matches!(
            t.encode(&mut buf),
            Err(CdsError::BufferTooShort { required: 7, .. })
        ));
    }

    #[test]
    fn not_cds_p_field() {
        // CUC P-field (time code ID = 010)
        let err = CdsConfig::from_p_field(0x2E);
        assert!(matches!(err, Err(CdsError::NotCds(0b010))));
    }

    #[test]
    fn max_16bit_day() {
        let t = CdsTime::new(CdsConfig::CCSDS_16, 65535, 0, 0);
        let mut buf = [0u8; 8];
        let len = t.encode(&mut buf).unwrap();
        let decoded = CdsTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.day, 65535);
    }

    #[test]
    fn max_24bit_day() {
        let config = CdsConfig {
            epoch: EpochId::Ccsds,
            day_24bit: true,
            sub_millis: SubMillis::None,
        };
        let t = CdsTime::new(config, 0xFF_FFFF, 0, 0);
        let mut buf = [0u8; 12];
        let len = t.encode(&mut buf).unwrap();
        let decoded = CdsTime::decode(&buf[..len]).unwrap();
        assert_eq!(decoded.day, 0xFF_FFFF);
    }

    #[test]
    fn midnight_and_end_of_day() {
        let config = CdsConfig::CCSDS_16;

        let midnight = CdsTime::new(config, 0, 0, 0);
        assert_eq!(midnight.ms_of_day, 0);

        let end = CdsTime::new(config, 0, MS_PER_DAY - 1, 0);
        assert_eq!(end.ms_of_day, 86_399_999);

        let mut buf = [0u8; 8];
        end.encode(&mut buf).unwrap();
        let decoded = CdsTime::decode(&buf).unwrap();
        assert_eq!(decoded.ms_of_day, 86_399_999);
    }

    #[test]
    fn encoded_len_values() {
        assert_eq!(CdsConfig::CCSDS_16.encoded_len(), 7);
        assert_eq!(CdsConfig::CCSDS_16_US.encoded_len(), 9);
        let ps_24 = CdsConfig {
            epoch: EpochId::Ccsds,
            day_24bit: true,
            sub_millis: SubMillis::Picoseconds,
        };
        assert_eq!(ps_24.encoded_len(), 12);
    }
}
