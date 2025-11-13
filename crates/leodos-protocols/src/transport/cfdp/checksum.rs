/// A synchronous, stateful checksum calculator.
pub trait CfdpChecksum: Send {
    /// Updates the internal state with a new chunk of data.
    fn update(&mut self, data: &[u8]);

    /// Consumes the calculator and returns the final 32-bit checksum.
    fn finalize(self) -> u32;
}

pub mod modular {
    use heapless::Vec;

    use crate::transport::cfdp::checksum::CfdpChecksum;

    pub struct ModularChecksum {
        sum: u32,
        pending: Vec<u8, 4>,
    }

    impl ModularChecksum {
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

pub mod crc {
    use crc::CRC_32_ISCSI;
    use crc::CRC_32_ISO_HDLC;
    use crc::Crc;
    use crc::Digest;

    use crate::transport::cfdp::checksum::CfdpChecksum;

    static CRC_CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
    static CRC_IEEE: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

    pub struct CrcChecksum {
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

pub mod null {
    use crate::transport::cfdp::checksum::CfdpChecksum;

    pub struct NullChecksum;

    impl CfdpChecksum for NullChecksum {
        fn update(&mut self, _data: &[u8]) {}
        fn finalize(self) -> u32 {
            0
        }
    }
}
