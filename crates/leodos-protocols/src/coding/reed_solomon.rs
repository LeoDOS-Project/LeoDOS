//! CCSDS Reed-Solomon (255,223) Forward Error Correction
//!
//! Implements RS(255,223) over GF(2^8) as specified in
//! CCSDS 131.0-B-5 (TM Synchronization and Channel Coding).
//!
//! # Parameters
//!
//! - Field polynomial: p(x) = x^8 + x^7 + x^2 + x + 1 (0x187)
//! - Primitive element α = 0x02 (the polynomial x)
//! - First consecutive root: α^112
//! - Code generator: g(x) = ∏(x - α^(112+i)) for i = 0..31
//! - 32 parity symbols per codeword
//! - Corrects up to 16 symbol errors per codeword
//!
//! # Interleaving
//!
//! Supports interleaving depths I = 1..5 as per CCSDS spec.
//! With interleaving depth I, the total codeblock is I×255 symbols
//! and can correct up to I×16 symbol errors.

/// Number of symbols in a full RS codeword.
pub const N: usize = 255;
/// Number of data symbols per codeword.
pub const K: usize = 223;
/// Number of parity symbols (2T where T=16).
pub const PARITY: usize = N - K; // 32
/// Maximum correctable symbol errors per codeword.
pub const T: usize = PARITY / 2; // 16
/// First consecutive root exponent.
const FCR: u8 = 112;

/// GF(2^8) arithmetic with CCSDS polynomial 0x187.
mod gf {
    /// Field polynomial: x^8 + x^7 + x^2 + x + 1.
    const POLY: u16 = 0x187;

    /// Exponential table: exp[i] = α^i mod p(x).
    /// Doubled to 512 entries so exp[a + b] works without mod.
    pub(super) static EXP: [u8; 512] = {
        let mut t = [0u8; 512];
        let mut val: u16 = 1;
        let mut i = 0;
        while i < 255 {
            t[i] = val as u8;
            t[i + 255] = val as u8;
            // val *= α (left-shift + reduce)
            val <<= 1;
            if val & 0x100 != 0 {
                val ^= POLY;
            }
            i += 1;
        }
        t[255] = t[0];
        t
    };

    /// Logarithm table: log[α^i] = i. log[0] is undefined.
    pub(super) static LOG: [u8; 256] = {
        let mut t = [0u8; 256];
        let mut i = 0u16;
        while i < 255 {
            t[EXP[i as usize] as usize] = i as u8;
            i += 1;
        }
        t
    };

    /// α^n.
    #[inline]
    pub fn exp(n: usize) -> u8 {
        EXP[n % 255]
    }

    /// log_α(x).
    #[inline]
    pub fn log(x: u8) -> u8 {
        LOG[x as usize]
    }

    /// a × b in GF(2^8).
    #[inline]
    pub fn mul(a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        EXP[(LOG[a as usize] as usize) + (LOG[b as usize] as usize)]
    }

    /// a + b in GF(2^8) (XOR).
    #[inline]
    pub fn add(a: u8, b: u8) -> u8 {
        a ^ b
    }

    /// a^(-1) in GF(2^8).
    #[inline]
    pub fn inv(a: u8) -> u8 {
        if a == 0 {
            return 0;
        }
        EXP[255 - LOG[a as usize] as usize]
    }

    /// a / b in GF(2^8).
    #[inline]
    pub fn div(a: u8, b: u8) -> u8 {
        if a == 0 {
            return 0;
        }
        mul(a, inv(b))
    }

    /// Evaluates polynomial at x using Horner's method.
    /// Descending order: poly[0] is the highest-degree coefficient.
    pub fn poly_eval(poly: &[u8], x: u8) -> u8 {
        if poly.is_empty() {
            return 0;
        }
        let mut result = poly[0];
        for &coef in &poly[1..] {
            result = add(mul(result, x), coef);
        }
        result
    }

    /// Evaluates polynomial at x using Horner's method.
    /// Ascending order: poly[0] is the constant term.
    pub fn poly_eval_asc(poly: &[u8], x: u8) -> u8 {
        if poly.is_empty() {
            return 0;
        }
        let mut result = poly[poly.len() - 1];
        for i in (0..poly.len() - 1).rev() {
            result = add(mul(result, x), poly[i]);
        }
        result
    }
}

/// Errors from Reed-Solomon operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RsError {
    /// Buffer too short.
    BufferTooShort {
        /// Required size.
        required: usize,
        /// Provided size.
        provided: usize,
    },
    /// Too many errors to correct.
    TooManyErrors,
    /// Invalid interleave depth (must be 1..=5).
    InvalidInterleaveDepth(u8),
}

/// The RS(255,223) generator polynomial coefficients.
///
/// g(x) = ∏(x - α^(FCR+i)) for i=0..31
/// Stored as 33 coefficients, highest degree first.
static GENERATOR: [u8; PARITY + 1] = {
    // Start with g(x) = 1
    let mut g = [0u8; PARITY + 1];
    g[0] = 1;
    let mut len = 1;

    let mut i = 0u16;
    while i < PARITY as u16 {
        // Multiply g by (x - α^(FCR+i))
        let root = gf::EXP[(FCR as u16 + i) as usize % 255];
        let mut j = len;
        while j > 0 {
            if g[j - 1] != 0 {
                g[j] = g[j] ^ gf::EXP
                    [((gf::LOG[g[j - 1] as usize] as u16
                        + gf::LOG[root as usize] as u16)
                        % 255) as usize];
            }
            j -= 1;
        }
        len += 1;
        i += 1;
    }
    g
};

/// Encodes `data` (up to 223 bytes) into `output` (255 bytes).
///
/// The output contains the original data followed by 32 parity bytes.
/// Returns the number of bytes written (always 255 for a full block).
pub fn encode(data: &[u8], output: &mut [u8]) -> Result<usize, RsError> {
    let data_len = data.len();
    if data_len > K {
        return Err(RsError::BufferTooShort {
            required: K,
            provided: data_len,
        });
    }
    if output.len() < N {
        return Err(RsError::BufferTooShort {
            required: N,
            provided: output.len(),
        });
    }

    let mut codeword = [0u8; N];
    codeword[..data_len].copy_from_slice(data);

    // Systematic encoding: divide data polynomial by g(x),
    // remainder becomes the 32 parity symbols.
    let mut remainder = [0u8; PARITY];
    for i in 0..K {
        let feedback = gf::add(codeword[i], remainder[0]);
        if feedback != 0 {
            for j in 0..PARITY - 1 {
                remainder[j] = gf::add(
                    remainder[j + 1],
                    gf::mul(feedback, GENERATOR[j + 1]),
                );
            }
            remainder[PARITY - 1] =
                gf::mul(feedback, GENERATOR[PARITY]);
        } else {
            remainder.copy_within(1..PARITY, 0);
            remainder[PARITY - 1] = 0;
        }
    }

    output[..K].copy_from_slice(&codeword[..K]);
    output[K..N].copy_from_slice(&remainder);

    Ok(N)
}

/// S_j = c(α^(FCR+j)) for j = 0..2T-1.
fn syndromes(codeword: &[u8; N]) -> [u8; PARITY] {
    let mut s = [0u8; PARITY];
    for j in 0..PARITY {
        let root = gf::exp(FCR as usize + j);
        s[j] = gf::poly_eval(codeword, root);
    }
    s
}

/// Berlekamp-Massey: finds error locator σ(x) from syndromes.
///
/// σ(x) = 1 + σ_1·x + σ_2·x² + ... stored ascending (σ[0] = 1).
/// Returns (sigma, num_errors).
fn berlekamp_massey(
    syn: &[u8; PARITY],
) -> Result<([u8; PARITY + 1], usize), RsError> {
    let mut sigma = [0u8; PARITY + 1];
    sigma[0] = 1;
    let mut old_sigma = [0u8; PARITY + 1];
    old_sigma[0] = 1;
    let mut l = 0usize;

    for n in 0..PARITY {
        // Compute discrepancy Δ_n
        let mut delta = syn[n];
        for i in 1..=l {
            delta = gf::add(delta, gf::mul(sigma[i], syn[n - i]));
        }

        // Shift old_sigma (multiply by x)
        let mut shifted = [0u8; PARITY + 1];
        for i in 1..=PARITY {
            shifted[i] = old_sigma[i - 1];
        }
        old_sigma = shifted;

        if delta != 0 {
            let mut scaled = [0u8; PARITY + 1];
            for i in 0..=PARITY {
                scaled[i] = gf::mul(delta, old_sigma[i]);
            }

            // Update connection length when 2L ≤ n
            if 2 * l <= n {
                let inv_delta = gf::inv(delta);
                for i in 0..=PARITY {
                    old_sigma[i] =
                        gf::mul(sigma[i], inv_delta);
                }
                l = n + 1 - l;
            }

            // σ ← σ + Δ·old_sigma
            for i in 0..=PARITY {
                sigma[i] = gf::add(sigma[i], scaled[i]);
            }
        }
    }

    if l > T {
        return Err(RsError::TooManyErrors);
    }

    Ok((sigma, l))
}

/// Chien search: finds error positions from σ(x).
///
/// σ(α^m) = 0 means there is an error at codeword position
/// (m + 254) % 255, because the error locator X_k = α^{254-pos}.
fn chien_search(
    sigma: &[u8; PARITY + 1],
    num_errors: usize,
) -> Result<[u8; T], RsError> {
    let mut positions = [0u8; T];
    let mut found = 0;

    for m in 0..N {
        let x = gf::exp(m);
        if gf::poly_eval_asc(&sigma[..=num_errors], x) == 0 {
            positions[found] = ((m + N - 1) % N) as u8;
            found += 1;
            if found == num_errors {
                break;
            }
        }
    }

    if found != num_errors {
        return Err(RsError::TooManyErrors);
    }

    Ok(positions)
}

/// Forney algorithm: computes error magnitudes.
///
/// e_k = X_k^{1-FCR} · Ω(X_k⁻¹) / σ'(X_k⁻¹)
fn forney(
    syn: &[u8; PARITY],
    sigma: &[u8; PARITY + 1],
    positions: &[u8],
    num_errors: usize,
) -> [u8; T] {
    // Error evaluator Ω(x) = S(x)·σ(x) mod x^{2T}
    let mut omega = [0u8; PARITY];
    for i in 0..PARITY {
        let mut val = 0u8;
        for j in 0..=i.min(num_errors) {
            val = gf::add(val, gf::mul(sigma[j], syn[i - j]));
        }
        omega[i] = val;
    }

    // Formal derivative σ'(x) in GF(2): only odd-index terms
    let mut sigma_deriv = [0u8; PARITY];
    for i in (1..=num_errors).step_by(2) {
        sigma_deriv[i - 1] = sigma[i];
    }

    let mut magnitudes = [0u8; T];
    for k in 0..num_errors {
        let pos = positions[k] as usize;
        // X_k = α^{254-pos}, X_k⁻¹ = α^{pos+1}
        let x_k_log = (N - 1 - pos) % N;
        let x_k_inv = gf::exp((N - x_k_log) % N);

        let omega_val =
            gf::poly_eval_asc(&omega[..PARITY], x_k_inv);
        let deriv_val =
            gf::poly_eval_asc(&sigma_deriv[..num_errors], x_k_inv);

        let power_log =
            (x_k_log * (1 + N - FCR as usize)) % N;
        let power = gf::exp(power_log);

        magnitudes[k] =
            gf::mul(power, gf::div(omega_val, deriv_val));
    }

    magnitudes
}

/// Decodes and corrects a 255-byte RS codeword in-place.
///
/// Returns the number of corrected symbol errors, or an error
/// if there are too many to correct (>16).
pub fn decode(codeword: &mut [u8; N]) -> Result<usize, RsError> {
    let syn = syndromes(codeword);

    if syn.iter().all(|&s| s == 0) {
        return Ok(0);
    }

    let (sigma, num_errors) = berlekamp_massey(&syn)?;
    let positions = chien_search(&sigma, num_errors)?;
    let magnitudes = forney(&syn, &sigma, &positions, num_errors);

    for i in 0..num_errors {
        let pos = positions[i] as usize;
        codeword[pos] = gf::add(codeword[pos], magnitudes[i]);
    }

    // Verify correction
    let check = syndromes(codeword);
    if check.iter().all(|&s| s == 0) {
        Ok(num_errors)
    } else {
        Err(RsError::TooManyErrors)
    }
}

/// Encodes with interleaving depth I (1..=5).
///
/// Input: `data` of length I×223 bytes.
/// Output: `output` of length I×255 bytes.
pub fn encode_interleaved(
    data: &[u8],
    depth: u8,
    output: &mut [u8],
) -> Result<usize, RsError> {
    if depth == 0 || depth > 5 {
        return Err(RsError::InvalidInterleaveDepth(depth));
    }
    let d = depth as usize;
    let total_data = d * K;
    let total_code = d * N;

    if data.len() < total_data {
        return Err(RsError::BufferTooShort {
            required: total_data,
            provided: data.len(),
        });
    }
    if output.len() < total_code {
        return Err(RsError::BufferTooShort {
            required: total_code,
            provided: output.len(),
        });
    }

    for i in 0..d {
        // De-interleave: pick every d-th symbol starting at i
        let mut block = [0u8; K];
        for j in 0..K {
            block[j] = data[j * d + i];
        }

        let mut codeword = [0u8; N];
        encode(&block, &mut codeword)?;

        // Re-interleave output
        for j in 0..N {
            output[j * d + i] = codeword[j];
        }
    }

    Ok(total_code)
}

/// Decodes with interleaving depth I (1..=5).
///
/// Operates in-place on a buffer of I×255 bytes.
/// Returns total number of corrected symbol errors.
pub fn decode_interleaved(
    data: &mut [u8],
    depth: u8,
) -> Result<usize, RsError> {
    if depth == 0 || depth > 5 {
        return Err(RsError::InvalidInterleaveDepth(depth));
    }
    let d = depth as usize;
    let total_code = d * N;

    if data.len() < total_code {
        return Err(RsError::BufferTooShort {
            required: total_code,
            provided: data.len(),
        });
    }

    let mut total_corrected = 0;

    for i in 0..d {
        // De-interleave
        let mut codeword = [0u8; N];
        for j in 0..N {
            codeword[j] = data[j * d + i];
        }

        let corrected = decode(&mut codeword)?;
        total_corrected += corrected;

        // Re-interleave corrected data back
        for j in 0..N {
            data[j * d + i] = codeword[j];
        }
    }

    Ok(total_corrected)
}

/// RS(255,223) encoder implementing [`FecEncoder`](super::FecEncoder).
pub struct ReedSolomonEncoder {
    interleave_depth: u8,
}

impl ReedSolomonEncoder {
    /// Creates an encoder with the given interleave depth (1..=5).
    pub fn new(interleave_depth: u8) -> Self {
        Self { interleave_depth }
    }
}

impl super::FecEncoder for ReedSolomonEncoder {
    type Error = RsError;

    fn encode(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error> {
        if self.interleave_depth <= 1 {
            encode(data, output)
        } else {
            encode_interleaved(data, self.interleave_depth, output)
        }
    }
}

/// RS(255,223) decoder implementing [`FecDecoder`](super::FecDecoder).
pub struct ReedSolomonDecoder {
    interleave_depth: u8,
}

impl ReedSolomonDecoder {
    /// Creates a decoder with the given interleave depth (1..=5).
    pub fn new(interleave_depth: u8) -> Self {
        Self { interleave_depth }
    }
}

impl super::FecDecoder for ReedSolomonDecoder {
    type Error = RsError;

    fn decode(&self, data: &mut [u8]) -> Result<usize, Self::Error> {
        if self.interleave_depth <= 1 {
            if data.len() < N {
                return Err(RsError::BufferTooShort { required: N, provided: data.len() });
            }
            let codeword: &mut [u8; N] = (&mut data[..N]).try_into().unwrap();
            decode(codeword)
        } else {
            decode_interleaved(data, self.interleave_depth)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gf_basic_arithmetic() {
        // α^0 = 1
        assert_eq!(gf::exp(0), 1);
        // α^1 = 2
        assert_eq!(gf::exp(1), 2);
        // α^255 = α^0 = 1 (field wraps)
        assert_eq!(gf::exp(255), 1);
        // a * 1 = a
        assert_eq!(gf::mul(42, 1), 42);
        // a * 0 = 0
        assert_eq!(gf::mul(42, 0), 0);
        // a * inv(a) = 1
        for a in 1..=255u8 {
            assert_eq!(gf::mul(a, gf::inv(a)), 1);
        }
    }

    #[test]
    fn gf_log_exp_inverse() {
        for i in 0..255usize {
            let a = gf::exp(i);
            assert_eq!(gf::log(a) as usize, i);
        }
    }

    #[test]
    fn encode_no_error_decode() {
        let data = [0xAB; K];
        let mut codeword = [0u8; N];
        encode(&data, &mut codeword).unwrap();

        // Data portion should match
        assert_eq!(&codeword[..K], &data);

        // Parity should be non-trivial
        assert!(codeword[K..].iter().any(|&b| b != 0));

        // Decode should find 0 errors
        let corrected = decode(&mut codeword).unwrap();
        assert_eq!(corrected, 0);
    }

    #[test]
    fn encode_decode_single_error() {
        let data = [0x42; K];
        let mut codeword = [0u8; N];
        encode(&data, &mut codeword).unwrap();

        // Introduce 1 error
        codeword[50] ^= 0xFF;

        let corrected = decode(&mut codeword).unwrap();
        assert_eq!(corrected, 1);
        assert_eq!(&codeword[..K], &[0x42; K]);
    }

    #[test]
    fn encode_decode_max_errors() {
        let data = [0x13; K];
        let mut codeword = [0u8; N];
        encode(&data, &mut codeword).unwrap();
        let original = codeword;

        // Introduce T (16) errors at different positions
        for i in 0..T {
            codeword[i * 15] ^= ((i as u8) + 1).wrapping_mul(0x37);
        }

        let corrected = decode(&mut codeword).unwrap();
        assert_eq!(corrected, T);
        assert_eq!(codeword, original);
    }

    #[test]
    fn too_many_errors() {
        let data = [0x00; K];
        let mut codeword = [0u8; N];
        encode(&data, &mut codeword).unwrap();

        // Introduce T+1 errors — should fail
        for i in 0..=T {
            codeword[i] ^= 0xFF;
        }

        let result = decode(&mut codeword);
        assert!(result.is_err());
    }

    #[test]
    fn generator_polynomial_degree() {
        // Generator should have degree PARITY (32)
        assert_eq!(GENERATOR[0], 1); // monic
        // At least some non-zero coefficients
        assert!(GENERATOR[1..].iter().any(|&c| c != 0));
    }

    #[test]
    fn generator_roots() {
        // Each α^(FCR+i) for i=0..31 should be a root of g(x)
        for i in 0..PARITY {
            let root = gf::exp(FCR as usize + i);
            let val = gf::poly_eval(&GENERATOR, root);
            assert_eq!(val, 0, "α^{} should be a root", FCR as usize + i);
        }
    }

    #[test]
    fn encode_zeros() {
        let data = [0u8; K];
        let mut codeword = [0u8; N];
        encode(&data, &mut codeword).unwrap();
        // All-zero data should produce all-zero codeword
        assert!(codeword.iter().all(|&b| b == 0));
    }

    #[test]
    fn interleaved_encode_decode_depth_1() {
        let data = [0x55; K];
        let mut output = [0u8; N];
        let len = encode_interleaved(&data, 1, &mut output).unwrap();
        assert_eq!(len, N);

        let corrected =
            decode_interleaved(&mut output, 1).unwrap();
        assert_eq!(corrected, 0);
        assert_eq!(&output[..K], &data);
    }

    #[test]
    fn interleaved_depth_2_with_errors() {
        let data = [0xAA; 2 * K];
        let mut output = [0u8; 2 * N];
        encode_interleaved(&data, 2, &mut output).unwrap();

        // Corrupt some symbols in each interleaved codeword
        output[0] ^= 0x11; // affects codeword 0
        output[1] ^= 0x22; // affects codeword 1

        let corrected =
            decode_interleaved(&mut output, 2).unwrap();
        assert_eq!(corrected, 2);
    }

    #[test]
    fn invalid_interleave_depth() {
        let data = [0u8; K];
        let mut output = [0u8; N];
        assert!(matches!(
            encode_interleaved(&data, 0, &mut output),
            Err(RsError::InvalidInterleaveDepth(0))
        ));
        assert!(matches!(
            encode_interleaved(&data, 6, &mut output),
            Err(RsError::InvalidInterleaveDepth(6))
        ));
    }

    #[test]
    fn parity_error_correction() {
        let data = [0x77; K];
        let mut codeword = [0u8; N];
        encode(&data, &mut codeword).unwrap();
        let original = codeword;

        // Corrupt only parity bytes
        codeword[K] ^= 0xFF;
        codeword[K + 5] ^= 0xAA;

        let corrected = decode(&mut codeword).unwrap();
        assert_eq!(corrected, 2);
        assert_eq!(codeword, original);
    }

    #[test]
    fn end_to_end_example() {
        // "HELLO" padded with zeros
        let mut data = [0u8; K];
        data[0] = 72;  // H
        data[1] = 69;  // E
        data[2] = 76;  // L
        data[3] = 76;  // L
        data[4] = 79;  // O

        // Encode: 223 data bytes -> 255 byte codeword
        let mut codeword = [0u8; N];
        encode(&data, &mut codeword).unwrap();

        // Data unchanged at front
        assert_eq!(&codeword[..5], &[72, 69, 76, 76, 79]);
        // Parity appended at back
        assert_eq!(
            &codeword[K..K + 8],
            &[243, 147, 197, 58, 154, 156, 250, 218]
        );

        // Corrupt byte 2: 'L' (76) -> 255
        codeword[2] = 255;

        // Decode: finds and fixes the error
        let corrected = decode(&mut codeword).unwrap();
        assert_eq!(corrected, 1);
        assert_eq!(codeword[2], 76); // 'L' restored
    }
}
