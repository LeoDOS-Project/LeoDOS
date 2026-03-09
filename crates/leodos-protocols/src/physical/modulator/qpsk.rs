//! QPSK modulation and demodulation.
//!
//! Quadrature Phase Shift Keying maps two bits per complex symbol:
//! - bit 0 → I = ±1/√2
//! - bit 1 → Q = ±1/√2

use super::clamp_i16;

/// Modulate packed bits to QPSK symbols.
///
/// Maps consecutive bit pairs (MSB-first) to I/Q components:
/// - bit 0 → I = 1−2·b₀
/// - bit 1 → Q = 1−2·b₁
///
/// Each component is ±1/√2 for unit energy per symbol.
/// `n_bits` must be even. Writes `n_bits/2` I values and
/// `n_bits/2` Q values.
pub fn modulate(
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
pub fn demodulate(
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

/// QPSK modulator/demodulator with configurable noise parameters.
pub struct Qpsk {
    noise_var: f32,
    scale: f32,
}

impl Qpsk {
    /// Creates a QPSK modem with the given noise variance and
    /// LLR scale factor.
    pub fn new(noise_var: f32, scale: f32) -> Self {
        Self { noise_var, scale }
    }
}

impl super::Modulator for Qpsk {
    fn modulate(
        &self,
        bits: &[u8],
        n_bits: usize,
        symbols: &mut [f32],
    ) -> usize {
        let n_sym = n_bits / 2;
        let (si, sq) = symbols.split_at_mut(n_sym);
        modulate(bits, n_bits, si, sq);
        n_sym * 2
    }
}

impl super::Demodulator for Qpsk {
    fn demodulate_soft(
        &self,
        symbols: &[f32],
        n_bits: usize,
        llr: &mut [i16],
    ) {
        let n_sym = n_bits / 2;
        let (si, sq) = symbols.split_at(n_sym);
        demodulate(si, sq, n_bits, self.noise_var, self.scale, llr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::noise_variance;

    #[test]
    fn modulate_unit_energy() {
        let bits = [0xFF]; // 8 bits → 4 QPSK symbols
        let mut si = [0f32; 4];
        let mut sq = [0f32; 4];
        modulate(&bits, 8, &mut si, &mut sq);

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
    fn roundtrip_hard() {
        let bits = [0xC3, 0x5A]; // 16 bits → 8 symbols
        let mut si = [0f32; 8];
        let mut sq = [0f32; 8];
        modulate(&bits, 16, &mut si, &mut sq);

        let mut llr = [0i16; 16];
        demodulate(&si, &sq, 16, 0.5, 100.0, &mut llr);

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
}
