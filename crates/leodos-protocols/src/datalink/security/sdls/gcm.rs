use aes_gcm::{
    aead::{
        generic_array::GenericArray, AeadInPlace, KeyInit,
    },
    Aes128Gcm, Aes256Gcm,
};

use super::{CryptoProvider, Error, SecurityAssociation};

enum Cipher {
    Aes128(Aes128Gcm),
    Aes256(Aes256Gcm),
}

/// AES-GCM authenticated encryption provider.
///
/// Supports 128-bit and 256-bit keys with 12-byte nonces
/// per CCSDS 355.0-B-2 Table 6-1. The GCM tag (16 bytes) is
/// used as the MAC in the SDLS Security Trailer.
pub struct AesGcmCrypto {
    cipher: Cipher,
}

impl AesGcmCrypto {
    /// Create a provider with a 128-bit key.
    pub fn new_128(key: &[u8; 16]) -> Self {
        Self {
            cipher: Cipher::Aes128(
                Aes128Gcm::new(key.into()),
            ),
        }
    }

    /// Create a provider with a 256-bit key.
    pub fn new_256(key: &[u8; 32]) -> Self {
        Self {
            cipher: Cipher::Aes256(
                Aes256Gcm::new(key.into()),
            ),
        }
    }
}

impl CryptoProvider for AesGcmCrypto {
    fn encrypt(
        &self,
        _sa: &SecurityAssociation,
        iv: &[u8],
        aad: &[u8],
        data: &mut [u8],
        tag_out: &mut [u8],
    ) -> Result<usize, Error> {
        if iv.len() != 12 {
            return Err(Error::CryptoError);
        }
        let nonce = GenericArray::from_slice(iv);
        let tag = match &self.cipher {
            Cipher::Aes128(c) => c
                .encrypt_in_place_detached(nonce, aad, data)
                .map_err(|_| Error::CryptoError)?,
            Cipher::Aes256(c) => c
                .encrypt_in_place_detached(nonce, aad, data)
                .map_err(|_| Error::CryptoError)?,
        };
        let len = tag_out.len().min(16);
        tag_out[..len].copy_from_slice(&tag[..len]);
        Ok(0)
    }

    fn decrypt(
        &self,
        _sa: &SecurityAssociation,
        iv: &[u8],
        aad: &[u8],
        data: &mut [u8],
        tag: &[u8],
    ) -> Result<usize, Error> {
        if iv.len() != 12 || tag.len() != 16 {
            return Err(Error::CryptoError);
        }
        let nonce = GenericArray::from_slice(iv);
        let tag_arr = GenericArray::from_slice(tag);
        match &self.cipher {
            Cipher::Aes128(c) => c
                .decrypt_in_place_detached(
                    nonce, aad, data, tag_arr,
                )
                .map_err(|_| Error::MacVerificationFailed)?,
            Cipher::Aes256(c) => c
                .decrypt_in_place_detached(
                    nonce, aad, data, tag_arr,
                )
                .map_err(|_| Error::MacVerificationFailed)?,
        };
        Ok(0)
    }

    fn compute_mac(
        &self,
        _sa: &SecurityAssociation,
        _iv: &[u8],
        _payload: &[u8],
        _mac_out: &mut [u8],
    ) -> Result<(), Error> {
        Err(Error::CryptoError)
    }
}
