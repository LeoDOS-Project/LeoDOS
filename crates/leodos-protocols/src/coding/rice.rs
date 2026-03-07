//! CCSDS 121.0-B-3 Lossless Data Compression (Rice coding).
//!
//! Block-adaptive entropy coder with unit-delay predictor. Each
//! block of J samples is coded with the option that minimizes the
//! total encoded length. Options: zero-block, second extension,
//! fundamental sequence, split-sample k=1..n-2, no compression.
//!
//! # Configuration
//!
//! | Parameter | Range | Description |
//! |-----------|-------|-------------|
//! | n | 1..32 | bits per sample |
//! | J | 8,16,32,64 | block size in samples |
//! | r | 0..4096 | reference sample interval (0=off) |

/// Maximum block size.
const MAX_J: usize = 64;

/// Compression parameters.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Bits per sample (1..=32).
    pub bits_per_sample: u32,
    /// Block size in samples.
    pub block_size: u32,
    /// Reference sample interval in blocks (0 = disabled).
    pub ref_interval: u32,
    /// Whether the unit-delay preprocessor is enabled.
    pub preprocessor: bool,
}

/// Compression/decompression error.
#[derive(Debug)]
pub enum Error {
    /// Invalid configuration.
    InvalidConfig,
    /// Output buffer too small.
    OutputFull,
    /// Input bitstream truncated or malformed.
    Truncated,
}

impl Config {
    /// ID field bit width (from Table 5-1).
    fn id_len(&self) -> u32 {
        match self.bits_per_sample {
            1..=2 => 1,
            3..=4 => 2,
            5..=8 => 3,
            9..=16 => 4,
            17..=32 => 5,
            _ => 0,
        }
    }

    fn validate(&self) -> Result<(), Error> {
        let n = self.bits_per_sample;
        if n == 0 || n > 32 {
            return Err(Error::InvalidConfig);
        }
        if !matches!(self.block_size, 8 | 16 | 32 | 64) {
            return Err(Error::InvalidConfig);
        }
        if self.ref_interval > 4096 {
            return Err(Error::InvalidConfig);
        }
        Ok(())
    }

    fn x_max(&self) -> u64 {
        (1u64 << self.bits_per_sample) - 1
    }
}

// ── Bit Writer ───────────────────────────────────────────

struct BitWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
    bit: u32,
}

impl<'a> BitWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        if !buf.is_empty() {
            buf[0] = 0;
        }
        Self { buf, pos: 0, bit: 8 }
    }

    fn write(&mut self, value: u64, n_bits: u32) {
        let mut rem = n_bits;
        while rem > 0 {
            let w = rem.min(self.bit);
            let shift = self.bit - w;
            let bits = ((value >> (rem - w)) & ((1u64 << w) - 1)) as u8;
            self.buf[self.pos] |= bits << shift;
            self.bit -= w;
            rem -= w;
            if self.bit == 0 {
                self.pos += 1;
                if self.pos < self.buf.len() {
                    self.buf[self.pos] = 0;
                }
                self.bit = 8;
            }
        }
    }

    fn write_fs(&mut self, m: u64) {
        // m zero bits
        let mut zeros = m;
        while zeros > 0 {
            let w = (zeros as u32).min(self.bit);
            self.bit -= w;
            zeros -= w as u64;
            if self.bit == 0 {
                self.pos += 1;
                if self.pos < self.buf.len() {
                    self.buf[self.pos] = 0;
                }
                self.bit = 8;
            }
        }
        // terminating 1
        self.write(1, 1);
    }

    fn bytes_written(&self) -> usize {
        if self.bit == 8 { self.pos } else { self.pos + 1 }
    }
}

// ── Bit Reader ───────────────────────────────────────────

struct BitReader<'a> {
    buf: &'a [u8],
    pos: usize,
    bit: u32,
}

impl<'a> BitReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0, bit: 8 }
    }

    fn read(&mut self, n_bits: u32) -> Result<u64, Error> {
        let mut result = 0u64;
        let mut rem = n_bits;
        while rem > 0 {
            if self.pos >= self.buf.len() {
                return Err(Error::Truncated);
            }
            let w = rem.min(self.bit);
            let shift = self.bit - w;
            let mask = ((1u32 << w) - 1) as u8;
            let bits = (self.buf[self.pos] >> shift) & mask;
            result = (result << w) | bits as u64;
            self.bit -= w;
            rem -= w;
            if self.bit == 0 {
                self.pos += 1;
                self.bit = 8;
            }
        }
        Ok(result)
    }

    fn read_fs(&mut self) -> Result<u64, Error> {
        let mut m = 0u64;
        loop {
            if self.read(1)? == 1 {
                return Ok(m);
            }
            m += 1;
        }
    }
}

// ── Preprocessor ─────────────────────────────────────────

/// Forward mapper: signed prediction error → unsigned mapped value.
fn map_error(delta: i64, x_hat: u64, x_max: u64) -> u64 {
    let theta = (x_hat).min(x_max - x_hat) as i64;
    if delta >= 0 && delta <= theta {
        2 * delta as u64
    } else if delta < 0 && (-delta) <= theta {
        (2 * (-delta)) as u64 - 1
    } else {
        (theta as u64 + delta.unsigned_abs())
    }
}

/// Inverse mapper: unsigned mapped value → signed prediction error.
fn unmap_error(d: u64, x_hat: u64, x_max: u64) -> i64 {
    let theta = (x_hat).min(x_max - x_hat) as i64;
    if (d as i64) <= 2 * theta {
        if d % 2 == 0 {
            (d / 2) as i64
        } else {
            -(((d + 1) / 2) as i64)
        }
    } else if x_hat as i64 <= (x_max - x_hat) as i64 {
        d as i64 - theta
    } else {
        theta - d as i64
    }
}

/// Preprocess one block. Returns the number of data samples
/// written to `mapped` (J-1 if reference block, J otherwise).
/// `prev` is the last sample of the previous block.
fn preprocess_block(
    samples: &[u32],
    mapped: &mut [u64; MAX_J],
    is_ref: bool,
    prev: u32,
    x_max: u64,
) -> usize {
    let j = samples.len();
    let start = if is_ref { 1 } else { 0 };
    let mut prev_val = if is_ref { samples[0] } else { prev };

    for i in start..j {
        let x = samples[i] as u64;
        let x_hat = prev_val as u64;
        let delta = x as i64 - x_hat as i64;
        mapped[i - start] = map_error(delta, x_hat, x_max);
        prev_val = samples[i];
    }
    j - start
}

// ── Cost Functions ───────────────────────────────────────

fn cost_split(data: &[u64], k: u32) -> u64 {
    let mut total = 0u64;
    for &d in data {
        total += (d >> k) + 1;
    }
    total + k as u64 * data.len() as u64
}

fn cost_second_ext(data: &[u64]) -> Option<u64> {
    if data.len() % 2 != 0 {
        return None;
    }
    let mut total = 0u64;
    for pair in data.chunks_exact(2) {
        let sum = pair[0] + pair[1];
        let gamma = sum * (sum + 1) / 2 + pair[1];
        total += gamma + 1;
    }
    Some(total)
}

fn cost_no_comp(data_len: usize, n: u32) -> u64 {
    data_len as u64 * n as u64
}

// ── Coding Option ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodingOption {
    ZeroBlock,
    SecondExtension,
    Split(u32),
    NoCompression,
}

// ── Encoder ──────────────────────────────────────────────

/// Compress samples using CCSDS 121.0-B-3.
///
/// `samples` contains unsigned n-bit sample values. The length
/// must be a multiple of the block size J. Returns the number
/// of bytes written to `output`.
pub fn compress(
    cfg: &Config,
    samples: &[u32],
    output: &mut [u8],
) -> Result<usize, Error> {
    cfg.validate()?;

    let n = cfg.bits_per_sample;
    let j = cfg.block_size as usize;
    let id_len = cfg.id_len();
    let x_max = cfg.x_max();

    if samples.is_empty() {
        return Ok(0);
    }
    if samples.len() % j != 0 {
        return Err(Error::InvalidConfig);
    }

    let num_blocks = samples.len() / j;
    let mut w = BitWriter::new(output);
    let mut prev_sample: u32 = 0;
    let mut zero_run: u32 = 0;
    let mut zero_ref: Option<u32> = None;
    let mut seg_remaining: u32 = seg_size(cfg, 0);

    for blk in 0..num_blocks {
        let block = &samples[blk * j..(blk + 1) * j];
        let is_ref = cfg.preprocessor
            && cfg.ref_interval > 0
            && blk % cfg.ref_interval as usize == 0;

        // Preprocess
        let mut mapped = [0u64; MAX_J];
        let data_len = if cfg.preprocessor {
            preprocess_block(block, &mut mapped, is_ref, prev_sample, x_max)
        } else {
            for (i, &s) in block.iter().enumerate() {
                mapped[i] = s as u64;
            }
            j
        };
        let data = &mapped[..data_len];

        // Update previous sample for next block's predictor
        prev_sample = block[j - 1];

        // Check all-zeros
        let all_zero = data.iter().all(|&d| d == 0);

        if all_zero {
            if zero_run == 0 {
                zero_ref = if is_ref { Some(block[0]) } else { None };
            }
            zero_run += 1;
            seg_remaining -= 1;

            let at_seg_end = seg_remaining == 0;
            let at_input_end = blk == num_blocks - 1;

            if at_seg_end || at_input_end {
                flush_zero_blocks(
                    &mut w, id_len, n, zero_ref, zero_run,
                    at_seg_end && zero_run >= 5,
                );
                zero_run = 0;
                zero_ref = None;
                if at_seg_end {
                    seg_remaining = seg_size(cfg, blk + 1);
                }
            }
            continue;
        }

        // Flush pending zero blocks before this non-zero block
        if zero_run > 0 {
            flush_zero_blocks(
                &mut w, id_len, n, zero_ref, zero_run, false,
            );
            zero_run = 0;
            zero_ref = None;
        }

        seg_remaining -= 1;
        if seg_remaining == 0 {
            seg_remaining = seg_size(cfg, blk + 1);
        }

        // Select best option
        let max_k = ((1u32 << id_len) - 3).min(n.saturating_sub(2));
        let option = select_option(data, n, id_len, max_k);

        // Write CDS
        match option {
            CodingOption::SecondExtension => {
                w.write(1, id_len + 1);
            }
            CodingOption::Split(k) => {
                w.write(k as u64 + 1, id_len);
            }
            CodingOption::NoCompression => {
                w.write((1u64 << id_len) - 1, id_len);
            }
            CodingOption::ZeroBlock => unreachable!(),
        }

        // Reference sample
        if is_ref {
            w.write(block[0] as u64, n);
        }

        // Encode data
        encode_data(&mut w, data, n, option);
    }

    Ok(w.bytes_written())
}

fn seg_size(cfg: &Config, block_idx: usize) -> u32 {
    if cfg.ref_interval == 0 {
        64
    } else {
        let blocks_in_interval = cfg.ref_interval;
        let pos_in_interval = block_idx as u32 % blocks_in_interval;
        let remaining = blocks_in_interval - pos_in_interval;
        remaining.min(64)
    }
}

fn flush_zero_blocks(
    w: &mut BitWriter,
    id_len: u32,
    n: u32,
    ref_sample: Option<u32>,
    count: u32,
    use_ros: bool,
) {
    // Zero-block ID
    w.write(0, id_len + 1);
    // Reference sample if present
    if let Some(r) = ref_sample {
        w.write(r as u64, n);
    }
    // FS codeword per Table 3-2
    if use_ros && count >= 5 {
        w.write_fs(4); // ROS
    } else if count <= 4 {
        w.write_fs(count as u64 - 1);
    } else {
        w.write_fs(count as u64);
    }
}

fn select_option(
    data: &[u64],
    n: u32,
    id_len: u32,
    max_k: u32,
) -> CodingOption {
    let mut best_cost = u64::MAX;
    let mut best = CodingOption::NoCompression;

    // No compression
    let nc_cost = cost_no_comp(data.len(), n) + id_len as u64;
    if nc_cost <= best_cost {
        best_cost = nc_cost;
        best = CodingOption::NoCompression;
    }

    // Second extension
    if let Some(se_cost) = cost_second_ext(data) {
        let total = se_cost + (id_len + 1) as u64;
        if total < best_cost {
            best_cost = total;
            best = CodingOption::SecondExtension;
        }
    }

    // Split-sample k=0..max_k (k=0 is FS)
    for k in 0..=max_k {
        let total = cost_split(data, k) + id_len as u64;
        if total < best_cost {
            best_cost = total;
            best = CodingOption::Split(k);
        }
    }

    best
}

fn encode_data(
    w: &mut BitWriter,
    data: &[u64],
    n: u32,
    option: CodingOption,
) {
    match option {
        CodingOption::Split(k) => {
            // All FS codes first
            for &d in data {
                w.write_fs(d >> k);
            }
            // Then all k-bit LSBs
            if k > 0 {
                let mask = (1u64 << k) - 1;
                for &d in data {
                    w.write(d & mask, k);
                }
            }
        }
        CodingOption::SecondExtension => {
            // Pairs with δ₁=0 prepended if reference block
            // (caller already arranged data with δ₁=0 for ref)
            for pair in data.chunks(2) {
                let d1 = pair[0];
                let d2 = if pair.len() > 1 { pair[1] } else { 0 };
                let sum = d1 + d2;
                let gamma = sum * (sum + 1) / 2 + d2;
                w.write_fs(gamma);
            }
        }
        CodingOption::NoCompression => {
            for &d in data {
                w.write(d, n);
            }
        }
        CodingOption::ZeroBlock => {}
    }
}

// ── Decoder ──────────────────────────────────────────────

/// Decompress CCSDS 121.0-B-3 data.
///
/// Returns the number of samples written to `output`.
pub fn decompress(
    cfg: &Config,
    input: &[u8],
    output: &mut [u32],
) -> Result<usize, Error> {
    cfg.validate()?;

    let n = cfg.bits_per_sample;
    let j = cfg.block_size as usize;
    let id_len = cfg.id_len();
    let x_max = cfg.x_max();
    let no_comp_id = (1u64 << id_len) - 1;

    let mut r = BitReader::new(input);
    let mut out_pos = 0usize;
    let mut blk_idx = 0usize;
    let mut prev_sample: u32 = 0;
    let mut seg_remaining: u32 = seg_size(cfg, 0);

    while out_pos + j <= output.len() {
        let is_ref = cfg.preprocessor
            && cfg.ref_interval > 0
            && blk_idx % cfg.ref_interval as usize == 0;

        // Read ID
        let id_val = r.read(id_len)?;

        if id_val == 0 {
            // Zero-Block or Second-Extension
            let extra = r.read(1)?;
            if extra == 0 {
                // Zero-Block
                let ref_val = if is_ref {
                    r.read(n)? as u32
                } else {
                    0
                };

                let fs_val = r.read_fs()?;
                let count = decode_zero_count(
                    fs_val, seg_remaining,
                );

                for i in 0..count as usize {
                    let blk_start = out_pos;
                    let this_ref = is_ref && i == 0;

                    if this_ref {
                        output[blk_start] = ref_val;
                    } else if cfg.preprocessor {
                        // Mapped value 0 → delta = 0
                        output[blk_start] = prev_sample;
                    } else {
                        output[blk_start] = 0;
                    }

                    // Fill remaining samples
                    for s in 1..j {
                        if cfg.preprocessor {
                            // delta=0 → sample = previous
                            output[blk_start + s] =
                                output[blk_start + s - 1];
                        } else {
                            output[blk_start + s] = 0;
                        }
                    }

                    // Handle preprocessor for reference block
                    if this_ref && cfg.preprocessor {
                        // First sample is reference (raw)
                        // Rest have mapped=0 → delta=0
                        for s in 1..j {
                            output[blk_start + s] =
                                output[blk_start + s - 1];
                        }
                    }

                    prev_sample = output[blk_start + j - 1];
                    out_pos += j;
                    blk_idx += 1;
                    seg_remaining -= 1;
                    if seg_remaining == 0 {
                        seg_remaining = seg_size(cfg, blk_idx);
                    }
                }
                continue;
            } else {
                // Second Extension
                decode_single_block(
                    &mut r, output, &mut out_pos, &mut prev_sample,
                    j, n, x_max, is_ref, cfg.preprocessor,
                    CodingOption::SecondExtension,
                )?;
            }
        } else if id_val == no_comp_id {
            decode_single_block(
                &mut r, output, &mut out_pos, &mut prev_sample,
                j, n, x_max, is_ref, cfg.preprocessor,
                CodingOption::NoCompression,
            )?;
        } else {
            let k = id_val as u32 - 1;
            decode_single_block(
                &mut r, output, &mut out_pos, &mut prev_sample,
                j, n, x_max, is_ref, cfg.preprocessor,
                CodingOption::Split(k),
            )?;
        }

        blk_idx += 1;
        seg_remaining -= 1;
        if seg_remaining == 0 {
            seg_remaining = seg_size(cfg, blk_idx);
        }
    }

    Ok(out_pos)
}

fn decode_zero_count(fs_val: u64, _seg_remaining: u32) -> u32 {
    // Table 3-2:
    //   FS(0..3) → 1..4 blocks
    //   FS(4)    → ROS (rest of segment, ≥5)
    //   FS(5..)  → 5.. blocks
    if fs_val <= 3 {
        fs_val as u32 + 1
    } else if fs_val == 4 {
        // ROS: rest of segment. Caller tracks seg_remaining.
        _seg_remaining
    } else {
        fs_val as u32
    }
}

fn decode_single_block(
    r: &mut BitReader,
    output: &mut [u32],
    out_pos: &mut usize,
    prev_sample: &mut u32,
    j: usize,
    n: u32,
    x_max: u64,
    is_ref: bool,
    preprocess: bool,
    option: CodingOption,
) -> Result<(), Error> {
    let blk_start = *out_pos;

    // Read reference sample
    let ref_val = if is_ref {
        r.read(n)? as u32
    } else {
        0
    };

    let data_len = if is_ref { j - 1 } else { j };
    let mut mapped = [0u64; MAX_J];

    match option {
        CodingOption::Split(k) => {
            // Read all FS codes
            for i in 0..data_len {
                mapped[i] = r.read_fs()? << k;
            }
            // Read all k-bit LSBs
            if k > 0 {
                for i in 0..data_len {
                    mapped[i] |= r.read(k)?;
                }
            }
        }
        CodingOption::SecondExtension => {
            // For ref blocks, δ₁=0 is prepended → J/2 pairs
            let se_data_len = if is_ref { j } else { data_len };
            let n_pairs = se_data_len / 2;
            let mut se_mapped = [0u64; MAX_J];
            let se_start = if is_ref { 1 } else { 0 };

            // δ₁ = 0 for reference blocks
            if is_ref {
                se_mapped[0] = 0;
            }

            // Decode pairs
            for p in 0..n_pairs {
                let gamma = r.read_fs()?;
                let (d1, d2) = decode_second_ext_pair(gamma);
                se_mapped[se_start + 2 * p] = d1;
                se_mapped[se_start + 2 * p + 1] = d2;
            }

            // Copy to mapped (skip the prepended 0 for ref)
            let copy_start = if is_ref { 1 } else { 0 };
            for i in 0..data_len {
                mapped[i] = se_mapped[copy_start + i];
            }
        }
        CodingOption::NoCompression => {
            for i in 0..data_len {
                mapped[i] = r.read(n)?;
            }
        }
        CodingOption::ZeroBlock => unreachable!(),
    }

    // Reconstruct samples
    if is_ref {
        output[blk_start] = ref_val;
    }

    let data_start = if is_ref { blk_start + 1 } else { blk_start };

    if preprocess {
        let mut prev = if is_ref {
            ref_val
        } else {
            *prev_sample
        };

        for i in 0..data_len {
            let delta = unmap_error(mapped[i], prev as u64, x_max);
            let sample = (prev as i64 + delta) as u32;
            output[data_start + i] = sample;
            prev = sample;
        }
    } else {
        for i in 0..data_len {
            output[data_start + i] = mapped[i] as u32;
        }
    }

    *prev_sample = output[blk_start + j - 1];
    *out_pos += j;
    Ok(())
}

fn decode_second_ext_pair(gamma: u64) -> (u64, u64) {
    // γ = (d1+d2)(d1+d2+1)/2 + d2
    // Find β = d1+d2 such that β(β+1)/2 ≤ γ < (β+1)(β+2)/2
    let mut beta = 0u64;
    let mut threshold = 0u64;
    loop {
        let next = threshold + beta + 1;
        if gamma < next {
            break;
        }
        threshold = next;
        beta += 1;
    }
    let d2 = gamma - threshold;
    let d1 = beta - d2;
    (d1, d2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(
        n: u32, j: u32, r: u32, preprocess: bool, samples: &[u32],
    ) {
        let cfg = Config {
            bits_per_sample: n,
            block_size: j,
            ref_interval: r,
            preprocessor: preprocess,
        };
        let mut compressed = [0u8; 4096];
        let len = compress(&cfg, samples, &mut compressed)
            .expect("compress");

        let mut decompressed = [0u32; 256];
        let dec_len = decompress(
            &cfg,
            &compressed[..len],
            &mut decompressed[..samples.len()],
        )
        .expect("decompress");

        assert_eq!(dec_len, samples.len());
        assert_eq!(&decompressed[..dec_len], samples);
    }

    #[test]
    fn roundtrip_constant() {
        let samples = [100u32; 16];
        roundtrip(8, 16, 1, true, &samples);
    }

    #[test]
    fn roundtrip_ramp() {
        let mut samples = [0u32; 16];
        for i in 0..16 {
            samples[i] = i as u32;
        }
        roundtrip(8, 16, 1, true, &samples);
    }

    #[test]
    fn roundtrip_no_preprocess() {
        let mut samples = [0u32; 16];
        for i in 0..16 {
            samples[i] = i as u32 * 3;
        }
        roundtrip(8, 16, 0, false, &samples);
    }

    #[test]
    fn roundtrip_16bit() {
        let mut samples = [0u32; 32];
        for i in 0..32 {
            samples[i] = 1000 + i as u32 * 10;
        }
        roundtrip(16, 32, 1, true, &samples);
    }

    #[test]
    fn roundtrip_all_zeros() {
        let samples = [0u32; 64];
        roundtrip(8, 16, 1, true, &samples);
    }

    #[test]
    fn roundtrip_random_like() {
        let mut samples = [0u32; 64];
        let mut v = 42u32;
        for s in samples.iter_mut() {
            v = v.wrapping_mul(1103515245).wrapping_add(12345);
            *s = (v >> 16) & 0xFF;
        }
        roundtrip(8, 16, 1, true, &samples);
    }

    #[test]
    fn roundtrip_small_block() {
        let mut samples = [0u32; 8];
        for i in 0..8 {
            samples[i] = i as u32;
        }
        roundtrip(8, 8, 1, true, &samples);
    }

    #[test]
    fn second_ext_pair_decode() {
        // γ=0 → (0,0)
        assert_eq!(decode_second_ext_pair(0), (0, 0));
        // γ=1 → β=1, d2=0, d1=1
        assert_eq!(decode_second_ext_pair(1), (1, 0));
        // γ=2 → β=1, d2=1, d1=0
        assert_eq!(decode_second_ext_pair(2), (0, 1));
        // γ=3 → β=2, d2=0, d1=2
        assert_eq!(decode_second_ext_pair(3), (2, 0));
        // γ=4 → β=2, d2=1, d1=1
        assert_eq!(decode_second_ext_pair(4), (1, 1));
        // γ=5 → β=2, d2=2, d1=0
        assert_eq!(decode_second_ext_pair(5), (0, 2));
    }

    #[test]
    fn mapper_roundtrip() {
        for x_hat in [0u64, 50, 100, 127, 200, 255] {
            for x in 0..=255u64 {
                let delta = x as i64 - x_hat as i64;
                let mapped = map_error(delta, x_hat, 255);
                let recovered = unmap_error(mapped, x_hat, 255);
                assert_eq!(
                    recovered, delta,
                    "x_hat={x_hat}, x={x}, mapped={mapped}"
                );
            }
        }
    }

    #[test]
    fn fs_write_read() {
        let mut buf = [0u8; 32];
        let mut w = BitWriter::new(&mut buf);
        w.write_fs(0);
        w.write_fs(1);
        w.write_fs(5);
        w.write_fs(0);
        let len = w.bytes_written();

        let mut r = BitReader::new(&buf[..len]);
        assert_eq!(r.read_fs().unwrap(), 0);
        assert_eq!(r.read_fs().unwrap(), 1);
        assert_eq!(r.read_fs().unwrap(), 5);
        assert_eq!(r.read_fs().unwrap(), 0);
    }

    #[test]
    fn id_len_values() {
        let c = |n| Config {
            bits_per_sample: n,
            block_size: 16,
            ref_interval: 0,
            preprocessor: false,
        };
        assert_eq!(c(1).id_len(), 1);
        assert_eq!(c(2).id_len(), 1);
        assert_eq!(c(3).id_len(), 2);
        assert_eq!(c(4).id_len(), 2);
        assert_eq!(c(8).id_len(), 3);
        assert_eq!(c(16).id_len(), 4);
        assert_eq!(c(32).id_len(), 5);
    }
}
