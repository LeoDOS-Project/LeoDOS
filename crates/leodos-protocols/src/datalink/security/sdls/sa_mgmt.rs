//! Security Association lifecycle management (CCSDS 355.1-B-1).
//!
//! SA state machine:
//!   UNKEYED → (rekey) → KEYED → (start) → OPERATIONAL
//!   OPERATIONAL → (stop) → KEYED
//!   KEYED → (expire) → UNKEYED
//!   Any → (delete) → removed

use super::Error;
use super::SecurityAssociation;
use super::ServiceType;

/// SA lifecycle state.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SaState {
    /// Created but no keys assigned.
    Unkeyed,
    /// Keys assigned, ready to start.
    Keyed,
    /// Active in frame security processing.
    Operational,
}

/// A managed Security Association with lifecycle state.
pub struct ManagedSa {
    /// The underlying SA parameters.
    pub sa: SecurityAssociation,
    /// Current lifecycle state.
    pub state: SaState,
    /// Assigned encryption key ID (if keyed).
    pub key_id: Option<u16>,
}

/// Maximum number of managed SAs.
pub const MAX_SAS: usize = 64;

impl ManagedSa {
    /// Creates a new SA in UNKEYED state.
    pub fn new(spi: u16, service_type: ServiceType) -> Result<Self, Error> {
        if spi == 0 || spi == 0xFFFF {
            return Err(Error::ReservedSpi(spi));
        }
        Ok(Self {
            sa: SecurityAssociation {
                spi,
                service_type,
                iv_len: 0,
                sn_len: 0,
                pl_len: 0,
                mac_len: 0,
                sequence_number: 0,
                sequence_window: 0,
                auth_mask: heapless::Vec::new(),
            },
            state: SaState::Unkeyed,
            key_id: None,
        })
    }

    /// Assign a key to this SA (UNKEYED → KEYED).
    ///
    /// Per Section 3.3.3.3.4: SA must be in the Unkeyed state.
    pub fn rekey(&mut self, key_id: u16) -> Result<(), Error> {
        (self.state == SaState::Unkeyed)
            .then(|| {
                self.key_id = Some(key_id);
                self.state = SaState::Keyed;
            })
            .ok_or(Error::InvalidSaState)
    }

    /// Activate the SA for operations (KEYED → OPERATIONAL).
    pub fn start(&mut self) -> Result<(), Error> {
        (self.state == SaState::Keyed)
            .then(|| self.state = SaState::Operational)
            .ok_or(Error::InvalidSaState)
    }

    /// Deactivate the SA (OPERATIONAL → KEYED).
    pub fn stop(&mut self) -> Result<(), Error> {
        (self.state == SaState::Operational)
            .then(|| self.state = SaState::Keyed)
            .ok_or(Error::InvalidSaState)
    }

    /// Expire the SA, removing its key association (KEYED → UNKEYED).
    ///
    /// Per Section 3.3.3.4.2: SA must be in the Keyed state.
    pub fn expire(&mut self) -> Result<(), Error> {
        (self.state == SaState::Keyed)
            .then(|| {
                self.key_id = None;
                self.state = SaState::Unkeyed;
            })
            .ok_or(Error::InvalidSaState)
    }

    /// Set the Anti-Replay Sequence Number.
    pub fn set_arsn(&mut self, arsn: u64) {
        self.sa.sequence_number = arsn;
    }

    /// Read the current ARSN.
    pub fn read_arsn(&self) -> u64 {
        self.sa.sequence_number
    }
}

/// A table of managed Security Associations.
pub struct SaTable {
    entries: heapless::Vec<ManagedSa, MAX_SAS>,
}

impl SaTable {
    /// Creates an empty SA table.
    pub fn new() -> Self {
        Self {
            entries: heapless::Vec::new(),
        }
    }

    /// Creates a new SA and adds it to the table.
    pub fn create(&mut self, spi: u16, service_type: ServiceType) -> Result<(), Error> {
        if self.find(spi).is_some() {
            return Err(Error::DuplicateSpi(spi));
        }
        let sa = ManagedSa::new(spi, service_type)?;
        self.entries.push(sa).map_err(|_| Error::SaTableFull)
    }

    /// Deletes an SA from the table.
    ///
    /// Per Section 3.3.3.6.2: SA must be in the Unkeyed state.
    pub fn delete(&mut self, spi: u16) -> Result<(), Error> {
        let pos = self
            .entries
            .iter()
            .position(|e| e.sa.spi == spi)
            .ok_or(Error::UnknownSpi(spi))?;
        if self.entries[pos].state != SaState::Unkeyed {
            return Err(Error::InvalidSaState);
        }
        self.entries.swap_remove(pos);
        Ok(())
    }

    /// Finds an SA by SPI.
    pub fn find(&self, spi: u16) -> Option<&ManagedSa> {
        self.entries.iter().find(|e| e.sa.spi == spi)
    }

    /// Finds an SA by SPI (mutable).
    pub fn find_mut(&mut self, spi: u16) -> Option<&mut ManagedSa> {
        self.entries.iter_mut().find(|e| e.sa.spi == spi)
    }

    /// Rekeys an SA.
    pub fn rekey(&mut self, spi: u16, key_id: u16) -> Result<(), Error> {
        self.find_mut(spi)
            .ok_or(Error::UnknownSpi(spi))?
            .rekey(key_id)
    }

    /// Starts an SA.
    pub fn start(&mut self, spi: u16) -> Result<(), Error> {
        self.find_mut(spi)
            .ok_or(Error::UnknownSpi(spi))?
            .start()
    }

    /// Stops an SA.
    pub fn stop(&mut self, spi: u16) -> Result<(), Error> {
        self.find_mut(spi)
            .ok_or(Error::UnknownSpi(spi))?
            .stop()
    }

    /// Expires an SA.
    pub fn expire(&mut self, spi: u16) -> Result<(), Error> {
        self.find_mut(spi)
            .ok_or(Error::UnknownSpi(spi))?
            .expire()
    }

    /// Sets the ARSN for an SA.
    pub fn set_arsn(&mut self, spi: u16, arsn: u64) -> Result<(), Error> {
        self.find_mut(spi)
            .ok_or(Error::UnknownSpi(spi))?
            .set_arsn(arsn);
        Ok(())
    }

    /// Reads the ARSN for an SA.
    pub fn read_arsn(&self, spi: u16) -> Result<u64, Error> {
        Ok(self.find(spi).ok_or(Error::UnknownSpi(spi))?.read_arsn())
    }

    /// Returns an iterator over all SAs with their states.
    pub fn iter(&self) -> impl Iterator<Item = &ManagedSa> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sa_lifecycle() {
        let mut sa = ManagedSa::new(1, ServiceType::AuthenticatedEncryption).unwrap();
        assert_eq!(sa.state, SaState::Unkeyed);

        assert!(sa.start().is_err());
        assert!(sa.stop().is_err());
        assert!(sa.expire().is_err());

        sa.rekey(10).unwrap();
        assert_eq!(sa.state, SaState::Keyed);
        assert_eq!(sa.key_id, Some(10));

        // Can't rekey from Keyed — must be Unkeyed
        assert!(sa.rekey(20).is_err());

        sa.start().unwrap();
        assert_eq!(sa.state, SaState::Operational);
        assert!(sa.start().is_err());

        // Can't expire from Operational — must stop first
        assert!(sa.expire().is_err());

        sa.stop().unwrap();
        assert_eq!(sa.state, SaState::Keyed);

        sa.expire().unwrap();
        assert_eq!(sa.state, SaState::Unkeyed);
        assert_eq!(sa.key_id, None);
    }

    #[test]
    fn sa_table_crud() {
        let mut table = SaTable::new();
        table
            .create(1, ServiceType::Authentication)
            .unwrap();
        table
            .create(2, ServiceType::Encryption)
            .unwrap();

        assert!(table
            .create(1, ServiceType::Authentication)
            .is_err());

        table.rekey(1, 100).unwrap();
        table.start(1).unwrap();
        assert_eq!(
            table.find(1).unwrap().state,
            SaState::Operational
        );

        // Can't delete from Operational
        assert!(table.delete(1).is_err());

        table.stop(1).unwrap();
        // Can't delete from Keyed either
        assert!(table.delete(1).is_err());

        table.expire(1).unwrap();
        // Now in Unkeyed — can delete
        table.delete(1).unwrap();
        assert!(table.find(1).is_none());
    }

    #[test]
    fn sa_table_arsn() {
        let mut table = SaTable::new();
        table
            .create(5, ServiceType::AuthenticatedEncryption)
            .unwrap();

        table.set_arsn(5, 42).unwrap();
        assert_eq!(table.read_arsn(5).unwrap(), 42);

        assert!(table.set_arsn(99, 0).is_err());
    }

    #[test]
    fn reserved_spi_rejected() {
        assert!(ManagedSa::new(0, ServiceType::Authentication).is_err());
        assert!(ManagedSa::new(0xFFFF, ServiceType::Authentication).is_err());
    }
}
