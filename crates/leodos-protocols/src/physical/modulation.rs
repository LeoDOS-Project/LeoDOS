//! BPSK and QPSK modulation and demodulation.
//!
//! # Modulation
//!
//! Maps coded bits to baseband symbols for transmission:
//! - **BPSK**: one real symbol per bit (0 → +1, 1 → −1)
//! - **QPSK**: one complex symbol per two bits
//!
//! # Demodulation
//!
//! Converts noisy received symbols to soft-decision log-likelihood
//! ratios (LLRs) for the channel decoder. The LLR for each bit is:
//!
//! ```text
//! LLR = 2 · y / σ²
//! ```
//!
//! where `y` is the received symbol and `σ²` is the noise variance.
//! Positive LLR → probably bit 0, negative → probably bit 1.

/// Modulation scheme.
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

// ── BPSK ──────────────────────────────────────────────────

/// Modulate packed bits to BPSK symbols (+1.0 / −1.0).
///
/// Reads `n_bits` from `bits` (MSB-first) and writes one `f32`
/// symbol per bit into `symbols`.
pub fn modulate_bpsk(
    bits: &[u8],
    n_bits: usize,
    symbols: &mut [f32],
) {
    assert!(bits.len() * 8 >= n_bits);
    assert!(symbols.len() >= n_bits);

    for i in 0..n_bits {
        let byte = bits[i / 8];
        let bit = (byte >> (7 - (i % 8))) & 1;
        symbols[i] = 1.0 - 2.0 * bit as f32;
    }
}

/// Demodulate BPSK symbols to soft-decision i16 LLRs.
///
/// `noise_var` is σ² (noise variance = N₀/2). The `scale`
/// factor converts floating-point LLRs to the i16 range used
/// by the LDPC decoder. A value of 100–500 works well.
///
/// The exact LLR is `2 · y / σ²`, quantized as
/// `clamp(round(scale · 2 · y / σ²), −32767, 32767)`.
pub fn demodulate_bpsk(
    symbols: &[f32],
    n_bits: usize,
    noise_var: f32,
    scale: f32,
    llr: &mut [i16],
) {
    assert!(symbols.len() >= n_bits);
    assert!(llr.len() >= n_bits);

    let factor = scale * 2.0 / noise_var;
    for i in 0..n_bits {
        let v = symbols[i] * factor;
        llr[i] = clamp_i16(v);
    }
}

// ── QPSK ──────────────────────────────────────────────────

/// Modulate packed bits to QPSK symbols.
///
/// Maps consecutive bit pairs (MSB-first) to I/Q components:
/// - bit 0 → I = 1−2·b₀
/// - bit 1 → Q = 1−2·b₁
///
/// Each component is ±1/√2 for unit energy per symbol.
/// `n_bits` must be even. Writes `n_bits/2` I values and
/// `n_bits/2` Q values.
pub fn modulate_qpsk(
    bits: &[u8],
    n_bits: usize,
    symbols_i: &mut [f32],
    symbols_q: &mut [f32],
) {
    assert!(n_bits % 2 == 0);
    let n_sym = n_bits / 2;
    assert!(bits.len() * 8 >= n_bits);
    assert!(symbols_i.len() >= n_sym);
    assert!(symbols_q.len() >= n_sym);

    let s = core::f32::consts::FRAC_1_SQRT_2;

    for k in 0..n_sym {
        let bit_i = 2 * k;
        let bit_q = 2 * k + 1;
        let b0 = ((bits[bit_i / 8] >> (7 - (bit_i % 8))) & 1) as f32;
        let b1 = ((bits[bit_q / 8] >> (7 - (bit_q % 8))) & 1) as f32;
        symbols_i[k] = s * (1.0 - 2.0 * b0);
        symbols_q[k] = s * (1.0 - 2.0 * b1);
    }
}

/// Demodulate QPSK symbols to soft-decision i16 LLRs.
///
/// Produces `n_bits` LLRs from `n_bits/2` I/Q symbol pairs.
/// LLRs are interleaved: even indices from I, odd from Q.
pub fn demodulate_qpsk(
    symbols_i: &[f32],
    symbols_q: &[f32],
    n_bits: usize,
    noise_var: f32,
    scale: f32,
    llr: &mut [i16],
) {
    assert!(n_bits % 2 == 0);
    let n_sym = n_bits / 2;
    assert!(symbols_i.len() >= n_sym);
    assert!(symbols_q.len() >= n_sym);
    assert!(llr.len() >= n_bits);

    let s = core::f32::consts::FRAC_1_SQRT_2;
    let factor = scale * 2.0 / noise_var;

    // Undo the 1/√2 scaling: multiply received by √2
    // so effective LLR = 2·(y·√2)/σ² = 2·y/(σ²/√2)
    // Equivalently: factor already includes the geometry.
    let f = factor * s;

    for k in 0..n_sym {
        // I component → even bit, Q component → odd bit
        llr[2 * k] = clamp_i16(symbols_i[k] * f);
        llr[2 * k + 1] = clamp_i16(symbols_q[k] * f);
    }
}

// ── Helpers ───────────────────────────────────────────────

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
    fn bpsk_modulate_basic() {
        // 0xA5 = 0b10100101 → [-1,+1,-1,+1,+1,-1,+1,-1]
        let bits = [0xA5u8];
        let mut sym = [0f32; 8];
        modulate_bpsk(&bits, 8, &mut sym);
        let expected = [-1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0];
        assert_eq!(sym, expected);
    }

    #[test]
    fn bpsk_roundtrip_hard() {
        let bits = [0xC3, 0x5A]; // 16 bits
        let mut sym = [0f32; 16];
        modulate_bpsk(&bits, 16, &mut sym);

        // No noise → perfect LLRs
        let mut llr = [0i16; 16];
        demodulate_bpsk(&sym, 16, 0.5, 100.0, &mut llr);

        // Recover bits from LLR signs
        let mut recovered = [0u8; 2];
        for i in 0..16 {
            if llr[i] < 0 {
                recovered[i / 8] |= 1 << (7 - (i % 8));
            }
        }
        assert_eq!(recovered, bits);
    }

    #[test]
    fn qpsk_modulate_unit_energy() {
        let bits = [0xFF]; // 8 bits → 4 QPSK symbols
        let mut si = [0f32; 4];
        let mut sq = [0f32; 4];
        modulate_qpsk(&bits, 8, &mut si, &mut sq);

        // All bits = 1, so all components = -1/√2
        let s = -core::f32::consts::FRAC_1_SQRT_2;
        for k in 0..4 {
            assert!((si[k] - s).abs() < 1e-6);
            assert!((sq[k] - s).abs() < 1e-6);
            // Unit energy: I² + Q² = 1
            let energy = si[k] * si[k] + sq[k] * sq[k];
            assert!((energy - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn qpsk_roundtrip_hard() {
        let bits = [0xC3, 0x5A]; // 16 bits → 8 symbols
        let mut si = [0f32; 8];
        let mut sq = [0f32; 8];
        modulate_qpsk(&bits, 16, &mut si, &mut sq);

        let mut llr = [0i16; 16];
        demodulate_qpsk(&si, &sq, 16, 0.5, 100.0, &mut llr);

        let mut recovered = [0u8; 2];
        for i in 0..16 {
            if llr[i] < 0 {
                recovered[i / 8] |= 1 << (7 - (i % 8));
            }
        }
        assert_eq!(recovered, bits);
    }

    #[test]
    fn noise_variance_known_values() {
        // Eb/N0 = 0 dB, rate 1/2 → σ² = 1/(2·0.5·1) = 1.0
        let v = noise_variance(0.0, 0.5);
        assert!((v - 1.0).abs() < 1e-6);

        // Eb/N0 = 10 dB, rate 1 → σ² = 1/(2·1·10) = 0.05
        let v = noise_variance(10.0, 1.0);
        assert!((v - 0.05).abs() < 1e-4);
    }

    #[test]
    fn scheme_bits_per_symbol() {
        assert_eq!(Scheme::Bpsk.bits_per_symbol(), 1);
        assert_eq!(Scheme::Qpsk.bits_per_symbol(), 2);
        assert_eq!(Scheme::Bpsk.symbols_for(2048), 2048);
        assert_eq!(Scheme::Qpsk.symbols_for(2048), 1024);
    }

    #[test]
    fn partial_byte_bpsk() {
        // Only 3 bits from 0xE0 = 0b111_00000
        let bits = [0xE0u8];
        let mut sym = [0f32; 3];
        modulate_bpsk(&bits, 3, &mut sym);
        assert_eq!(sym, [-1.0, -1.0, -1.0]);
    }

    #[test]
    fn bpsk_with_awgn() {
        // Simulate light noise and verify demodulation still works
        let bits = [0xA5u8]; // 10100101
        let mut sym = [0f32; 8];
        modulate_bpsk(&bits, 8, &mut sym);

        // Add small deterministic "noise"
        for (i, s) in sym.iter_mut().enumerate() {
            *s += 0.1 * (i as f32 - 4.0) / 4.0;
        }

        let mut llr = [0i16; 8];
        demodulate_bpsk(&sym, 8, 0.5, 100.0, &mut llr);

        // Should still decode correctly (noise is small)
        let mut recovered = [0u8; 1];
        for i in 0..8 {
            if llr[i] < 0 {
                recovered[0] |= 1 << (7 - i);
            }
        }
        assert_eq!(recovered[0], 0xA5);
    }
}
