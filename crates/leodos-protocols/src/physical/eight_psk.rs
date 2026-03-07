//! Gray-coded 8PSK Modulation and Demodulation
//!
//! Maps 3-bit groups to one of 8 equally-spaced constellation
//! points on the unit circle. Gray coding ensures adjacent symbols
//! differ by exactly one bit, minimizing BER at moderate SNR.
//!
//! Used in high-rate downlinks and DVB-S2 based CCSDS links
//! (CCSDS 131.2-B).
//!
//! # Constellation
//!
//! ```text
//!         Q
//!     011 · 010
//!    /    |    \
//!  001   |   110
//!  ──────┼──────→ I
//!  000   |   111
//!    \   |   /
//!     100 · 101
//! ```

use super::modulation::clamp_i16;

/// Gray code: `GRAY[n]` = Gray-coded value for constellation
/// point `n` (angle = n·π/4).
static GRAY: [u8; 8] = [0, 1, 3, 2, 6, 7, 5, 4];

/// Inverse Gray code: `GRAY_INV[data]` = constellation index for
/// 3-bit data value `data`.
static GRAY_INV: [u8; 8] = {
    let mut t = [0u8; 8];
    let mut i = 0;
    while i < 8 {
        t[GRAY[i] as usize] = i as u8;
        i += 1;
    }
    t
};

/// Precomputed constellation points: `CONSTELLATION[n]` = (I, Q)
/// for constellation index `n`, with angle = n·π/4.
static CONSTELLATION: [(f32, f32); 8] = {
    // Compute at compile time using known values of cos/sin at
    // multiples of π/4.
    let s = 0.707_106_77; // 1/√2
    [
        (1.0, 0.0),   // 0
        (s, s),        // π/4
        (0.0, 1.0),   // π/2
        (-s, s),       // 3π/4
        (-1.0, 0.0),   // π
        (-s, -s),      // 5π/4
        (0.0, -1.0),   // 3π/2
        (s, -s),       // 7π/4
    ]
};

/// For each of the 3 bits, which constellation indices have that
/// bit = 0 and which have bit = 1.
///
/// `BIT_SETS[bit][0]` = indices where bit is 0 in the Gray code.
/// `BIT_SETS[bit][1]` = indices where bit is 1 in the Gray code.
static BIT_SETS: [[[u8; 4]; 2]; 3] = {
    let mut sets = [[[0u8; 4]; 2]; 3];
    let mut bit = 0;
    while bit < 3 {
        let mut count0 = 0usize;
        let mut count1 = 0usize;
        let mut idx = 0;
        while idx < 8 {
            if (GRAY[idx] >> (2 - bit)) & 1 == 0 {
                sets[bit][0][count0] = idx as u8;
                count0 += 1;
            } else {
                sets[bit][1][count1] = idx as u8;
                count1 += 1;
            }
            idx += 1;
        }
        bit += 1;
    }
    sets
};

/// Number of bits per 8PSK symbol.
pub const BITS_PER_SYMBOL: usize = 3;

/// Modulates packed bits to 8PSK I/Q symbols.
///
/// Groups `n_bits` into 3-bit chunks (MSB-first), maps each
/// through the Gray-coded constellation, and writes one I/Q pair
/// per symbol. `n_bits` must be a multiple of 3.
///
/// Writes `n_bits / 3` samples to each of `symbols_i`, `symbols_q`.
pub fn modulate_8psk(
    bits: &[u8],
    n_bits: usize,
    symbols_i: &mut [f32],
    symbols_q: &mut [f32],
) {
    assert!(n_bits % 3 == 0);
    let n_sym = n_bits / 3;
    assert!(bits.len() * 8 >= n_bits);
    assert!(symbols_i.len() >= n_sym);
    assert!(symbols_q.len() >= n_sym);

    for k in 0..n_sym {
        let base = 3 * k;
        let b0 = ((bits[base / 8] >> (7 - (base % 8))) & 1) as u8;
        let b1 =
            ((bits[(base + 1) / 8] >> (7 - ((base + 1) % 8))) & 1) as u8;
        let b2 =
            ((bits[(base + 2) / 8] >> (7 - ((base + 2) % 8))) & 1) as u8;

        let data = (b0 << 2) | (b1 << 1) | b2;
        let idx = GRAY_INV[data as usize] as usize;
        let (ci, cq) = CONSTELLATION[idx];
        symbols_i[k] = ci;
        symbols_q[k] = cq;
    }
}

/// Demodulates 8PSK symbols to soft-decision i16 LLRs.
///
/// Uses the max-log-MAP approximation: for each of the 3 bits
/// per symbol, computes the difference between the minimum
/// squared distance to constellation points where that bit is 0
/// vs 1.
///
/// Produces `n_bits` LLRs (3 per symbol, MSB first).
pub fn demodulate_8psk(
    symbols_i: &[f32],
    symbols_q: &[f32],
    n_bits: usize,
    noise_var: f32,
    scale: f32,
    llr: &mut [i16],
) {
    assert!(n_bits % 3 == 0);
    let n_sym = n_bits / 3;
    assert!(symbols_i.len() >= n_sym);
    assert!(symbols_q.len() >= n_sym);
    assert!(llr.len() >= n_bits);

    let factor = scale / noise_var;

    for k in 0..n_sym {
        let yi = symbols_i[k];
        let yq = symbols_q[k];

        // Squared distances to all 8 constellation points
        let mut dist2 = [0f32; 8];
        for p in 0..8 {
            let (ci, cq) = CONSTELLATION[p];
            let di = yi - ci;
            let dq = yq - cq;
            dist2[p] = di * di + dq * dq;
        }

        // For each of the 3 bits, compute max-log LLR
        for bit in 0..3 {
            let mut min_d0 = f32::MAX;
            for &idx in &BIT_SETS[bit][0] {
                let d = dist2[idx as usize];
                if d < min_d0 {
                    min_d0 = d;
                }
            }

            let mut min_d1 = f32::MAX;
            for &idx in &BIT_SETS[bit][1] {
                let d = dist2[idx as usize];
                if d < min_d1 {
                    min_d1 = d;
                }
            }

            // LLR > 0 means bit 0 more likely (smaller distance)
            let raw = factor * (min_d1 - min_d0);
            llr[3 * k + bit] = clamp_i16(raw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_code_adjacency() {
        // Adjacent constellation points should differ by 1 bit
        for i in 0..8 {
            let j = (i + 1) % 8;
            let diff = GRAY[i] ^ GRAY[j];
            assert_eq!(
                diff.count_ones(),
                1,
                "points {i} and {j} differ by {} bits",
                diff.count_ones()
            );
        }
    }

    #[test]
    fn gray_inverse_roundtrip() {
        for data in 0..8u8 {
            let idx = GRAY_INV[data as usize];
            assert_eq!(GRAY[idx as usize], data);
        }
    }

    #[test]
    fn constellation_unit_energy() {
        for (i, &(ci, cq)) in CONSTELLATION.iter().enumerate() {
            let energy = ci * ci + cq * cq;
            assert!(
                (energy - 1.0).abs() < 1e-5,
                "point {i}: energy = {energy}"
            );
        }
    }

    #[test]
    fn modulate_known_symbols() {
        // Data 000 → constellation index 0 → angle 0 → (1, 0)
        let bits = [0b000_000_00u8];
        let mut si = [0f32; 2];
        let mut sq = [0f32; 2];
        modulate_8psk(&bits, 6, &mut si, &mut sq);
        assert!((si[0] - 1.0).abs() < 1e-6);
        assert!(sq[0].abs() < 1e-6);
    }

    #[test]
    fn roundtrip_no_noise() {
        // 12 bits = 4 symbols
        let bits = [0b101_011_00, 0b0_110_0000];
        let mut si = [0f32; 4];
        let mut sq = [0f32; 4];
        modulate_8psk(&bits, 12, &mut si, &mut sq);

        let mut llr = [0i16; 12];
        demodulate_8psk(&si, &sq, 12, 0.5, 100.0, &mut llr);

        let mut recovered = [0u8; 2];
        for i in 0..12 {
            if llr[i] < 0 {
                recovered[i / 8] |= 1 << (7 - (i % 8));
            }
        }
        // Original bits: 101_011_000_110 = 0b10101100, 0b01100000
        assert_eq!(recovered[0], 0b10101100);
        assert_eq!(recovered[1] & 0xF0, 0b01100000);
    }

    #[test]
    fn all_symbols_roundtrip() {
        // Encode all 8 possible 3-bit values
        // 000 001 010 011 100 101 110 111 = 0x05, 0x39, 0xBF
        let bits = [0b000_001_01, 0b0_011_100_1, 0b01_110_111];
        let mut si = [0f32; 8];
        let mut sq = [0f32; 8];
        modulate_8psk(&bits, 24, &mut si, &mut sq);

        let mut llr = [0i16; 24];
        demodulate_8psk(&si, &sq, 24, 0.5, 100.0, &mut llr);

        let mut recovered = [0u8; 3];
        for i in 0..24 {
            if llr[i] < 0 {
                recovered[i / 8] |= 1 << (7 - (i % 8));
            }
        }
        assert_eq!(recovered, bits);
    }

    #[test]
    fn bit_sets_partition() {
        // Each bit's 0-set and 1-set should cover all 8 points
        for bit in 0..3 {
            let mut seen = [false; 8];
            for &idx in &BIT_SETS[bit][0] {
                seen[idx as usize] = true;
            }
            for &idx in &BIT_SETS[bit][1] {
                seen[idx as usize] = true;
            }
            assert!(seen.iter().all(|&s| s), "bit {bit} doesn't cover all");
        }
    }
}
