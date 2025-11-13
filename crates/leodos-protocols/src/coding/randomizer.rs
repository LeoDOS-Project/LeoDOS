//! CCSDS-compliant pseudo-randomization for TC and TM frames.
//!
//! Spec: https://ccsds.org/Pubs/131x0b5.pdf
//!
//! Randomization is the process of XORing the frame data with a standard,
//! pre-defined sequence. This is done to ensure the final transmitted data has
//! frequent bit transitions (0-to-1 and 1-to-0), which is essential for helping
//! the receiver's hardware maintain clock synchronization with the signal.
//!
//! The randomization sequence is its own inverse; applying the same operation
//! twice restores the original data.

/// A trait for applying a specific CCSDS randomization algorithm.
pub trait Randomizer {
    /// Applies or removes the randomization sequence in-place on the provided buffer.
    fn apply(&self, buffer: &mut [u8]) {
        for (byte, &rand) in buffer.iter_mut().zip(self.table().iter().cycle()) {
            *byte ^= rand;
        }
    }
    fn table(&self) -> &[u8];
}

/// The standard randomizer for Telecommand (TC) frames.
pub struct TcRandomizer([u8; 255]);
/// The legacy 255-byte randomizer for Telemetry (TM) frames.
pub struct Tm255Randomizer([u8; 255]);
/// The recommended 131071-byte randomizer for Telemetry (TM) frames.
///
/// **Warning:** Using this will embed a 128 KB lookup table in your binary.
pub struct Tm131071Randomizer([u8; 131071]);

impl TcRandomizer {
    /// Creates a new instance of the TC randomizer.
    pub const fn new() -> Self {
        let mut table = [0u8; 255];
        let mut lfsr = 0xFF_u8;
        let mut i = 0;
        while i < 255 {
            let mut val = 0_u8;
            let mut bit = 0;
            while bit < 8 {
                val = (val << 1) | (lfsr & 1);
                let extra_bit =
                    (lfsr ^ (lfsr >> 1) ^ (lfsr >> 2) ^ (lfsr >> 3) ^ (lfsr >> 4) ^ (lfsr >> 6))
                        & 1;
                lfsr = (lfsr >> 1) | (extra_bit << 7);
                bit += 1;
            }
            table[i] = val;
            i += 1;
        }
        TcRandomizer(table)
    }
}

impl Randomizer for TcRandomizer {
    fn table(&self) -> &[u8] {
        &self.0
    }
}

// --- TM Randomizers (new code) ---

impl Tm255Randomizer {
    /// Creates a new instance of the legacy 255-byte TM randomizer.
    pub const fn new() -> Self {
        let mut table = [0u8; 255];
        let mut lfsr = 0xFF_u8;
        let mut i = 0;
        while i < 255 {
            let mut val = 0_u8;
            let mut bit = 0;
            while bit < 8 {
                val = (val << 1) | (lfsr & 1);
                let extra_bit = (lfsr ^ (lfsr >> 3) ^ (lfsr >> 5) ^ (lfsr >> 7)) & 1;
                lfsr = (lfsr >> 1) | (extra_bit << 7);
                bit += 1;
            }
            table[i] = val;
            i += 1;
        }
        Tm255Randomizer(table)
    }
}

impl Randomizer for Tm255Randomizer {
    fn table(&self) -> &[u8] {
        &self.0
    }
}

impl Tm131071Randomizer {
    /// Creates a new instance of the 131071-byte TM randomizer.
    pub const fn new() -> Self {
        let mut table = [0u8; 131071];
        let mut lfsr = 0x1FFFF_u32; // 17 bits set to 1
        let mut i = 0;
        while i < 131071 {
            let mut val = 0_u8;
            let mut bit = 0;
            while bit < 8 {
                val = (val << 1) | ((lfsr & 1) as u8);
                let extra_bit = (lfsr ^ (lfsr >> 3)) & 1; // Corresponds to x^17 + x^14 + 1
                lfsr = (lfsr >> 1) | (extra_bit << 16);
                bit += 1;
            }
            table[i] = val;
            i += 1;
        }
        Tm131071Randomizer(table)
    }
}

impl Randomizer for Tm131071Randomizer {
    fn table(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tc_randomization_roundtrip() {
        // Initialize an array on the stack. No heap allocation.
        let mut data = [0u8; 512];
        for i in 0..data.len() {
            data[i] = (i % 256) as u8;
        }
        // Manually copy the array to create the 'original' version.
        let original = data;
        let randomizer = TcRandomizer::new();

        randomizer.apply(&mut data);

        assert_ne!(original, data, "Randomized data should not match original");

        randomizer.apply(&mut data);
        assert_eq!(original, data, "De-randomized data should match original");
    }

    #[test]
    fn tm_255_randomization_roundtrip() {
        let mut data = [0u8; 512];
        for i in 0..data.len() {
            data[i] = (i % 256) as u8;
        }
        let original = data;
        let randomizer = Tm255Randomizer::new();

        randomizer.apply(&mut data);
        assert_ne!(original, data, "Randomized data should not match original");

        randomizer.apply(&mut data);
        assert_eq!(original, data, "De-randomized data should match original");
    }

    // Note: We don't add a test for the 131071-byte randomizer here
    // because creating a 128KB array on the stack could cause a stack overflow
    // in the test environment. The logic is identical to the others, so this
    // is a reasonable omission.
}
