//! CCSDS Convolutional Code (Rate 1/2, K=7) with Viterbi Decoding
//!
//! Implements the convolutional code specified in CCSDS 131.0-B-5
//! (TM Synchronization and Channel Coding).
//!
//! # Parameters
//!
//! - Constraint length: K = 7 (64 encoder states)
//! - Code rate: 1/2 (2 output symbols per input bit)
//! - Generator polynomials: G1 = 171₈ (0x79), G2 = 133₈ (0x5B)
//! - Tail: K−1 = 6 zero bits flush the encoder to state 0
//!
//! # Encoding
//!
//! Each input bit plus the 6-bit shift register state produces two
//! coded symbols (G1 first, then G2). After all data bits, six zero
//! tail bits terminate the trellis at state 0.
//!
//! # Decoding
//!
//! Soft-decision Viterbi algorithm with i16 LLR inputs (positive
//! means bit 0 is more likely), matching the LDPC decoder convention.
//! Uses a sliding-window traceback (depth 5K = 35) to bound stack
//! usage regardless of frame length.

/// Constraint length.
pub const K: usize = 7;

/// Number of encoder memory elements (K − 1).
const MEMORY: usize = K - 1;

/// Number of trellis states (2^(K−1) = 64).
const NUM_STATES: usize = 1 << MEMORY;

/// Generator polynomial G1 (octal 171).
const G1: u8 = 0x79;

/// Generator polynomial G2 (octal 133).
const G2: u8 = 0x5B;

/// Traceback depth for the sliding-window Viterbi decoder (5 × K).
const TRACEBACK_DEPTH: usize = 5 * K;

/// Path metric for unreachable states.
const NEGINF: i32 = i32::MIN / 2;

/// Errors from convolutional coding operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConvError {
    /// Output buffer is too small.
    BufferTooSmall {
        /// Minimum required size in bytes.
        required: usize,
        /// Provided buffer size in bytes.
        provided: usize,
    },
    /// LLR count must be even (two LLRs per trellis step).
    OddLlrCount,
    /// Frame too short (need at least K−1 trellis steps).
    FrameTooShort,
}

/// Precomputed branch output symbols for each (state, input_bit).
///
/// `BRANCH[state][input]` = `(G1_output, G2_output)`.
static BRANCH: [[(u8, u8); 2]; NUM_STATES] = {
    let mut t = [[(0u8, 0u8); 2]; NUM_STATES];
    let mut s = 0;
    while s < NUM_STATES {
        let mut b = 0u8;
        while b < 2 {
            let reg = (b << 6) | (s as u8);
            let g1 = (reg & G1).count_ones() as u8 & 1;
            let g2 = (reg & G2).count_ones() as u8 & 1;
            t[s][b as usize] = (g1, g2);
            b += 1;
        }
        s += 1;
    }
    t
};

/// Returns the number of encoded bytes for `data_len` data bytes.
///
/// Accounts for K−1 tail bits and rate-1/2 expansion.
pub fn encoded_len(data_len: usize) -> usize {
    let coded_bits = (data_len * 8 + MEMORY) * 2;
    (coded_bits + 7) / 8
}

/// Encodes data using the rate-1/2, K=7 convolutional code.
///
/// Appends K−1 = 6 zero tail bits to terminate the trellis.
/// Output symbols are packed MSB-first: for each input bit the
/// G1 symbol comes first, then G2.
///
/// Returns the number of bytes written to `output`.
pub fn encode(data: &[u8], output: &mut [u8]) -> Result<usize, ConvError> {
    let out_len = encoded_len(data.len());
    if output.len() < out_len {
        return Err(ConvError::BufferTooSmall {
            required: out_len,
            provided: output.len(),
        });
    }

    output[..out_len].fill(0);
    let mut state = 0u8;
    let mut oi = 0usize;

    for &byte in data {
        for bit_pos in (0..8).rev() {
            let b = (byte >> bit_pos) & 1;
            let (g1, g2) = BRANCH[state as usize][b as usize];
            output[oi / 8] |= g1 << (7 - oi % 8);
            oi += 1;
            output[oi / 8] |= g2 << (7 - oi % 8);
            oi += 1;
            state = (state >> 1) | (b << 5);
        }
    }

    for _ in 0..MEMORY {
        let (g1, g2) = BRANCH[state as usize][0];
        output[oi / 8] |= g1 << (7 - oi % 8);
        oi += 1;
        output[oi / 8] |= g2 << (7 - oi % 8);
        oi += 1;
        state >>= 1;
    }

    Ok(out_len)
}

/// Converts hard bits (packed bytes, MSB-first) to LLR values.
///
/// Each bit becomes an i16: bit 0 → `+magnitude`, bit 1 → `−magnitude`.
/// This is useful for testing the Viterbi decoder with hard-decision
/// input.
pub fn hard_to_llr(bits: &[u8], num_bits: usize, magnitude: i16, llrs: &mut [i16]) {
    for i in 0..num_bits.min(llrs.len()) {
        let bit = (bits[i / 8] >> (7 - i % 8)) & 1;
        llrs[i] = if bit == 0 { magnitude } else { -magnitude };
    }
}

/// Decodes a convolutionally coded frame using soft-decision Viterbi.
///
/// `llrs` contains paired (G1, G2) log-likelihood ratios per trellis
/// step. Positive LLR means bit 0 is more likely. The trellis must
/// be terminated (K−1 tail steps appended by the encoder).
///
/// Returns the number of decoded data bytes written to `output`.
pub fn decode(llrs: &[i16], output: &mut [u8]) -> Result<usize, ConvError> {
    if llrs.len() % 2 != 0 {
        return Err(ConvError::OddLlrCount);
    }

    let num_steps = llrs.len() / 2;
    if num_steps < MEMORY {
        return Err(ConvError::FrameTooShort);
    }

    let info_bits = num_steps - MEMORY;
    let out_bytes = (info_bits + 7) / 8;

    if output.len() < out_bytes {
        return Err(ConvError::BufferTooSmall {
            required: out_bytes,
            provided: output.len(),
        });
    }

    output[..out_bytes].fill(0);

    let mut pm = [[NEGINF; NUM_STATES]; 2];
    pm[0][0] = 0;
    let mut cur = 0usize;
    let mut decisions = [0u64; TRACEBACK_DEPTH];
    let mut decoded = 0usize;

    for step in 0..num_steps {
        let l1 = llrs[2 * step] as i32;
        let l2 = llrs[2 * step + 1] as i32;

        // Branch metrics: bm[g1_out][g2_out]
        let bm = [
            [l1 + l2, l1 - l2],
            [-l1 + l2, -l1 - l2],
        ];

        let prev = cur;
        cur = 1 - cur;
        let dslot = step % TRACEBACK_DEPTH;
        let mut dword = 0u64;

        for ns in 0..NUM_STATES {
            let input = (ns >> 5) & 1;
            let p0 = (ns << 1) & (NUM_STATES - 1);
            let p1 = p0 | 1;

            let (g1_0, g2_0) = BRANCH[p0][input];
            let (g1_1, g2_1) = BRANCH[p1][input];

            let m0 = pm[prev][p0]
                .saturating_add(bm[g1_0 as usize][g2_0 as usize]);
            let m1 = pm[prev][p1]
                .saturating_add(bm[g1_1 as usize][g2_1 as usize]);

            if m1 > m0 {
                pm[cur][ns] = m1;
                dword |= 1 << ns;
            } else {
                pm[cur][ns] = m0;
            }
        }

        decisions[dslot] = dword;

        // Normalize metrics to prevent overflow
        if step & 0xFF == 0xFF {
            let mut min_val = pm[cur][0];
            for s in 1..NUM_STATES {
                if pm[cur][s] < min_val {
                    min_val = pm[cur][s];
                }
            }
            for m in &mut pm[cur] {
                *m -= min_val;
            }
        }

        // Sliding-window traceback: output one decoded bit per step
        // once the window is full.
        if step >= TRACEBACK_DEPTH - 1 && decoded < info_bits {
            // Find the state with the best metric
            let mut best_s = 0;
            let mut best_m = pm[cur][0];
            for s in 1..NUM_STATES {
                if pm[cur][s] > best_m {
                    best_m = pm[cur][s];
                    best_s = s;
                }
            }

            // Trace back through the decision buffer
            let mut s = best_s;
            let mut slot = dslot;
            for _ in 0..TRACEBACK_DEPTH - 1 {
                let d = ((decisions[slot] >> s) & 1) as usize;
                s = ((s << 1) | d) & (NUM_STATES - 1);
                slot = if slot == 0 {
                    TRACEBACK_DEPTH - 1
                } else {
                    slot - 1
                };
            }

            // The MSB of the traced-back state is the decoded bit
            let bit = (s >> 5) & 1;
            if bit == 1 {
                output[decoded / 8] |= 1 << (7 - decoded % 8);
            }
            decoded += 1;
        }
    }

    // Final traceback from the known terminal state (0) for the
    // remaining info bits that the sliding window didn't cover.
    if decoded < info_bits {
        let mut s = 0usize;
        let last_slot = (num_steps - 1) % TRACEBACK_DEPTH;
        let available = num_steps.min(TRACEBACK_DEPTH);

        // Collect input bits by tracing backward from state 0.
        // bits[i] = input bit at trellis step (num_steps − 1 − i).
        let mut bits = [0u8; TRACEBACK_DEPTH];
        bits[0] = ((s >> 5) & 1) as u8;

        let mut slot = last_slot;
        for i in 1..available {
            let d = ((decisions[slot] >> s) & 1) as usize;
            s = ((s << 1) | d) & (NUM_STATES - 1);
            slot = if slot == 0 {
                TRACEBACK_DEPTH - 1
            } else {
                slot - 1
            };
            bits[i] = ((s >> 5) & 1) as u8;
        }

        while decoded < info_bits {
            let rev_idx = num_steps - 1 - decoded;
            if rev_idx < available && bits[rev_idx] == 1 {
                output[decoded / 8] |= 1 << (7 - decoded % 8);
            }
            decoded += 1;
        }
    }

    Ok(out_bytes)
}

/// Convolutional encoder implementing [`FecEncoder`](super::FecEncoder).
pub struct ConvolutionalEncoder;

impl super::FecEncoder for ConvolutionalEncoder {
    type Error = ConvError;

    fn encode(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error> {
        encode(data, output)
    }
}

/// Hard-decision Viterbi decoder implementing [`FecDecoder`](super::FecDecoder).
pub struct ViterbiDecoder {
    llr_magnitude: i16,
}

impl ViterbiDecoder {
    /// Creates a decoder with the given hard-decision LLR magnitude.
    pub fn new(llr_magnitude: i16) -> Self {
        Self { llr_magnitude }
    }
}

impl super::FecDecoder for ViterbiDecoder {
    type Error = ConvError;

    fn decode(&self, data: &mut [u8]) -> Result<usize, Self::Error> {
        let num_bits = data.len() * 8;
        let mut llrs = [0i16; 8192];
        hard_to_llr(data, num_bits, self.llr_magnitude, &mut llrs[..num_bits]);
        let mut output = [0u8; 1024];
        let len = decode(&llrs[..num_bits], &mut output)?;
        data[..len].copy_from_slice(&output[..len]);
        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_len_calculation() {
        // 1 byte = 8 bits + 6 tail = 14 steps × 2 = 28 coded bits
        // = 4 bytes (28 / 8 = 3.5 → 4)
        assert_eq!(encoded_len(1), 4);
        // 10 bytes = 80 + 6 = 86 steps × 2 = 172 bits = 22 bytes
        assert_eq!(encoded_len(10), 22);
        // 0 bytes = 6 tail × 2 = 12 bits = 2 bytes
        assert_eq!(encoded_len(0), 2);
    }

    #[test]
    fn encode_zeros() {
        let data = [0u8; 4];
        let mut out = [0u8; 10];
        let len = encode(&data, &mut out).unwrap();

        // All-zero input with all-zero initial state should produce
        // all-zero output (G1(0)=0, G2(0)=0 for state 0, input 0).
        assert_eq!(len, encoded_len(4));
        assert!(out[..len].iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_buffer_too_small() {
        let data = [0u8; 4];
        let mut out = [0u8; 2]; // too small
        let err = encode(&data, &mut out);
        assert!(matches!(err, Err(ConvError::BufferTooSmall { .. })));
    }

    #[test]
    fn encoder_state_returns_to_zero() {
        // After encoding with tail bits, encoder must be in state 0.
        // We verify indirectly: the last 6 coded symbol pairs should
        // all be (0,0) when starting from state 0 (which happens
        // when input is all zeros).
        let data = [0u8; 1];
        let mut out = [0u8; 4];
        encode(&data, &mut out).unwrap();

        // For all-zero input, state never leaves 0, so all output = 0
        assert!(out.iter().all(|&b| b == 0));
    }

    #[test]
    fn encode_known_pattern() {
        // Single byte 0x80 = bit pattern 10000000
        // Bit 0 (=1): state 0, input 1
        //   reg = (1<<6)|0 = 0b1000000
        //   G1 = parity(0b1000000 & 0x79) = 1
        //   G2 = parity(0b1000000 & 0x5B) = 1
        //   state → 32
        // Bit 1 (=0): state 32, input 0
        //   reg = (0<<6)|32 = 0b0100000
        //   G1 = parity(0b0100000 & 0x79) = 1 (bit 5 matches)
        //   G2 = parity(0b0100000 & 0x5B) = 0 (no overlap)
        //   state → 16
        // First 4 coded bits: 1,1,1,0 → 0xE0
        let data = [0x80];
        let mut out = [0u8; 4];
        encode(&data, &mut out).unwrap();
        assert_eq!(out[0] & 0xF0, 0xE0);
    }

    #[test]
    fn roundtrip_hard_decision() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF];
        let mut encoded = [0u8; 32];
        encode(&data, &mut encoded).unwrap();

        let num_bits = (data.len() * 8 + MEMORY) * 2;
        let mut llrs = [0i16; 256];
        hard_to_llr(&encoded, num_bits, 127, &mut llrs);

        let mut decoded = [0u8; 4];
        let len = decode(&llrs[..num_bits], &mut decoded).unwrap();
        assert_eq!(len, 4);
        assert_eq!(decoded, data);
    }

    #[test]
    fn roundtrip_single_byte() {
        for val in [0x00, 0x01, 0x55, 0xAA, 0xFF] {
            let data = [val];
            let mut encoded = [0u8; 4];
            encode(&data, &mut encoded).unwrap();

            let num_bits = (8 + MEMORY) * 2;
            let mut llrs = [0i16; 32];
            hard_to_llr(&encoded, num_bits, 127, &mut llrs);

            let mut decoded = [0u8; 1];
            decode(&llrs[..num_bits], &mut decoded).unwrap();
            assert_eq!(decoded[0], val, "failed for 0x{val:02X}");
        }
    }

    #[test]
    fn roundtrip_large_frame() {
        let mut data = [0u8; 128];
        for i in 0..data.len() {
            data[i] = i as u8;
        }
        let mut encoded = [0u8; 300];
        encode(&data, &mut encoded).unwrap();

        let num_bits = (data.len() * 8 + MEMORY) * 2;
        let mut llrs = [0i16; 2200];
        hard_to_llr(&encoded, num_bits, 127, &mut llrs);

        let mut decoded = [0u8; 128];
        decode(&llrs[..num_bits], &mut decoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn corrects_bit_errors() {
        let data = [0x42, 0x37, 0x99, 0x10];
        let mut encoded = [0u8; 32];
        encode(&data, &mut encoded).unwrap();

        let num_bits = (data.len() * 8 + MEMORY) * 2;
        let mut llrs = [0i16; 256];
        hard_to_llr(&encoded, num_bits, 50, &mut llrs);

        // Flip some bits (simulate channel errors) by negating LLRs
        llrs[0] = -llrs[0];
        llrs[15] = -llrs[15];
        llrs[30] = -llrs[30];
        llrs[50] = -llrs[50];

        let mut decoded = [0u8; 4];
        decode(&llrs[..num_bits], &mut decoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn soft_decision_advantage() {
        // With soft decisions (varied magnitudes), the decoder should
        // still recover the correct data even with a flipped hard bit,
        // as long as the LLR magnitude for that bit is small.
        let data = [0xAB];
        let mut encoded = [0u8; 4];
        encode(&data, &mut encoded).unwrap();

        let num_bits = (8 + MEMORY) * 2;
        let mut llrs = [0i16; 32];
        hard_to_llr(&encoded, num_bits, 100, &mut llrs);

        // Flip bit 3 but make it a weak decision
        llrs[3] = -llrs[3].signum() * 5;

        let mut decoded = [0u8; 1];
        decode(&llrs[..num_bits], &mut decoded).unwrap();
        assert_eq!(decoded[0], 0xAB);
    }

    #[test]
    fn decode_odd_llr_count() {
        let llrs = [0i16; 5];
        let mut out = [0u8; 1];
        assert_eq!(decode(&llrs, &mut out), Err(ConvError::OddLlrCount));
    }

    #[test]
    fn decode_frame_too_short() {
        let llrs = [0i16; 4]; // 2 steps < MEMORY=6
        let mut out = [0u8; 1];
        assert_eq!(decode(&llrs, &mut out), Err(ConvError::FrameTooShort));
    }

    #[test]
    fn decode_output_too_small() {
        // 20 LLRs = 10 steps, 10 - 6 = 4 info bits → need 1 byte
        let llrs = [100i16; 20];
        let mut out = [0u8; 0];
        assert!(matches!(
            decode(&llrs, &mut out),
            Err(ConvError::BufferTooSmall { .. })
        ));
    }

    #[test]
    fn branch_table_symmetry() {
        // For state 0, input 0: output should be (0, 0)
        assert_eq!(BRANCH[0][0], (0, 0));
        // For state 0, input 1: reg = 0b1000000 = 64
        //   G1: 64 & 0x79 = 0b1000000 & 0b1111001 = 0b1000000 → parity=1
        //   G2: 64 & 0x5B = 0b1000000 & 0b1011011 = 0b1000000 → parity=1
        assert_eq!(BRANCH[0][1], (1, 1));
    }

    #[test]
    fn roundtrip_all_ones() {
        let data = [0xFF; 8];
        let mut encoded = [0u8; 24];
        encode(&data, &mut encoded).unwrap();

        let num_bits = (64 + MEMORY) * 2;
        let mut llrs = [0i16; 256];
        hard_to_llr(&encoded, num_bits, 127, &mut llrs);

        let mut decoded = [0u8; 8];
        decode(&llrs[..num_bits], &mut decoded).unwrap();
        assert_eq!(decoded, data);
    }
}
