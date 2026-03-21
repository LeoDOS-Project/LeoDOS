//! Key management for SDLS Extended Procedures (CCSDS 355.1-B-1).
//!
//! Keys follow a state machine:
//!   PREACTIVE → ACTIVE → DEACTIVATED → DESTROYED
//!
//! Master keys decrypt session keys delivered via OTAR.
//! Session keys are used for frame encryption/authentication.

use super::CryptoProvider;
use super::Error;

/// Maximum key length in bytes (AES-256).
pub const MAX_KEY_LEN: usize = 32;

/// Maximum number of keys in a key ring.
pub const MAX_KEYS: usize = 128;

/// Key state in the lifecycle.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum KeyState {
    /// Loaded but not yet activated.
    Preactive,
    /// Available for cryptographic operations.
    Active,
    /// Disabled, no longer usable for new operations.
    Deactivated,
    /// Cryptographically wiped.
    Destroyed,
}

/// A managed cryptographic key.
#[derive(Clone)]
pub struct ManagedKey {
    /// Key identifier.
    pub key_id: u16,
    /// Current lifecycle state.
    pub state: KeyState,
    /// Key material (AES-128 = 16 bytes, AES-256 = 32 bytes).
    pub material: [u8; MAX_KEY_LEN],
    /// Actual key length in bytes.
    pub key_len: u8,
}

impl ManagedKey {
    /// Creates a new key in PREACTIVE state.
    pub fn new(key_id: u16, material: &[u8]) -> Result<Self, Error> {
        if material.is_empty() || material.len() > MAX_KEY_LEN {
            return Err(Error::InvalidKeyLength);
        }
        let mut key = Self {
            key_id,
            state: KeyState::Preactive,
            material: [0u8; MAX_KEY_LEN],
            key_len: material.len() as u8,
        };
        key.material[..material.len()].copy_from_slice(material);
        Ok(key)
    }

    /// Transition from PREACTIVE to ACTIVE.
    pub fn activate(&mut self) -> Result<(), Error> {
        (self.state == KeyState::Preactive)
            .then(|| self.state = KeyState::Active)
            .ok_or(Error::InvalidKeyState)
    }

    /// Transition from ACTIVE to DEACTIVATED.
    pub fn deactivate(&mut self) -> Result<(), Error> {
        (self.state == KeyState::Active)
            .then(|| self.state = KeyState::Deactivated)
            .ok_or(Error::InvalidKeyState)
    }

    /// Cryptographically destroy the key material.
    pub fn destroy(&mut self) {
        self.material = [0u8; MAX_KEY_LEN];
        self.key_len = 0;
        self.state = KeyState::Destroyed;
    }

    /// Returns the key material slice.
    pub fn material(&self) -> &[u8] {
        &self.material[..self.key_len as usize]
    }
}

/// A key ring holding master and session keys.
pub struct KeyRing {
    keys: heapless::Vec<ManagedKey, MAX_KEYS>,
}

impl KeyRing {
    /// Creates an empty key ring.
    pub fn new() -> Self {
        Self {
            keys: heapless::Vec::new(),
        }
    }

    /// Loads a key into the ring in PREACTIVE state.
    pub fn load(&mut self, key_id: u16, material: &[u8]) -> Result<(), Error> {
        if self.find(key_id).is_some() {
            return Err(Error::DuplicateKeyId);
        }
        let key = ManagedKey::new(key_id, material)?;
        self.keys.push(key).map_err(|_| Error::KeyRingFull)
    }

    /// Finds a key by ID.
    pub fn find(&self, key_id: u16) -> Option<&ManagedKey> {
        self.keys.iter().find(|k| k.key_id == key_id)
    }

    /// Finds a key by ID (mutable).
    pub fn find_mut(&mut self, key_id: u16) -> Option<&mut ManagedKey> {
        self.keys.iter_mut().find(|k| k.key_id == key_id)
    }

    /// Activates a key by ID.
    pub fn activate(&mut self, key_id: u16) -> Result<(), Error> {
        self.find_mut(key_id)
            .ok_or(Error::UnknownKeyId(key_id))?
            .activate()
    }

    /// Deactivates a key by ID.
    pub fn deactivate(&mut self, key_id: u16) -> Result<(), Error> {
        self.find_mut(key_id)
            .ok_or(Error::UnknownKeyId(key_id))?
            .deactivate()
    }

    /// Destroys a key by ID.
    pub fn destroy(&mut self, key_id: u16) -> Result<(), Error> {
        self.find_mut(key_id)
            .ok_or(Error::UnknownKeyId(key_id))?
            .destroy();
        Ok(())
    }

    /// Processes an OTAR delivery: decrypts session keys using a
    /// master key and loads them into the ring.
    ///
    /// `master_key_id` — the key used to decrypt the encrypted key block.
    /// `iv` — initialization vector for decryption.
    /// `encrypted_block` — encrypted payload containing (key_id, key_material) pairs.
    /// `crypto` — cryptographic backend for decryption.
    /// `sa` — security association for the master key context.
    pub fn otar(
        &mut self,
        master_key_id: u16,
        iv: &[u8],
        encrypted_block: &mut [u8],
        tag: &[u8],
        crypto: &impl CryptoProvider,
        sa: &super::SecurityAssociation,
    ) -> Result<usize, Error> {
        let master = self
            .find(master_key_id)
            .ok_or(Error::UnknownKeyId(master_key_id))?;
        if master.state != KeyState::Active {
            return Err(Error::InvalidKeyState);
        }

        crypto.decrypt(sa, iv, &[], encrypted_block, tag)?;

        let mut pos = 0;
        let mut count = 0;
        while pos + 4 <= encrypted_block.len() {
            let key_id = u16::from_be_bytes([encrypted_block[pos], encrypted_block[pos + 1]]);
            let key_len = u16::from_be_bytes([encrypted_block[pos + 2], encrypted_block[pos + 3]])
                as usize;
            pos += 4;
            if pos + key_len > encrypted_block.len() {
                break;
            }
            let material = &encrypted_block[pos..pos + key_len];
            self.load(key_id, material)?;
            pos += key_len;
            count += 1;
        }

        Ok(count)
    }

    /// Returns an iterator over all keys for inventory queries.
    pub fn inventory(&self) -> impl Iterator<Item = (u16, KeyState)> + '_ {
        self.keys.iter().map(|k| (k.key_id, k.state))
    }

    /// Verifies a key via challenge-response.
    ///
    /// Encrypts `challenge` with the specified key and returns
    /// the encrypted result in `response_out`.
    pub fn verify(
        &self,
        key_id: u16,
        iv: &[u8],
        challenge: &[u8],
        response_out: &mut [u8],
        tag_out: &mut [u8],
        crypto: &impl CryptoProvider,
        sa: &super::SecurityAssociation,
    ) -> Result<(), Error> {
        let key = self.find(key_id).ok_or(Error::UnknownKeyId(key_id))?;
        if key.state != KeyState::Active {
            return Err(Error::InvalidKeyState);
        }
        let len = challenge.len().min(response_out.len());
        response_out[..len].copy_from_slice(&challenge[..len]);
        crypto.encrypt(sa, iv, &[], &mut response_out[..len], tag_out)
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_lifecycle() {
        let mut key = ManagedKey::new(1, &[0xAA; 16]).unwrap();
        assert_eq!(key.state, KeyState::Preactive);
        assert_eq!(key.material(), &[0xAA; 16]);

        assert!(key.deactivate().is_err());
        key.activate().unwrap();
        assert_eq!(key.state, KeyState::Active);

        assert!(key.activate().is_err());
        key.deactivate().unwrap();
        assert_eq!(key.state, KeyState::Deactivated);

        key.destroy();
        assert_eq!(key.state, KeyState::Destroyed);
        assert_eq!(key.key_len, 0);
    }

    #[test]
    fn keyring_load_activate() {
        let mut ring = KeyRing::new();
        ring.load(1, &[0x42; 16]).unwrap();
        ring.load(2, &[0x7F; 32]).unwrap();

        assert!(ring.load(1, &[0x00; 16]).is_err());

        ring.activate(1).unwrap();
        assert_eq!(ring.find(1).unwrap().state, KeyState::Active);

        assert!(ring.activate(99).is_err());
    }

    #[test]
    fn keyring_destroy() {
        let mut ring = KeyRing::new();
        ring.load(5, &[0xFF; 16]).unwrap();
        ring.destroy(5).unwrap();
        assert_eq!(ring.find(5).unwrap().state, KeyState::Destroyed);
    }

    #[test]
    fn keyring_inventory() {
        let mut ring = KeyRing::new();
        ring.load(1, &[0x11; 16]).unwrap();
        ring.load(2, &[0x22; 16]).unwrap();
        ring.activate(1).unwrap();

        let inv: heapless::Vec<(u16, KeyState), MAX_KEYS> = ring.inventory().collect();
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0], (1, KeyState::Active));
        assert_eq!(inv[1], (2, KeyState::Preactive));
    }

    #[test]
    fn invalid_key_length() {
        assert!(ManagedKey::new(1, &[]).is_err());
        assert!(ManagedKey::new(1, &[0u8; 33]).is_err());
    }
}
