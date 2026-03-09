//! Modulation and demodulation schemes.
//!
//! Each submodule implements a modulation scheme with `modulate()`
//! and `demodulate()` functions producing soft-decision LLRs.

/// Binary Phase Shift Keying (1 bit/symbol).
pub mod bpsk;
/// Quadrature Phase Shift Keying (2 bits/symbol).
pub mod qpsk;
/// Offset QPSK (Proximity-1, CCSDS 211.0).
pub mod oqpsk;
/// Gray-coded 8PSK (3 bits/symbol, CCSDS 131.2-B).
pub mod eight_psk;
/// Gaussian Minimum Shift Keying (CCSDS 211.0).
pub mod gmsk;

/// Modulation scheme selector.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Scheme {
    /// Binary Phase Shift Keying (1 bit/symbol).
    Bpsk,
    /// Quadrature Phase Shift Keying (2 bits/symbol).
    Qpsk,
}

impl Scheme {
    /// Bits carried per symbol.
    pub const fn bits_per_symbol(self) -> usize {
        match self {
            Self::Bpsk => 1,
            Self::Qpsk => 2,
        }
    }

    /// Number of symbols needed for `n_bits` bits.
    pub const fn symbols_for(self, n_bits: usize) -> usize {
        let bps = self.bits_per_symbol();
        (n_bits + bps - 1) / bps
    }
}

/// Clamps a float to the i16 range (−32767..32767) and truncates.
pub fn clamp_i16(v: f32) -> i16 {
    if v > 32767.0 {
        32767
    } else if v < -32767.0 {
        -32767
    } else {
        v as i16
    }
}

/// Compute noise variance σ² from Eb/N₀ (in dB) and code rate.
///
/// For BPSK: σ² = 1 / (2 · rate · 10^(eb_n0_db/10))
/// For QPSK: same formula (QPSK has same BER vs Eb/N₀ as BPSK).
pub fn noise_variance(eb_n0_db: f32, code_rate: f32) -> f32 {
    let eb_n0_lin = libm::powf(10.0, eb_n0_db / 10.0);
    1.0 / (2.0 * code_rate * eb_n0_lin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_bits_per_symbol() {
        assert_eq!(Scheme::Bpsk.bits_per_symbol(), 1);
        assert_eq!(Scheme::Qpsk.bits_per_symbol(), 2);
        assert_eq!(Scheme::Bpsk.symbols_for(2048), 2048);
        assert_eq!(Scheme::Qpsk.symbols_for(2048), 1024);
    }
}
