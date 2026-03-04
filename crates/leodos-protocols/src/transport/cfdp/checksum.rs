/// A synchronous, stateful checksum calculator.
pub trait CfdpChecksum: Send {
    /// Updates the internal state with a new chunk of data.
    fn update(&mut self, data: &[u8]);

    /// Consumes the calculator and returns the final 32-bit checksum.
    fn finalize(self) -> u32;
}

/// CCSDS Modular Checksum implementation (sum of 32-bit words).
pub mod modular {
    use heapless::Vec;

    use crate::transport::cfdp::checksum::CfdpChecksum;

    /// A CCSDS Modular Checksum calculator (sum of big-endian 32-bit words).
    pub struct ModularChecksum {
        /// Running 32-bit checksum accumulator.
        sum: u32,
        /// Buffer for incomplete trailing bytes not yet forming a full word.
        pending: Vec<u8, 4>,
    }

    impl ModularChecksum {
        /// Creates a new modular checksum calculator with an initial sum of zero.
        pub fn new() -> Self {
            Self {
                sum: 0,
                pending: Vec::new(),
            }
        }
    }

    impl CfdpChecksum for ModularChecksum {
        fn update(&mut self, data: &[u8]) {
            let mut cursor = 0;

            // 1. Fill pending buffer
            while !self.pending.is_full() && cursor < data.len() {
                let _ = self.pending.push(data[cursor]);
                cursor += 1;
            }

            // 2. Process pending word
            if self.pending.is_full() {
                let word = u32::from_be_bytes(self.pending.as_slice().try_into().unwrap());
                self.sum = self.sum.wrapping_add(word);
                self.pending.clear();
            }

            // 3. Process main body
            let remaining = &data[cursor..];
            let mut chunks = remaining.chunks_exact(4);
            for chunk in chunks.by_ref() {
                let word = u32::from_be_bytes(chunk.try_into().unwrap());
                self.sum = self.sum.wrapping_add(word);
            }

            // 4. Save remainder
            for b in chunks.remainder() {
                let _ = self.pending.push(*b);
            }
        }

        fn finalize(self) -> u32 {
            let mut final_sum = self.sum;
            if !self.pending.is_empty() {
                let mut padded = [0u8; 4];
                padded[..self.pending.len()].copy_from_slice(&self.pending);
                let word = u32::from_be_bytes(padded);
                final_sum = final_sum.wrapping_add(word);
            }
            final_sum
        }
    }
}

/// CRC-based checksum implementations (CRC-32C and IEEE 802.3).
pub mod crc {
    use crc::CRC_32_ISCSI;
    use crc::CRC_32_ISO_HDLC;
    use crc::Crc;
    use crc::Digest;

    use crate::transport::cfdp::checksum::CfdpChecksum;

    static CRC_CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
    static CRC_IEEE: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

    /// A CRC-32 checksum calculator supporting CRC-32C and IEEE variants.
    pub struct CrcChecksum {
        /// The underlying CRC digest state.
        digest: Digest<'static, u32>,
    }

    impl CrcChecksum {
        /// Creates a calculator for CRC-32C (Castagnoli).
        pub fn crc32c() -> Self {
            Self {
                digest: CRC_CASTAGNOLI.digest(),
            }
        }

        /// Creates a calculator for IEEE 802.3 CRC-32 (also used for Proximity-1).
        pub fn ieee() -> Self {
            Self {
                digest: CRC_IEEE.digest(),
            }
        }
    }
    impl CfdpChecksum for CrcChecksum {
        fn update(&mut self, data: &[u8]) {
            self.digest.update(data);
        }

        fn finalize(self) -> u32 {
            self.digest.finalize()
        }
    }
}

/// A no-op checksum that always returns zero.
pub mod null {
    use crate::transport::cfdp::checksum::CfdpChecksum;

    /// A null checksum calculator that always produces zero.
    pub struct NullChecksum;

    impl CfdpChecksum for NullChecksum {
        fn update(&mut self, _data: &[u8]) {}
        fn finalize(self) -> u32 {
            0
        }
    }
}
