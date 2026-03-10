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

mod gcm;
mod security_header;

pub use gcm::AesGcmCrypto;
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
        2 + self.iv_len as usize + self.sn_len as usize + self.pl_len as usize
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
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    /// The SPI value is reserved (0 or 65535).
    #[error("reserved SPI: {0}")]
    ReservedSpi(u16),
    /// No SA found for the given SPI.
    #[error("unknown SPI: {0}")]
    UnknownSpi(u16),
    /// The buffer is too small for the security fields.
    #[error("buffer too small: need {required} bytes, have {available} bytes")]
    BufferTooSmall {
        /// Minimum bytes needed.
        required: usize,
        /// Actual bytes available.
        available: usize,
    },
    /// The frame is too short to contain a Security Header.
    #[error("frame too short to contain Security Header")]
    FrameTooShort,
    /// MAC verification failed.
    #[error("MAC verification failed")]
    MacVerificationFailed,
    /// Anti-replay sequence number check failed.
    #[error("sequence number rejected: got {received}, expected {expected}")]
    SequenceNumberRejected {
        /// The received sequence number.
        received: u64,
        /// The expected sequence number.
        expected: u64,
    },
    /// Padding error during decryption.
    #[error("padding error")]
    PaddingError,
    /// A crypto backend error occurred.
    #[error("cryptographic operation failed")]
    CryptoError,
}

/// Trait for pluggable cryptographic backends.
///
/// Implementations provide the actual encryption, decryption, and
/// MAC computation. The SDLS framing layer is algorithm-agnostic;
/// concrete algorithms (AES-GCM, CMAC, etc.) are supplied here.
///
/// For AEAD ciphers (AES-GCM), `encrypt` and `decrypt` handle
/// both confidentiality and authentication via `aad` and `tag`.
/// For authentication-only mode (CMAC), use `compute_mac`.
pub trait CryptoProvider {
    /// Encrypt data in place (AEAD). `aad` is additional
    /// authenticated data (frame headers). The authentication
    /// tag is written to `tag_out`. Returns padding byte count
    /// (0 for GCM/CTR).
    fn encrypt(
        &self,
        sa: &SecurityAssociation,
        iv: &[u8],
        aad: &[u8],
        data: &mut [u8],
        tag_out: &mut [u8],
    ) -> Result<usize, Error>;

    /// Decrypt data in place (AEAD). Verifies the authentication
    /// tag against `aad` and ciphertext. Returns padding byte
    /// count.
    fn decrypt(
        &self,
        sa: &SecurityAssociation,
        iv: &[u8],
        aad: &[u8],
        data: &mut [u8],
        tag: &[u8],
    ) -> Result<usize, Error>;

    /// Compute a MAC for authentication-only mode. The `iv` is
    /// passed for algorithms that need it (e.g. GMAC).
    fn compute_mac(
        &self,
        sa: &SecurityAssociation,
        iv: &[u8],
        payload: &[u8],
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
        _aad: &[u8],
        _data: &mut [u8],
        _tag_out: &mut [u8],
    ) -> Result<usize, Error> {
        Ok(0)
    }

    fn decrypt(
        &self,
        _sa: &SecurityAssociation,
        _iv: &[u8],
        _aad: &[u8],
        _data: &mut [u8],
        _tag: &[u8],
    ) -> Result<usize, Error> {
        Ok(0)
    }

    fn compute_mac(
        &self,
        sa: &SecurityAssociation,
        _iv: &[u8],
        _payload: &[u8],
        mac_out: &mut [u8],
    ) -> Result<(), Error> {
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
        out[pos..pos + pl_len].copy_from_slice(&pl_bytes[start..]);
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
/// 2. Encrypts the data field if needed (AEAD for GCM)
/// 3. Computes and writes the MAC if needed
///
/// The caller must have left room for the security overhead.
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
    let mac_len = sa.mac_len as usize;
    let trailer_end = trailer_start + mac_len;
    if frame.len() < trailer_end {
        return Err(Error::BufferTooSmall {
            required: trailer_end,
            available: frame.len(),
        });
    }

    // Prepare IV and SN from sequence number
    let mut iv_buf = [0u8; MAX_IV_SIZE];
    let mut sn_buf = [0u8; MAX_SN_SIZE];
    let iv_len = sa.iv_len as usize;
    let sn_len = sa.sn_len as usize;

    if iv_len > 0 {
        let sn_bytes = sa.sequence_number.to_be_bytes();
        let start = 8usize.saturating_sub(iv_len);
        let copy_len = iv_len.min(8);
        let iv_start = iv_len.saturating_sub(8);
        iv_buf[iv_start..iv_start + copy_len].copy_from_slice(&sn_bytes[start..start + copy_len]);
    }

    if sn_len > 0 {
        let sn_bytes = sa.sequence_number.to_be_bytes();
        let start = 8 - sn_len;
        sn_buf[..sn_len].copy_from_slice(&sn_bytes[start..]);
    }

    // Write Security Header first (needed as AAD for AEAD).
    // pad_len is always 0 for GCM/CTR (CCSDS stream ciphers).
    write_security_header(
        sa,
        &iv_buf[..iv_len],
        &sn_buf[..sn_len],
        0,
        &mut frame[header_end..data_start],
    )?;

    let mut mac_buf = [0u8; MAX_MAC_SIZE];

    match sa.service_type {
        ServiceType::Authentication => {
            let auth_end = data_end;
            if sa.auth_mask.len() >= auth_end {
                let mut masked = heapless::Vec::<u8, 2048>::new();
                masked.resize(auth_end, 0).ok();
                for i in 0..auth_end {
                    masked[i] = frame[i] & sa.auth_mask[i];
                }
                crypto.compute_mac(
                    sa,
                    &iv_buf[..iv_len],
                    &masked[..auth_end],
                    &mut mac_buf[..mac_len],
                )?;
            } else {
                crypto.compute_mac(
                    sa,
                    &iv_buf[..iv_len],
                    &frame[..auth_end],
                    &mut mac_buf[..mac_len],
                )?;
            }
            frame[trailer_start..trailer_start + mac_len].copy_from_slice(&mac_buf[..mac_len]);
        }
        ServiceType::Encryption => {
            crypto.encrypt(
                sa,
                &iv_buf[..iv_len],
                &[],
                &mut frame[data_start..data_end],
                &mut [],
            )?;
        }
        ServiceType::AuthenticatedEncryption => {
            // AAD = frame header + security header (before data)
            let (aad_part, rest) = frame.split_at_mut(data_start);
            let data_len = data_end - data_start;

            if sa.auth_mask.len() >= data_start {
                let mut masked_aad = [0u8; 256];
                for i in 0..aad_part.len() {
                    masked_aad[i] = aad_part[i] & sa.auth_mask[i];
                }
                crypto.encrypt(
                    sa,
                    &iv_buf[..iv_len],
                    &masked_aad[..aad_part.len()],
                    &mut rest[..data_len],
                    &mut mac_buf[..mac_len],
                )?;
            } else {
                crypto.encrypt(
                    sa,
                    &iv_buf[..iv_len],
                    aad_part,
                    &mut rest[..data_len],
                    &mut mac_buf[..mac_len],
                )?;
            }
            rest[data_len..data_len + mac_len].copy_from_slice(&mac_buf[..mac_len]);
        }
    }

    sa.sequence_number = sa.sequence_number.wrapping_add(1);

    Ok(())
}

/// Process security on a received frame (receiving side).
///
/// Verifies authentication, checks anti-replay, and decrypts.
/// For AEAD (AES-GCM), tag verification and decryption are
/// combined in a single `decrypt` call.
pub fn process_security(
    sa: &mut SecurityAssociation,
    crypto: &impl CryptoProvider,
    frame: &mut [u8],
    header_end: usize,
    data_start: usize,
    data_end: usize,
) -> Result<(), Error> {
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

    let (received_sn, iv_copy) = {
        let sec_hdr = SecurityHeader::parse(sa, &frame[header_end..data_start])?;
        let sn_val = sec_hdr.sequence_number_value();
        let mut iv_buf = [0u8; MAX_IV_SIZE];
        let iv = sec_hdr.iv();
        iv_buf[..iv.len()].copy_from_slice(iv);
        (sn_val, iv_buf)
    };

    let iv_len = sa.iv_len as usize;
    let sn_len = sa.sn_len as usize;

    match sa.service_type {
        ServiceType::Authentication => {
            let mut mac_buf = [0u8; MAX_MAC_SIZE];
            let auth_end = data_end;

            if sa.auth_mask.len() >= auth_end {
                let mut masked = heapless::Vec::<u8, 2048>::new();
                masked.resize(auth_end, 0).ok();
                for i in 0..auth_end {
                    masked[i] = frame[i] & sa.auth_mask[i];
                }
                crypto.compute_mac(
                    sa,
                    &iv_copy[..iv_len],
                    &masked[..auth_end],
                    &mut mac_buf[..mac_len],
                )?;
            } else {
                crypto.compute_mac(
                    sa,
                    &iv_copy[..iv_len],
                    &frame[..auth_end],
                    &mut mac_buf[..mac_len],
                )?;
            }

            let received_mac = &frame[trailer_start..trailer_start + mac_len];
            if received_mac != &mac_buf[..mac_len] {
                return Err(Error::MacVerificationFailed);
            }

            if sn_len > 0 {
                check_sequence_number(sa, received_sn)?;
            }
        }
        ServiceType::Encryption => {
            crypto.decrypt(
                sa,
                &iv_copy[..iv_len],
                &[],
                &mut frame[data_start..data_end],
                &[],
            )?;
        }
        ServiceType::AuthenticatedEncryption => {
            // Copy tag from trailer before splitting
            let mut tag = [0u8; MAX_MAC_SIZE];
            tag[..mac_len].copy_from_slice(&frame[trailer_start..trailer_start + mac_len]);

            // AEAD decrypt + tag verification
            let (aad_part, rest) = frame.split_at_mut(data_start);
            let data_len = data_end - data_start;

            if sa.auth_mask.len() >= data_start {
                let mut masked_aad = [0u8; 256];
                for i in 0..aad_part.len() {
                    masked_aad[i] = aad_part[i] & sa.auth_mask[i];
                }
                crypto.decrypt(
                    sa,
                    &iv_copy[..iv_len],
                    &masked_aad[..aad_part.len()],
                    &mut rest[..data_len],
                    &tag[..mac_len],
                )?;
            } else {
                crypto.decrypt(
                    sa,
                    &iv_copy[..iv_len],
                    aad_part,
                    &mut rest[..data_len],
                    &tag[..mac_len],
                )?;
            }

            if sn_len > 0 {
                check_sequence_number(sa, received_sn)?;
            }
        }
    }

    Ok(())
}

fn check_sequence_number(sa: &mut SecurityAssociation, received: u64) -> Result<(), Error> {
    let expected = sa.sequence_number;
    if received < expected {
        return Err(Error::SequenceNumberRejected { received, expected });
    }
    let window = sa.sequence_window;
    if window > 0 && received > expected.wrapping_add(window) {
        return Err(Error::SequenceNumberRejected { received, expected });
    }
    sa.sequence_number = received.wrapping_add(1);
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
        let err = write_security_header(&sa, &[0; 4], &[0; 4], 0, &mut buf).unwrap_err();
        assert_eq!(err, Error::ReservedSpi(0));

        sa.spi = 0xFFFF;
        let err = write_security_header(&sa, &[0; 4], &[0; 4], 0, &mut buf).unwrap_err();
        assert_eq!(err, Error::ReservedSpi(0xFFFF));
    }

    #[test]
    fn header_roundtrip() {
        let sa = test_sa();
        let mut buf = [0u8; 64];
        let iv = [0x01, 0x02, 0x03, 0x04];
        let sn = [0x00, 0x00, 0x00, 0x05];
        let written = write_security_header(&sa, &iv, &sn, 0, &mut buf).unwrap();
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
        frame[data_start..data_end].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

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
        let written = write_security_header(&sa, &[], &[], 42, &mut buf).unwrap();
        // SPI(2) + PL(2) = 4
        assert_eq!(written, 4);

        let hdr = SecurityHeader::parse(&sa, &buf[..written]).unwrap();
        assert_eq!(hdr.spi(), 10);
        assert_eq!(hdr.pad_length(), 42);
    }

    fn aes_gcm_sa() -> SecurityAssociation {
        SecurityAssociation {
            spi: 5,
            service_type: ServiceType::AuthenticatedEncryption,
            iv_len: 12,
            sn_len: 0,
            pl_len: 0,
            mac_len: 16,
            sequence_number: 1,
            sequence_window: 100,
            auth_mask: heapless::Vec::new(),
        }
    }

    #[test]
    fn aes_gcm_128_roundtrip() {
        let key = [0x42u8; 16];
        let crypto = AesGcmCrypto::new_128(&key);
        let sa = aes_gcm_sa();

        let iv = [0u8; 12];
        let aad = [0xC0, 0x00, 0x2A];
        let original = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut data = original;
        let mut tag = [0u8; 16];

        crypto.encrypt(&sa, &iv, &aad, &mut data, &mut tag).unwrap();
        assert_ne!(data, original);

        crypto.decrypt(&sa, &iv, &aad, &mut data, &tag).unwrap();
        assert_eq!(data, original);
    }

    #[test]
    fn aes_gcm_256_roundtrip() {
        let key = [0x7Fu8; 32];
        let crypto = AesGcmCrypto::new_256(&key);
        let sa = aes_gcm_sa();

        let iv = [0u8; 12];
        let aad = b"header";
        let original = *b"secret!!";
        let mut data = original;
        let mut tag = [0u8; 16];

        crypto.encrypt(&sa, &iv, aad, &mut data, &mut tag).unwrap();
        crypto.decrypt(&sa, &iv, aad, &mut data, &tag).unwrap();
        assert_eq!(data, original);
    }

    #[test]
    fn aes_gcm_tampered_ciphertext() {
        let key = [0x42u8; 16];
        let crypto = AesGcmCrypto::new_128(&key);
        let sa = aes_gcm_sa();

        let iv = [0u8; 12];
        let aad = [0xC0];
        let mut data = [1u8, 2, 3, 4];
        let mut tag = [0u8; 16];

        crypto.encrypt(&sa, &iv, &aad, &mut data, &mut tag).unwrap();

        data[0] ^= 0xFF; // tamper
        let err = crypto.decrypt(&sa, &iv, &aad, &mut data, &tag).unwrap_err();
        assert_eq!(err, Error::MacVerificationFailed);
    }

    #[test]
    fn aes_gcm_tampered_aad() {
        let key = [0x42u8; 16];
        let crypto = AesGcmCrypto::new_128(&key);
        let sa = aes_gcm_sa();

        let iv = [0u8; 12];
        let aad = [0xC0];
        let mut data = [1u8, 2, 3, 4];
        let mut tag = [0u8; 16];

        crypto.encrypt(&sa, &iv, &aad, &mut data, &mut tag).unwrap();

        let bad_aad = [0xC1]; // tampered
        let err = crypto
            .decrypt(&sa, &iv, &bad_aad, &mut data, &tag)
            .unwrap_err();
        assert_eq!(err, Error::MacVerificationFailed);
    }

    #[test]
    fn aes_gcm_apply_process_roundtrip() {
        let key = [0xABu8; 16];
        let crypto = AesGcmCrypto::new_128(&key);

        let mut sa_send = aes_gcm_sa();
        let mut sa_recv = aes_gcm_sa();

        // Frame layout:
        // [5B header][14B sec hdr][10B data][16B MAC]
        // sec hdr = SPI(2) + IV(12) + SN(0) + PL(0) = 14
        let header_end = 5;
        let data_start = header_end + sa_send.header_size();
        let data_end = data_start + 10;
        let total = data_end + sa_send.trailer_size();
        let mut frame = [0u8; 64];

        // Fake frame header
        frame[0..5].copy_from_slice(&[0xC0, 0x00, 0x2A, 0x00, 0x28]);
        // Payload
        frame[data_start..data_end].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        apply_security(
            &mut sa_send,
            &crypto,
            &mut frame,
            header_end,
            data_start,
            data_end,
        )
        .unwrap();

        assert_eq!(sa_send.sequence_number, 2);
        // Data should be encrypted (not cleartext)
        assert_ne!(
            &frame[data_start..data_end],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
        // MAC should be non-zero
        assert_ne!(&frame[data_end..total], &[0u8; 16]);

        // Process on receive side
        process_security(
            &mut sa_recv,
            &crypto,
            &mut frame,
            header_end,
            data_start,
            data_end,
        )
        .unwrap();

        // Data should be decrypted back
        assert_eq!(
            &frame[data_start..data_end],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
    }

    #[test]
    fn aes_gcm_detect_tampered_frame() {
        let key = [0xABu8; 16];
        let crypto = AesGcmCrypto::new_128(&key);

        let mut sa_send = aes_gcm_sa();
        let mut sa_recv = aes_gcm_sa();

        let header_end = 5;
        let data_start = header_end + sa_send.header_size();
        let data_end = data_start + 10;
        let mut frame = [0u8; 64];

        frame[0..5].copy_from_slice(&[0xC0, 0x00, 0x2A, 0x00, 0x28]);
        frame[data_start..data_end].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        apply_security(
            &mut sa_send,
            &crypto,
            &mut frame,
            header_end,
            data_start,
            data_end,
        )
        .unwrap();

        // Tamper with encrypted data
        frame[data_start] ^= 0xFF;

        let err = process_security(
            &mut sa_recv,
            &crypto,
            &mut frame,
            header_end,
            data_start,
            data_end,
        )
        .unwrap_err();
        assert_eq!(err, Error::MacVerificationFailed);
    }
}
