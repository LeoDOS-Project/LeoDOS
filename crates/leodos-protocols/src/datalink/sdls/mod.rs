//! Space Data Link Security (SDLS) Protocol (CCSDS 355.0-B-2).
//!
//! Provides authentication, encryption, and authenticated encryption
//! services for CCSDS data link frames (TM, TC, AOS, USLP).
//!
//! The protocol inserts a Security Header after the transfer frame
//! header and an optional Security Trailer before the frame trailer.
//! All field lengths are determined by the Security Association (SA),
//! which is identified by the Security Parameter Index (SPI) in the
//! Security Header.

mod security_header;

pub use security_header::SecurityHeader;
pub use security_header::SecurityTrailer;

/// Maximum Security Header size in bytes (per CCSDS 355.0-B-2 4.1.1.1.4).
pub const MAX_SECURITY_HEADER_SIZE: usize = 64;

/// Maximum MAC size in bytes (per Table 6-1).
pub const MAX_MAC_SIZE: usize = 64;

/// Maximum IV size in bytes (per Table 6-1).
pub const MAX_IV_SIZE: usize = 32;

/// Maximum Sequence Number size in bytes (per Table 6-1).
pub const MAX_SN_SIZE: usize = 8;

/// Maximum Pad Length field size in bytes (per Table 6-1).
pub const MAX_PL_SIZE: usize = 2;

/// The cryptographic service type of a Security Association.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ServiceType {
    /// Authentication only (MAC computed, data in cleartext).
    Authentication,
    /// Encryption only (data encrypted, no MAC).
    Encryption,
    /// Authenticated encryption (encrypt-then-MAC).
    AuthenticatedEncryption,
}

/// Configuration for a Security Association (SA).
///
/// An SA defines the security context for a virtual channel or MAP.
/// Both sender and receiver must share identical SA parameters.
#[derive(Debug, Clone)]
pub struct SecurityAssociation {
    /// Security Parameter Index (1-65534; 0 and 65535 reserved).
    pub spi: u16,
    /// The cryptographic service type.
    pub service_type: ServiceType,
    /// Length of the IV field in the Security Header (0-32 bytes).
    pub iv_len: u8,
    /// Length of the Sequence Number field (0-8 bytes).
    pub sn_len: u8,
    /// Length of the Pad Length field (0-2 bytes).
    pub pl_len: u8,
    /// Length of the MAC field in the Security Trailer (0-64 bytes).
    pub mac_len: u8,
    /// Current anti-replay sequence number (sender: next to send).
    pub sequence_number: u64,
    /// Sequence number window for the receiver.
    pub sequence_window: u64,
    /// Authentication bit mask (applied before MAC computation).
    /// Sized to cover the largest expected frame.
    pub auth_mask: heapless::Vec<u8, 2048>,
}

impl SecurityAssociation {
    /// Total size of the Security Header for this SA.
    pub fn header_size(&self) -> usize {
        2 + self.iv_len as usize
            + self.sn_len as usize
            + self.pl_len as usize
    }

    /// Total size of the Security Trailer for this SA.
    pub fn trailer_size(&self) -> usize {
        self.mac_len as usize
    }

    /// Total overhead added by SDLS (header + trailer).
    pub fn overhead(&self) -> usize {
        self.header_size() + self.trailer_size()
    }
}

/// Errors from SDLS processing.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Error {
    /// The SPI value is reserved (0 or 65535).
    ReservedSpi(u16),
    /// No SA found for the given SPI.
    UnknownSpi(u16),
    /// The buffer is too small for the security fields.
    BufferTooSmall {
        /// Minimum bytes needed.
        required: usize,
        /// Actual bytes available.
        available: usize,
    },
    /// The frame is too short to contain a Security Header.
    FrameTooShort,
    /// MAC verification failed.
    MacVerificationFailed,
    /// Anti-replay sequence number check failed.
    SequenceNumberRejected {
        /// The received sequence number.
        received: u64,
        /// The expected sequence number.
        expected: u64,
    },
    /// Padding error during decryption.
    PaddingError,
    /// A crypto backend error occurred.
    CryptoError,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ReservedSpi(spi) => {
                write!(f, "reserved SPI: {spi}")
            }
            Self::UnknownSpi(spi) => {
                write!(f, "unknown SPI: {spi}")
            }
            Self::BufferTooSmall {
                required,
                available,
            } => {
                write!(
                    f,
                    "buffer too small: need {required}, have {available}"
                )
            }
            Self::FrameTooShort => write!(f, "frame too short"),
            Self::MacVerificationFailed => {
                write!(f, "MAC verification failed")
            }
            Self::SequenceNumberRejected {
                received,
                expected,
            } => write!(
                f,
                "sequence number rejected: got {received}, \
                 expected {expected}"
            ),
            Self::PaddingError => write!(f, "padding error"),
            Self::CryptoError => write!(f, "crypto error"),
        }
    }
}

impl core::error::Error for Error {}

/// Trait for pluggable cryptographic backends.
///
/// Implementations provide the actual encryption, decryption, and
/// MAC computation. The SDLS framing layer is algorithm-agnostic;
/// concrete algorithms (AES-GCM, HMAC, etc.) are supplied here.
pub trait CryptoProvider {
    /// Encrypt the plaintext in place, returning the number of
    /// padding bytes added. The `iv` is the current initialization
    /// vector from the SA.
    fn encrypt(
        &self,
        sa: &SecurityAssociation,
        iv: &[u8],
        plaintext: &mut [u8],
    ) -> Result<usize, Error>;

    /// Decrypt the ciphertext in place. Returns the number of
    /// padding bytes that were added during encryption (to be
    /// stripped by the caller).
    fn decrypt(
        &self,
        sa: &SecurityAssociation,
        iv: &[u8],
        ciphertext: &mut [u8],
    ) -> Result<usize, Error>;

    /// Compute a MAC over the authentication payload.
    /// The result is written into `mac_out` (length = `sa.mac_len`).
    fn compute_mac(
        &self,
        sa: &SecurityAssociation,
        auth_payload: &[u8],
        mac_out: &mut [u8],
    ) -> Result<(), Error>;
}

/// A no-op crypto provider for "clear mode" testing.
///
/// Per CCSDS 355.0-B-2, a clear-mode SA uses a no-op algorithm so
/// the Security Header and Trailer are present but no actual crypto
/// is performed. Useful for development and integration testing.
pub struct ClearModeCrypto;

impl CryptoProvider for ClearModeCrypto {
    fn encrypt(
        &self,
        _sa: &SecurityAssociation,
        _iv: &[u8],
        _plaintext: &mut [u8],
    ) -> Result<usize, Error> {
        Ok(0) // no padding
    }

    fn decrypt(
        &self,
        _sa: &SecurityAssociation,
        _iv: &[u8],
        _ciphertext: &mut [u8],
    ) -> Result<usize, Error> {
        Ok(0)
    }

    fn compute_mac(
        &self,
        sa: &SecurityAssociation,
        _auth_payload: &[u8],
        mac_out: &mut [u8],
    ) -> Result<(), Error> {
        // Fill MAC with zeros in clear mode.
        let len = sa.mac_len as usize;
        mac_out[..len].fill(0);
        Ok(())
    }
}

/// Write a Security Header into the given buffer.
///
/// Returns the number of bytes written.
pub fn write_security_header(
    sa: &SecurityAssociation,
    iv: &[u8],
    sn: &[u8],
    pad_len: u16,
    out: &mut [u8],
) -> Result<usize, Error> {
    let hdr_size = sa.header_size();
    if out.len() < hdr_size {
        return Err(Error::BufferTooSmall {
            required: hdr_size,
            available: out.len(),
        });
    }
    if sa.spi == 0 || sa.spi == 0xFFFF {
        return Err(Error::ReservedSpi(sa.spi));
    }

    let mut pos = 0;

    // SPI (16 bits, big-endian)
    out[pos..pos + 2].copy_from_slice(&sa.spi.to_be_bytes());
    pos += 2;

    // IV
    let iv_len = sa.iv_len as usize;
    if iv_len > 0 {
        out[pos..pos + iv_len].copy_from_slice(&iv[..iv_len]);
        pos += iv_len;
    }

    // Sequence Number
    let sn_len = sa.sn_len as usize;
    if sn_len > 0 {
        out[pos..pos + sn_len].copy_from_slice(&sn[..sn_len]);
        pos += sn_len;
    }

    // Pad Length
    let pl_len = sa.pl_len as usize;
    if pl_len > 0 {
        let pl_bytes = pad_len.to_be_bytes();
        let start = 2 - pl_len;
        out[pos..pos + pl_len]
            .copy_from_slice(&pl_bytes[start..]);
        pos += pl_len;
    }

    Ok(pos)
}

/// Read the SPI from the first 2 bytes of a security header.
pub fn read_spi(header_bytes: &[u8]) -> Result<u16, Error> {
    if header_bytes.len() < 2 {
        return Err(Error::FrameTooShort);
    }
    Ok(u16::from_be_bytes([header_bytes[0], header_bytes[1]]))
}

/// Extract fields from a security header given the SA configuration.
pub fn parse_security_header<'a>(
    sa: &SecurityAssociation,
    header_bytes: &'a [u8],
) -> Result<SecurityHeader<'a>, Error> {
    let hdr_size = sa.header_size();
    if header_bytes.len() < hdr_size {
        return Err(Error::FrameTooShort);
    }
    SecurityHeader::parse(sa, header_bytes)
}

/// Apply security processing to a frame (sending side).
///
/// Given a frame buffer with the transfer frame header already
/// written, this function:
/// 1. Writes the Security Header after `header_end`
/// 2. Encrypts the data field if needed
/// 3. Computes and writes the MAC if needed
///
/// The caller must have left room for the security overhead.
///
/// `frame` is the complete frame buffer.
/// `header_end` is the offset where the Security Header starts.
/// `data_start` is the offset where the Frame Data Field starts.
/// `data_end` is the offset where the Frame Data Field ends.
pub fn apply_security(
    sa: &mut SecurityAssociation,
    crypto: &impl CryptoProvider,
    frame: &mut [u8],
    header_end: usize,
    data_start: usize,
    data_end: usize,
) -> Result<(), Error> {
    if sa.spi == 0 || sa.spi == 0xFFFF {
        return Err(Error::ReservedSpi(sa.spi));
    }

    let trailer_start = data_end;
    let trailer_end = trailer_start + sa.trailer_size();
    if frame.len() < trailer_end {
        return Err(Error::BufferTooSmall {
            required: trailer_end,
            available: frame.len(),
        });
    }

    // Prepare IV and SN
    let mut iv_buf = [0u8; MAX_IV_SIZE];
    let mut sn_buf = [0u8; MAX_SN_SIZE];
    let iv_len = sa.iv_len as usize;
    let sn_len = sa.sn_len as usize;

    // Write current IV from SA sequence state
    if iv_len > 0 {
        let sn_bytes = sa.sequence_number.to_be_bytes();
        let start = 8usize.saturating_sub(iv_len);
        let copy_len = iv_len.min(8);
        let iv_start = iv_len.saturating_sub(8);
        iv_buf[iv_start..iv_start + copy_len]
            .copy_from_slice(&sn_bytes[start..start + copy_len]);
    }

    // Write sequence number
    if sn_len > 0 {
        let sn_bytes = sa.sequence_number.to_be_bytes();
        let start = 8 - sn_len;
        sn_buf[..sn_len].copy_from_slice(&sn_bytes[start..]);
    }

    // Encrypt if needed
    let mut pad_len: u16 = 0;
    let needs_encrypt = matches!(
        sa.service_type,
        ServiceType::Encryption | ServiceType::AuthenticatedEncryption
    );
    if needs_encrypt {
        pad_len = crypto.encrypt(
            sa,
            &iv_buf[..iv_len],
            &mut frame[data_start..data_end],
        )? as u16;
    }

    // Write Security Header
    write_security_header(
        sa,
        &iv_buf[..iv_len],
        &sn_buf[..sn_len],
        pad_len,
        &mut frame[header_end..data_start],
    )?;

    // Compute MAC if needed
    let needs_auth = matches!(
        sa.service_type,
        ServiceType::Authentication
            | ServiceType::AuthenticatedEncryption
    );
    if needs_auth {
        let mac_len = sa.mac_len as usize;
        let mut mac_buf = [0u8; MAX_MAC_SIZE];

        // Build authentication payload by applying the mask.
        // The auth payload covers: Primary Header through end
        // of Frame Data Field (before MAC).
        let auth_end = data_end;
        let auth_data = &frame[..auth_end];

        // Apply mask if provided, else use raw data
        if sa.auth_mask.len() >= auth_end {
            let mut masked =
                heapless::Vec::<u8, 2048>::new();
            masked.resize(auth_end, 0).ok();
            for i in 0..auth_end {
                masked[i] = auth_data[i] & sa.auth_mask[i];
            }
            crypto.compute_mac(
                sa,
                &masked[..auth_end],
                &mut mac_buf[..mac_len],
            )?;
        } else {
            crypto.compute_mac(
                sa,
                auth_data,
                &mut mac_buf[..mac_len],
            )?;
        }

        // Write MAC into Security Trailer
        frame[trailer_start..trailer_start + mac_len]
            .copy_from_slice(&mac_buf[..mac_len]);
    }

    // Increment sequence number for next frame
    sa.sequence_number = sa.sequence_number.wrapping_add(1);

    Ok(())
}

/// Process security on a received frame (receiving side).
///
/// Verifies the SA, checks MAC and sequence number, decrypts if
/// needed. Returns the range of the cleartext data field within
/// the frame buffer.
pub fn process_security(
    sa: &mut SecurityAssociation,
    crypto: &impl CryptoProvider,
    frame: &mut [u8],
    header_end: usize,
    data_start: usize,
    data_end: usize,
) -> Result<(), Error> {
    // Read and validate SPI
    let spi = read_spi(&frame[header_end..])?;
    if spi != sa.spi {
        return Err(Error::UnknownSpi(spi));
    }

    let trailer_start = data_end;
    let mac_len = sa.mac_len as usize;
    let trailer_end = trailer_start + mac_len;
    if frame.len() < trailer_end {
        return Err(Error::FrameTooShort);
    }

    // Parse security header and extract values we need before
    // taking a mutable borrow on frame.
    let (received_sn, iv_copy) = {
        let sec_hdr = SecurityHeader::parse(
            sa,
            &frame[header_end..data_start],
        )?;
        let sn_val = sec_hdr.sequence_number_value();
        let mut iv_buf = [0u8; MAX_IV_SIZE];
        let iv = sec_hdr.iv();
        iv_buf[..iv.len()].copy_from_slice(iv);
        (sn_val, iv_buf)
    };

    // Verify MAC if needed
    let needs_auth = matches!(
        sa.service_type,
        ServiceType::Authentication
            | ServiceType::AuthenticatedEncryption
    );
    if needs_auth {
        let mut mac_buf = [0u8; MAX_MAC_SIZE];
        let auth_end = data_end;

        if sa.auth_mask.len() >= auth_end {
            let mut masked =
                heapless::Vec::<u8, 2048>::new();
            masked.resize(auth_end, 0).ok();
            for i in 0..auth_end {
                masked[i] = frame[i] & sa.auth_mask[i];
            }
            crypto.compute_mac(
                sa,
                &masked[..auth_end],
                &mut mac_buf[..mac_len],
            )?;
        } else {
            crypto.compute_mac(
                sa,
                &frame[..auth_end],
                &mut mac_buf[..mac_len],
            )?;
        }

        // Compare received MAC with computed MAC
        let received_mac =
            &frame[trailer_start..trailer_start + mac_len];
        if received_mac != &mac_buf[..mac_len] {
            return Err(Error::MacVerificationFailed);
        }

        // Check anti-replay sequence number
        let sn_len = sa.sn_len as usize;
        if sn_len > 0 {
            let expected = sa.sequence_number;
            if received_sn < expected {
                return Err(Error::SequenceNumberRejected {
                    received: received_sn,
                    expected,
                });
            }
            let window = sa.sequence_window;
            if window > 0
                && received_sn > expected.wrapping_add(window)
            {
                return Err(Error::SequenceNumberRejected {
                    received: received_sn,
                    expected,
                });
            }
            sa.sequence_number = received_sn.wrapping_add(1);
        }
    }

    // Decrypt if needed
    let needs_decrypt = matches!(
        sa.service_type,
        ServiceType::Encryption | ServiceType::AuthenticatedEncryption
    );
    if needs_decrypt {
        let iv_len = sa.iv_len as usize;
        crypto.decrypt(
            sa,
            &iv_copy[..iv_len],
            &mut frame[data_start..data_end],
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sa() -> SecurityAssociation {
        SecurityAssociation {
            spi: 1,
            service_type: ServiceType::Authentication,
            iv_len: 4,
            sn_len: 4,
            pl_len: 0,
            mac_len: 16,
            sequence_number: 0,
            sequence_window: 100,
            auth_mask: heapless::Vec::new(),
        }
    }

    #[test]
    fn sa_sizes() {
        let sa = test_sa();
        // SPI(2) + IV(4) + SN(4) + PL(0) = 10
        assert_eq!(sa.header_size(), 10);
        assert_eq!(sa.trailer_size(), 16);
        assert_eq!(sa.overhead(), 26);
    }

    #[test]
    fn write_and_read_spi() {
        let sa = test_sa();
        let mut buf = [0u8; 64];
        let iv = [0u8; 4];
        let sn = [0u8; 4];
        write_security_header(&sa, &iv, &sn, 0, &mut buf).unwrap();

        let spi = read_spi(&buf).unwrap();
        assert_eq!(spi, 1);
    }

    #[test]
    fn reserved_spi_rejected() {
        let mut sa = test_sa();
        sa.spi = 0;
        let mut buf = [0u8; 64];
        let err = write_security_header(
            &sa,
            &[0; 4],
            &[0; 4],
            0,
            &mut buf,
        )
        .unwrap_err();
        assert_eq!(err, Error::ReservedSpi(0));

        sa.spi = 0xFFFF;
        let err = write_security_header(
            &sa,
            &[0; 4],
            &[0; 4],
            0,
            &mut buf,
        )
        .unwrap_err();
        assert_eq!(err, Error::ReservedSpi(0xFFFF));
    }

    #[test]
    fn header_roundtrip() {
        let sa = test_sa();
        let mut buf = [0u8; 64];
        let iv = [0x01, 0x02, 0x03, 0x04];
        let sn = [0x00, 0x00, 0x00, 0x05];
        let written =
            write_security_header(&sa, &iv, &sn, 0, &mut buf)
                .unwrap();
        assert_eq!(written, 10);

        let hdr = SecurityHeader::parse(&sa, &buf[..written]).unwrap();
        assert_eq!(hdr.spi(), 1);
        assert_eq!(hdr.iv(), &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(hdr.sequence_number(), &[0x00, 0x00, 0x00, 0x05]);
        assert_eq!(hdr.sequence_number_value(), 5);
        assert_eq!(hdr.pad_length(), 0);
    }

    #[test]
    fn clear_mode_apply_and_process() {
        // Simulate a frame: [5-byte fake header][sec hdr][data][mac]
        let mut sa_send = test_sa();
        let mut sa_recv = test_sa();
        let crypto = ClearModeCrypto;

        let header_end = 5;
        let data_start = header_end + sa_send.header_size(); // 15
        let data_end = data_start + 10; // 25
        let mut frame = [0u8; 64];
        // Write some fake frame header
        frame[0..5].copy_from_slice(&[0xC0, 0x00, 0x2A, 0x00, 0x28]);
        // Write some payload data
        frame[data_start..data_end]
            .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        apply_security(
            &mut sa_send,
            &crypto,
            &mut frame,
            header_end,
            data_start,
            data_end,
        )
        .unwrap();

        // SPI should be written at header_end
        assert_eq!(read_spi(&frame[header_end..]).unwrap(), 1);
        // Sequence number should have incremented
        assert_eq!(sa_send.sequence_number, 1);

        // Now process on the receive side
        process_security(
            &mut sa_recv,
            &crypto,
            &mut frame,
            header_end,
            data_start,
            data_end,
        )
        .unwrap();

        // Receiver sequence number should have advanced
        assert_eq!(sa_recv.sequence_number, 1);
        // Data should still be intact (clear mode)
        assert_eq!(
            &frame[data_start..data_end],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
    }

    #[test]
    fn encryption_only_sa() {
        let sa = SecurityAssociation {
            spi: 2,
            service_type: ServiceType::Encryption,
            iv_len: 12,
            sn_len: 0,
            pl_len: 1,
            mac_len: 0,
            sequence_number: 0,
            sequence_window: 0,
            auth_mask: heapless::Vec::new(),
        };
        // SPI(2) + IV(12) + SN(0) + PL(1) = 15
        assert_eq!(sa.header_size(), 15);
        assert_eq!(sa.trailer_size(), 0);
    }

    #[test]
    fn authenticated_encryption_sa() {
        let sa = SecurityAssociation {
            spi: 3,
            service_type: ServiceType::AuthenticatedEncryption,
            iv_len: 12,
            sn_len: 0,
            pl_len: 1,
            mac_len: 16,
            sequence_number: 0,
            sequence_window: 100,
            auth_mask: heapless::Vec::new(),
        };
        // SPI(2) + IV(12) + SN(0) + PL(1) = 15
        assert_eq!(sa.header_size(), 15);
        // MAC(16)
        assert_eq!(sa.trailer_size(), 16);
        assert_eq!(sa.overhead(), 31);
    }

    #[test]
    fn spi_read_too_short() {
        let err = read_spi(&[0x00]).unwrap_err();
        assert_eq!(err, Error::FrameTooShort);
    }

    #[test]
    fn header_with_pad_length() {
        let sa = SecurityAssociation {
            spi: 10,
            service_type: ServiceType::Encryption,
            iv_len: 0,
            sn_len: 0,
            pl_len: 2,
            mac_len: 0,
            sequence_number: 0,
            sequence_window: 0,
            auth_mask: heapless::Vec::new(),
        };
        let mut buf = [0u8; 16];
        let written =
            write_security_header(&sa, &[], &[], 42, &mut buf)
                .unwrap();
        // SPI(2) + PL(2) = 4
        assert_eq!(written, 4);

        let hdr = SecurityHeader::parse(&sa, &buf[..written]).unwrap();
        assert_eq!(hdr.spi(), 10);
        assert_eq!(hdr.pad_length(), 42);
    }
}
