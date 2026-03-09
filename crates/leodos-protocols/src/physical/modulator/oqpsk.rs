//! Offset Quadrature Phase Shift Keying (OQPSK)
//!
//! OQPSK staggers the I and Q channels by half a symbol period,
//! ensuring they never transition simultaneously. This reduces
//! the maximum phase change per transition from 180° (QPSK) to
//! 90°, yielding a more constant envelope. Used in Proximity-1
//! links (CCSDS 211.0).
//!
//! The modulator outputs at 2× the QPSK symbol rate to properly
//! represent the I/Q offset. The demodulator re-aligns the
//! channels before computing LLRs.

use super::clamp_i16;

/// Modulates packed bits to OQPSK at 2× the symbol rate.
///
/// Maps consecutive bit pairs to I/Q values (±1/√2), then
/// staggers them: I transitions at even sample indices, Q at
/// odd indices.
///
/// `n_bits` must be even. Writes `n_bits` samples each to
/// `symbols_i` and `symbols_q`.
pub fn modulate_oqpsk(
    bits: &[u8],
    n_bits: usize,
    symbols_i: &mut [f32],
    symbols_q: &mut [f32],
) {
    assert!(n_bits % 2 == 0);
    let n_sym = n_bits / 2;
    assert!(bits.len() * 8 >= n_bits);
    assert!(symbols_i.len() >= n_bits);
    assert!(symbols_q.len() >= n_bits);

    let s = core::f32::consts::FRAC_1_SQRT_2;

    fn get_bit(bits: &[u8], idx: usize) -> f32 {
        ((bits[idx / 8] >> (7 - (idx % 8))) & 1) as f32
    }

    for k in 0..n_sym {
        let i_val = s * (1.0 - 2.0 * get_bit(bits, 2 * k));
        let q_val = s * (1.0 - 2.0 * get_bit(bits, 2 * k + 1));

        // I transitions at even sample indices
        symbols_i[2 * k] = i_val;
        symbols_i[2 * k + 1] = i_val;

        // Q transitions at odd sample indices (half-symbol later)
        if k > 0 {
            // Q[2k] still holds the previous symbol's Q value
            // (already written as symbols_q[2k-1] in the previous iteration)
        } else {
            symbols_q[0] = 0.0;
        }
        symbols_q[2 * k + 1] = q_val;
        if 2 * k + 2 < n_bits {
            symbols_q[2 * k + 2] = q_val;
        }
    }
}

/// Demodulates OQPSK symbols to soft-decision i16 LLRs.
///
/// Re-aligns the staggered I/Q channels by sampling I at even
/// indices and Q at odd indices, then computes LLRs as for QPSK.
///
/// Produces `n_bits` LLRs: even indices from I, odd from Q.
pub fn demodulate_oqpsk(
    symbols_i: &[f32],
    symbols_q: &[f32],
    n_bits: usize,
    noise_var: f32,
    scale: f32,
    llr: &mut [i16],
) {
    assert!(n_bits % 2 == 0);
    let n_sym = n_bits / 2;
    assert!(symbols_i.len() >= n_bits);
    assert!(symbols_q.len() >= n_bits);
    assert!(llr.len() >= n_bits);

    let s = core::f32::consts::FRAC_1_SQRT_2;
    let factor = scale * 2.0 / noise_var * s;

    for k in 0..n_sym {
        // I bit sampled at even index (center of I pulse)
        llr[2 * k] = clamp_i16(symbols_i[2 * k] * factor);
        // Q bit sampled at odd index (center of Q pulse)
        llr[2 * k + 1] = clamp_i16(symbols_q[2 * k + 1] * factor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oqpsk_modulate_all_zeros() {
        let bits = [0x00u8];
        let mut si = [0f32; 8];
        let mut sq = [0f32; 8];
        modulate_oqpsk(&bits, 8, &mut si, &mut sq);

        let s = core::f32::consts::FRAC_1_SQRT_2;
        // All I values should be +1/√2
        for k in 0..4 {
            assert!((si[2 * k] - s).abs() < 1e-6);
            assert!((si[2 * k + 1] - s).abs() < 1e-6);
        }
    }

    #[test]
    fn oqpsk_i_q_never_simultaneous() {
        let bits = [0xFFu8];
        let mut si = [0f32; 8];
        let mut sq = [0f32; 8];
        modulate_oqpsk(&bits, 8, &mut si, &mut sq);

        // Between consecutive samples, at most one of I/Q changes
        for n in 1..8 {
            let i_changed = (si[n] - si[n - 1]).abs() > 1e-6;
            let q_changed = (sq[n] - sq[n - 1]).abs() > 1e-6;
            assert!(
                !(i_changed && q_changed),
                "I and Q both changed at sample {n}"
            );
        }
    }

    #[test]
    fn oqpsk_roundtrip() {
        let bits = [0xC3, 0x5A];
        let mut si = [0f32; 16];
        let mut sq = [0f32; 16];
        modulate_oqpsk(&bits, 16, &mut si, &mut sq);

        let mut llr = [0i16; 16];
        demodulate_oqpsk(&si, &sq, 16, 0.5, 100.0, &mut llr);

        let mut recovered = [0u8; 2];
        for i in 0..16 {
            if llr[i] < 0 {
                recovered[i / 8] |= 1 << (7 - (i % 8));
            }
        }
        assert_eq!(recovered, bits);
    }

    #[test]
    fn oqpsk_unit_energy() {
        let bits = [0xA5u8];
        let mut si = [0f32; 8];
        let mut sq = [0f32; 8];
        modulate_oqpsk(&bits, 8, &mut si, &mut sq);

        // At odd sample indices (where both I and Q are valid),
        // I² + Q² should be approximately 1.
        for k in 0..4 {
            let n = 2 * k + 1;
            let energy = si[n] * si[n] + sq[n] * sq[n];
            assert!(
                (energy - 1.0).abs() < 1e-5,
                "energy at sample {n} = {energy}"
            );
        }
    }
}
