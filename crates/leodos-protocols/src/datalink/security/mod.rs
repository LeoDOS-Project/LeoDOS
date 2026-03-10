//! Frame-level encryption and authentication for the datalink layer.
//!
//! Provides the [`SecurityProcessor`] trait, [`SdlsProcessor`] for
//! CCSDS 355.0-B-2 frame-level crypto, and [`NoSecurity`] as a
//! passthrough.

use core::convert::Infallible;

use sdls::{CryptoProvider, SecurityAssociation};

/// Space Data Link Security (CCSDS 355.0-B-2).
pub mod sdls;

/// Applies or removes security (encryption/authentication) on frames.
pub trait SecurityProcessor {
    /// Error type for security operations.
    type Error;
    /// Applies security (encrypt/authenticate) to a frame in-place.
    fn apply(
        &mut self,
        frame: &mut [u8],
    ) -> Result<usize, Self::Error>;
    /// Removes security (decrypt/verify) from a frame in-place.
    fn process(
        &mut self,
        frame: &mut [u8],
    ) -> Result<usize, Self::Error>;
}

/// SDLS-based security processor (CCSDS 355.0-B-2).
///
/// Wraps a [`SecurityAssociation`] and [`CryptoProvider`] to
/// implement in-place frame encryption and authentication.
///
/// The `header_end` offset tells the processor where the transfer
/// frame header ends (5 for TC, 6 for TM). The security header
/// is inserted at that offset, followed by the encrypted data
/// field and optional MAC trailer.
pub struct SdlsProcessor<C> {
    sa: SecurityAssociation,
    crypto: C,
    header_end: usize,
}

impl<C: CryptoProvider> SdlsProcessor<C> {
    /// Creates a new SDLS processor.
    ///
    /// `header_end` is the byte offset where the transfer frame
    /// header ends (5 for TC, 6 for TM).
    pub fn new(
        sa: SecurityAssociation,
        crypto: C,
        header_end: usize,
    ) -> Self {
        Self {
            sa,
            crypto,
            header_end,
        }
    }

    /// Returns a reference to the security association.
    pub fn sa(&self) -> &SecurityAssociation {
        &self.sa
    }

    /// Returns a mutable reference to the security association.
    pub fn sa_mut(&mut self) -> &mut SecurityAssociation {
        &mut self.sa
    }
}

impl<C: CryptoProvider> SecurityProcessor for SdlsProcessor<C> {
    type Error = sdls::Error;

    fn apply(
        &mut self,
        frame: &mut [u8],
    ) -> Result<usize, Self::Error> {
        let data_start = self.header_end + self.sa.header_size();
        let data_end = frame.len() - self.sa.trailer_size();
        sdls::apply_security(
            &mut self.sa,
            &self.crypto,
            frame,
            self.header_end,
            data_start,
            data_end,
        )?;
        Ok(frame.len())
    }

    fn process(
        &mut self,
        frame: &mut [u8],
    ) -> Result<usize, Self::Error> {
        let data_start = self.header_end + self.sa.header_size();
        let data_end = frame.len() - self.sa.trailer_size();
        sdls::process_security(
            &mut self.sa,
            &self.crypto,
            frame,
            self.header_end,
            data_start,
            data_end,
        )?;
        Ok(frame.len())
    }
}

/// No-op security processor (passthrough).
///
/// Leaves frames untouched. Use when SDLS is not configured.
pub struct NoSecurity;

impl SecurityProcessor for NoSecurity {
    type Error = Infallible;

    fn apply(
        &mut self,
        frame: &mut [u8],
    ) -> Result<usize, Self::Error> {
        Ok(frame.len())
    }

    fn process(
        &mut self,
        frame: &mut [u8],
    ) -> Result<usize, Self::Error> {
        Ok(frame.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdls::{
        AesGcmCrypto, ClearModeCrypto, SecurityAssociation,
        ServiceType,
    };

    fn auth_sa() -> SecurityAssociation {
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

    fn aead_sa() -> SecurityAssociation {
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
    fn no_security_passthrough() {
        let mut processor = NoSecurity;
        let mut frame = [0xAA; 32];
        let len = processor.apply(&mut frame).unwrap();
        assert_eq!(len, 32);
        assert!(frame.iter().all(|&b| b == 0xAA));
    }

    #[test]
    fn sdls_clear_mode_roundtrip() {
        let sa = auth_sa();
        let header_end = 5;
        let data_start = header_end + sa.header_size();
        let data_end = data_start + 10;
        let total = data_end + sa.trailer_size();

        let mut send =
            SdlsProcessor::new(sa.clone(), ClearModeCrypto, header_end);
        let mut recv =
            SdlsProcessor::new(sa, ClearModeCrypto, header_end);

        let mut frame = [0u8; 64];
        frame[0..5].copy_from_slice(&[0xC0, 0x00, 0x2A, 0x00, 0x28]);
        frame[data_start..data_end]
            .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

        let len = send.apply(&mut frame[..total]).unwrap();
        assert_eq!(len, total);

        let len = recv.process(&mut frame[..total]).unwrap();
        assert_eq!(len, total);
        assert_eq!(
            &frame[data_start..data_end],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
    }

    #[test]
    fn sdls_aes_gcm_roundtrip() {
        let sa = aead_sa();
        let header_end = 5;
        let data_start = header_end + sa.header_size();
        let data_end = data_start + 8;
        let total = data_end + sa.trailer_size();

        let key = [0xABu8; 16];

        let mut send = SdlsProcessor::new(
            sa.clone(),
            AesGcmCrypto::new_128(&key),
            header_end,
        );
        let mut recv = SdlsProcessor::new(
            sa,
            AesGcmCrypto::new_128(&key),
            header_end,
        );

        let mut frame = [0u8; 64];
        frame[0..5].copy_from_slice(&[0xC0, 0x00, 0x2A, 0x00, 0x28]);
        let original = [1u8, 2, 3, 4, 5, 6, 7, 8];
        frame[data_start..data_end].copy_from_slice(&original);

        send.apply(&mut frame[..total]).unwrap();
        assert_ne!(&frame[data_start..data_end], &original);

        recv.process(&mut frame[..total]).unwrap();
        assert_eq!(&frame[data_start..data_end], &original);
    }
}
