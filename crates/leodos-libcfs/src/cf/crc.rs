//! CFDP CRC calculation types and functions.

use crate::ffi;

/// CRC state object for computing CFDP checksums.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct Crc(pub(crate) ffi::CF_Crc_t);

impl Crc {
    /// Creates a new CRC state and initializes it for computation.
    pub fn new() -> Self {
        let mut crc = Self::default();
        crc.start();
        crc
    }

    /// Initializes or resets the CRC state for a new computation.
    pub fn start(&mut self) {
        unsafe { ffi::CF_CRC_Start(&mut self.0) }
    }

    /// Digests a chunk of data into the CRC computation.
    pub fn digest(&mut self, data: &[u8]) {
        unsafe { ffi::CF_CRC_Digest(&mut self.0, data.as_ptr(), data.len()) }
    }

    /// Finalizes the CRC computation.
    pub fn finalize(&mut self) {
        unsafe { ffi::CF_CRC_Finalize(&mut self.0) }
    }

    /// Returns the computed CRC result.
    pub fn result(&self) -> u32 {
        self.0.result
    }

    /// Computes the CRC of a complete data buffer.
    pub fn compute(data: &[u8]) -> u32 {
        let mut crc = Self::new();
        crc.digest(data);
        crc.finalize();
        crc.result()
    }
}
