//! ISP1 (Internet SLE Protocol 1) transport layer.
//!
//! Each SLE PDU is framed with a 4-byte big-endian length prefix
//! for transmission over TCP. This module provides the framing
//! types and authentication credential structures — actual TCP I/O
//! is left to the caller.

use super::types::SleError;

/// ISP1 framing header size: 4-byte big-endian length prefix.
pub const ISP1_HEADER_SIZE: usize = 4;

/// An ISP1 length-prefixed frame.
///
/// The wire format is simply `[u32 BE length][payload]` where
/// `length` is the number of payload bytes (not including itself).
pub struct Isp1Frame;

impl Isp1Frame {
    /// Encodes an ISP1 frame by writing the 4-byte length prefix
    /// followed by the payload into `buf`.
    ///
    /// Returns the total number of bytes written
    /// (4 + payload.len()).
    pub fn encode(
        payload: &[u8],
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        let total = ISP1_HEADER_SIZE + payload.len();
        if buf.len() < total {
            return Err(SleError::BufferTooSmall);
        }
        let len_bytes =
            (payload.len() as u32).to_be_bytes();
        buf[..4].copy_from_slice(&len_bytes);
        buf[4..total].copy_from_slice(payload);
        Ok(total)
    }

    /// Reads the 4-byte length prefix from `buf`, returning
    /// the payload length. Does not validate or consume the
    /// payload — the caller should read that many bytes next.
    ///
    /// Returns `(payload_length, bytes_consumed)` where
    /// `bytes_consumed` is always 4.
    pub fn decode_header(
        buf: &[u8],
    ) -> Result<(usize, usize), SleError> {
        if buf.len() < ISP1_HEADER_SIZE {
            return Err(SleError::Truncated);
        }
        let len = u32::from_be_bytes([
            buf[0], buf[1], buf[2], buf[3],
        ]) as usize;
        Ok((len, ISP1_HEADER_SIZE))
    }

    /// Decodes a complete ISP1 frame (header + payload) from
    /// `buf`, returning a slice of the payload.
    pub fn decode(buf: &[u8]) -> Result<&[u8], SleError> {
        let (payload_len, hdr) = Self::decode_header(buf)?;
        let total = hdr + payload_len;
        if buf.len() < total {
            return Err(SleError::Truncated);
        }
        Ok(&buf[hdr..total])
    }
}

/// ISP1 authentication credentials.
///
/// Used in Bind invocations to prove the caller's identity.
/// The hash is SHA-1 over (time ++ random ++ password).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Credentials {
    /// CDS time at which the credentials were generated.
    pub time: [u8; 8],
    /// Random nonce to prevent replay attacks.
    pub random: u32,
    /// SHA-1 hash: H(time || random || password).
    pub hash: [u8; 20],
}

impl Credentials {
    /// Total encoded size: 8 (time) + 4 (random) + 20 (hash).
    pub const SIZE: usize = 8 + 4 + 20;

    /// Encodes credentials into the buffer.
    /// Returns the number of bytes written (always 32).
    pub fn encode(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, SleError> {
        if buf.len() < Self::SIZE {
            return Err(SleError::BufferTooSmall);
        }
        buf[..8].copy_from_slice(&self.time);
        buf[8..12].copy_from_slice(&self.random.to_be_bytes());
        buf[12..32].copy_from_slice(&self.hash);
        Ok(Self::SIZE)
    }

    /// Decodes credentials from the buffer.
    pub fn decode(buf: &[u8]) -> Result<(Self, usize), SleError> {
        if buf.len() < Self::SIZE {
            return Err(SleError::Truncated);
        }
        let mut time = [0u8; 8];
        time.copy_from_slice(&buf[..8]);
        let random = u32::from_be_bytes([
            buf[8], buf[9], buf[10], buf[11],
        ]);
        let mut hash = [0u8; 20];
        hash.copy_from_slice(&buf[12..32]);
        Ok((Self { time, random, hash }, Self::SIZE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isp1_frame_roundtrip() {
        let payload = b"SLE RAF data";
        let mut buf = [0u8; 64];
        let n = Isp1Frame::encode(payload, &mut buf).unwrap();
        assert_eq!(n, 4 + payload.len());

        let decoded = Isp1Frame::decode(&buf[..n]).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn isp1_decode_header() {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&100u32.to_be_bytes());
        let (len, consumed) =
            Isp1Frame::decode_header(&buf).unwrap();
        assert_eq!(len, 100);
        assert_eq!(consumed, 4);
    }

    #[test]
    fn credentials_roundtrip() {
        let cred = Credentials {
            time: [1, 2, 3, 4, 5, 6, 7, 8],
            random: 0xDEADBEEF,
            hash: [
                0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
                0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF,
                0xB0, 0xB1, 0xB2, 0xB3,
            ],
        };
        let mut buf = [0u8; 64];
        let n = cred.encode(&mut buf).unwrap();
        assert_eq!(n, Credentials::SIZE);

        let (decoded, consumed) =
            Credentials::decode(&buf[..n]).unwrap();
        assert_eq!(consumed, n);
        assert_eq!(decoded, cred);
    }
}
