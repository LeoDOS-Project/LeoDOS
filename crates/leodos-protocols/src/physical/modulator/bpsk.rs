//! BPSK modulation and demodulation.
//!
//! Binary Phase Shift Keying maps one bit per symbol:
//! - bit 0 → +1.0
//! - bit 1 → −1.0

use super::clamp_i16;

/// Modulate packed bits to BPSK symbols (+1.0 / −1.0).
///
/// Reads `n_bits` from `bits` (MSB-first) and writes one `f32`
/// symbol per bit into `symbols`.
pub fn modulate(
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
pub fn demodulate(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modulate_basic() {
        // 0xA5 = 0b10100101 → [-1,+1,-1,+1,+1,-1,+1,-1]
        let bits = [0xA5u8];
        let mut sym = [0f32; 8];
        modulate(&bits, 8, &mut sym);
        let expected = [-1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0];
        assert_eq!(sym, expected);
    }

    #[test]
    fn roundtrip_hard() {
        let bits = [0xC3, 0x5A]; // 16 bits
        let mut sym = [0f32; 16];
        modulate(&bits, 16, &mut sym);

        // No noise → perfect LLRs
        let mut llr = [0i16; 16];
        demodulate(&sym, 16, 0.5, 100.0, &mut llr);

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
    fn partial_byte() {
        // Only 3 bits from 0xE0 = 0b111_00000
        let bits = [0xE0u8];
        let mut sym = [0f32; 3];
        modulate(&bits, 3, &mut sym);
        assert_eq!(sym, [-1.0, -1.0, -1.0]);
    }

    #[test]
    fn with_awgn() {
        // Simulate light noise and verify demodulation still works
        let bits = [0xA5u8]; // 10100101
        let mut sym = [0f32; 8];
        modulate(&bits, 8, &mut sym);

        // Add small deterministic "noise"
        for (i, s) in sym.iter_mut().enumerate() {
            *s += 0.1 * (i as f32 - 4.0) / 4.0;
        }

        let mut llr = [0i16; 8];
        demodulate(&sym, 8, 0.5, 100.0, &mut llr);

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
