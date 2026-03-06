//! SDLS Security Header and Trailer parsing (CCSDS 355.0-B-2 Section 4.1).

use super::{Error, SecurityAssociation};

/// A parsed view over a Security Header in a byte buffer.
///
/// The Security Header consists of:
/// - SPI (16 bits, mandatory)
/// - Initialization Vector (0-32 bytes, optional)
/// - Sequence Number (0-8 bytes, optional)
/// - Pad Length (0-2 bytes, optional)
///
/// All field lengths are determined by the Security Association.
#[derive(Debug)]
pub struct SecurityHeader<'a> {
    spi: u16,
    iv: &'a [u8],
    sn: &'a [u8],
    pl: &'a [u8],
}

impl<'a> SecurityHeader<'a> {
    /// Parse a Security Header from the given bytes using the SA
    /// to determine field lengths.
    pub fn parse(
        sa: &SecurityAssociation,
        bytes: &'a [u8],
    ) -> Result<Self, Error> {
        let hdr_size = sa.header_size();
        if bytes.len() < hdr_size {
            return Err(Error::FrameTooShort);
        }

        let spi =
            u16::from_be_bytes([bytes[0], bytes[1]]);
        let mut pos = 2;

        let iv_len = sa.iv_len as usize;
        let iv = &bytes[pos..pos + iv_len];
        pos += iv_len;

        let sn_len = sa.sn_len as usize;
        let sn = &bytes[pos..pos + sn_len];
        pos += sn_len;

        let pl_len = sa.pl_len as usize;
        let pl = &bytes[pos..pos + pl_len];

        Ok(Self { spi, iv, sn, pl })
    }

    /// Returns the Security Parameter Index.
    pub fn spi(&self) -> u16 {
        self.spi
    }

    /// Returns the Initialization Vector bytes.
    pub fn iv(&self) -> &[u8] {
        self.iv
    }

    /// Returns the Sequence Number bytes.
    pub fn sequence_number(&self) -> &[u8] {
        self.sn
    }

    /// Returns the Sequence Number as a u64 value.
    pub fn sequence_number_value(&self) -> u64 {
        let mut buf = [0u8; 8];
        let len = self.sn.len();
        if len > 0 {
            let start = 8 - len;
            buf[start..].copy_from_slice(self.sn);
        }
        u64::from_be_bytes(buf)
    }

    /// Returns the Pad Length value.
    pub fn pad_length(&self) -> u16 {
        let len = self.pl.len();
        if len == 0 {
            return 0;
        }
        let mut buf = [0u8; 2];
        let start = 2 - len;
        buf[start..].copy_from_slice(self.pl);
        u16::from_be_bytes(buf)
    }
}

/// A parsed view over a Security Trailer in a byte buffer.
///
/// The Security Trailer consists of a single optional MAC field.
#[derive(Debug)]
pub struct SecurityTrailer<'a> {
    mac: &'a [u8],
}

impl<'a> SecurityTrailer<'a> {
    /// Parse a Security Trailer from the given bytes.
    pub fn parse(
        sa: &SecurityAssociation,
        bytes: &'a [u8],
    ) -> Result<Self, Error> {
        let mac_len = sa.mac_len as usize;
        if bytes.len() < mac_len {
            return Err(Error::FrameTooShort);
        }
        Ok(Self {
            mac: &bytes[..mac_len],
        })
    }

    /// Returns the MAC bytes.
    pub fn mac(&self) -> &[u8] {
        self.mac
    }
}
