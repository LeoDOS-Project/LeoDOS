//! CCSDS 123.0-B-2 Low-Complexity Lossless and Near-Lossless
//! Multispectral and Hyperspectral Image Compression.
//!
//! Implements the predictor and sample-adaptive entropy coder for
//! 3D image data (NX x NY x NZ). Supports lossless compression
//! with full or reduced prediction mode, wide/narrow
//! neighbor-oriented or column-oriented local sums, and default
//! weight initialization.
//!
//! # Limitations
//!
//! - Sample-adaptive entropy coder only (no hybrid/block-adaptive)
//! - BSQ encoding order only
//! - Lossless mode only (no near-lossless quantization)
//! - Default weight initialization only (no custom)
//! - No supplementary information tables
//! - Dynamic range D limited to 2..=16
//! - P (prediction bands) limited to 0..=15
//! - Image dimensions capped by `MAX_DIM`

/// Maximum number of prediction bands.
const MAX_P: usize = 15;
/// Maximum weight vector length (3 directional + MAX_P spectral).
const MAX_CZ: usize = MAX_P + 3;

/// Prediction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredictionMode {
    /// Full prediction (directional + spectral).
    Full,
    /// Reduced prediction (spectral only).
    Reduced,
}

/// Local sum type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSumType {
    /// Wide neighbor-oriented local sums.
    WideNeighbor,
    /// Narrow neighbor-oriented local sums.
    NarrowNeighbor,
    /// Wide column-oriented local sums.
    WideColumn,
    /// Narrow column-oriented local sums.
    NarrowColumn,
}

/// Compressor configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Image width (number of columns).
    pub nx: u16,
    /// Image height (number of rows).
    pub ny: u16,
    /// Number of spectral bands.
    pub nz: u16,
    /// Dynamic range in bits (2..=16).
    pub dynamic_range: u8,
    /// Whether samples are signed.
    pub signed_samples: bool,
    /// Number of prediction bands (0..=15).
    pub p: u8,
    /// Prediction mode.
    pub mode: PredictionMode,
    /// Local sum type.
    pub local_sum_type: LocalSumType,
    /// Weight component resolution (4..=19).
    pub omega: u8,
    /// Register size R (max{32, D+Omega+2} .. 64).
    pub register_size: u8,
    /// Weight update scaling exponent change interval
    /// (power of 2 in range 2^4..=2^11).
    pub t_inc: u16,
    /// Weight update initial parameter (-6..=9).
    pub v_min: i8,
    /// Weight update final parameter (-6..=9).
    pub v_max: i8,
    /// Unary length limit for entropy coder (8..=32).
    pub u_max: u8,
    /// Initial count exponent (1..=8).
    pub gamma_0: u8,
    /// Rescaling counter size (max{4, gamma_0+1}..=11).
    pub gamma_star: u8,
    /// Accumulator initialization constant (optional).
    /// If None, per-band k''_z values default to 0.
    pub accum_init_k: Option<u8>,
}

/// Compression/decompression error.
#[derive(Debug)]
pub enum Error {
    /// Invalid configuration parameter.
    InvalidConfig,
    /// Output buffer too small.
    OutputFull,
    /// Input bitstream truncated or malformed.
    Truncated,
    /// Image dimensions exceed compile-time limits.
    ImageTooLarge,
    /// Scratch buffer too small.
    ScratchTooSmall,
}

/// Compute the required scratch buffer size (in `i64` elements)
/// for a given image configuration.
///
/// The caller must provide `&mut [i64]` of at least this many
/// elements to `compress` / `decompress`.
pub fn scratch_len(nx: usize, nz: usize) -> usize {
    // sr_buf: nz * 2 * nx
    // weights: nz * MAX_CZ
    // accumulator: nz  (stored as i64, reinterpreted as u64)
    nz * 2 * nx + nz * MAX_CZ + nz
}

impl Config {
    fn validate(&self) -> Result<(), Error> {
        if self.nx == 0 || self.ny == 0 || self.nz == 0 {
            return Err(Error::InvalidConfig);
        }
        let d = self.dynamic_range;
        if d < 2 || d > 16 {
            return Err(Error::InvalidConfig);
        }
        if self.p > 15 {
            return Err(Error::InvalidConfig);
        }
        if self.omega < 4 || self.omega > 19 {
            return Err(Error::InvalidConfig);
        }
        let r_min = core::cmp::max(32, d as u8 + self.omega + 2);
        if self.register_size < r_min || self.register_size > 64 {
            return Err(Error::InvalidConfig);
        }
        if !self.t_inc.is_power_of_two()
            || self.t_inc < 16
            || self.t_inc > 2048
        {
            return Err(Error::InvalidConfig);
        }
        if self.v_min < -6 || self.v_min > 9 {
            return Err(Error::InvalidConfig);
        }
        if self.v_max < self.v_min || self.v_max > 9 {
            return Err(Error::InvalidConfig);
        }
        if self.u_max < 8 || self.u_max > 32 {
            return Err(Error::InvalidConfig);
        }
        if self.gamma_0 < 1 || self.gamma_0 > 8 {
            return Err(Error::InvalidConfig);
        }
        let g_min = core::cmp::max(4, self.gamma_0 + 1);
        if self.gamma_star < g_min || self.gamma_star > 11 {
            return Err(Error::InvalidConfig);
        }
        if self.nx == 1
            && self.mode == PredictionMode::Full
        {
            return Err(Error::InvalidConfig);
        }
        if self.nx == 1
            && matches!(
                self.local_sum_type,
                LocalSumType::WideNeighbor
                    | LocalSumType::NarrowNeighbor
            )
        {
            return Err(Error::InvalidConfig);
        }
        Ok(())
    }

    fn d(&self) -> u32 {
        self.dynamic_range as u32
    }

    fn s_min(&self) -> i64 {
        if self.signed_samples {
            -(1i64 << (self.d() - 1))
        } else {
            0
        }
    }

    fn s_max(&self) -> i64 {
        if self.signed_samples {
            (1i64 << (self.d() - 1)) - 1
        } else {
            (1i64 << self.d()) - 1
        }
    }

    fn s_mid(&self) -> i64 {
        if self.signed_samples {
            0
        } else {
            1i64 << (self.d() - 1)
        }
    }

    /// Number of local differences used for prediction in band z.
    fn c_z(&self, z: usize) -> usize {
        let p_star = core::cmp::min(z, self.p as usize);
        match self.mode {
            PredictionMode::Reduced => p_star,
            PredictionMode::Full => p_star + 3,
        }
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
        Self { buf, pos: 0, bit: 0 }
    }

    fn write_bits(
        &mut self,
        value: u64,
        n: u32,
    ) -> Result<(), Error> {
        for i in (0..n).rev() {
            let b = ((value >> i) & 1) as u8;
            if self.pos >= self.buf.len() {
                return Err(Error::OutputFull);
            }
            self.buf[self.pos] |= b << (7 - self.bit);
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.pos += 1;
            }
        }
        Ok(())
    }

    /// Pad to byte boundary with zeros.
    fn flush(&mut self) -> Result<(), Error> {
        if self.bit > 0 {
            self.bit = 0;
            self.pos += 1;
        }
        Ok(())
    }

    fn bytes_written(&self) -> usize {
        if self.bit > 0 {
            self.pos + 1
        } else {
            self.pos
        }
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
        Self { buf, pos: 0, bit: 0 }
    }

    fn read_bits(&mut self, n: u32) -> Result<u64, Error> {
        let mut val = 0u64;
        for _ in 0..n {
            if self.pos >= self.buf.len() {
                return Err(Error::Truncated);
            }
            let b = (self.buf[self.pos] >> (7 - self.bit)) & 1;
            val = (val << 1) | b as u64;
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.pos += 1;
            }
        }
        Ok(val)
    }

}

// ── Predictor ────────────────────────────────────────────

/// Get sample from the image. Returns s[z][y*nx + x].
fn get_sample(
    image: &[u16],
    nx: usize,
    ny: usize,
    nz: usize,
    z: usize,
    y: usize,
    x: usize,
) -> i64 {
    let _ = (ny, nz);
    image[z * ny * nx + y * nx + x] as i64
}

/// Default weight initialization (eq 33-34).
fn init_weights_default(
    cfg: &Config,
    z: usize,
    w: &mut [i64],
) {
    let p_star = core::cmp::min(z, cfg.p as usize);
    let omega = cfg.omega as i64;

    // Spectral weights
    let c_z = cfg.c_z(z);
    for j in 0..c_z {
        w[j] = 0;
    }

    let dir_offset = match cfg.mode {
        PredictionMode::Full => {
            // Directional weights = 0
            w[0] = 0;
            w[1] = 0;
            w[2] = 0;
            3
        }
        PredictionMode::Reduced => 0,
    };

    if p_star > 0 {
        // w^(1) = 7/8 * 2^omega
        w[dir_offset] = (7 * (1i64 << omega)) / 8;
        for i in 1..p_star {
            w[dir_offset + i] = w[dir_offset + i - 1] / 8;
        }
    }
}

/// Compute weight update scaling exponent rho(t) (eq 50).
fn scaling_exponent(cfg: &Config, t: usize) -> i64 {
    let v_min = cfg.v_min as i64;
    let v_max = cfg.v_max as i64;
    let t_inc = cfg.t_inc as i64;
    let nx = cfg.nx as i64;
    let d = cfg.d() as i64;
    let omega = cfg.omega as i64;

    let base = v_min + ((t as i64 - nx) / t_inc);
    let clamped = if base < v_min {
        v_min
    } else if base > v_max {
        v_max
    } else {
        base
    };
    clamped + d - omega
}

/// mod*_R function (eq 4): R-bit two's complement modular.
fn mod_star(x: i64, r: u32) -> i64 {
    let two_r = 1i64 << r;
    let half = 1i64 << (r - 1);
    ((x + half).rem_euclid(two_r)) - half
}

/// clip function (eq 5).
fn clip(x: i64, x_min: i64, x_max: i64) -> i64 {
    if x < x_min {
        x_min
    } else if x > x_max {
        x_max
    } else {
        x
    }
}

/// sgn+ function (eq 7).
fn sgn_plus(x: i64) -> i64 {
    if x >= 0 { 1 } else { -1 }
}

/// Predicted sample value (eq 39).
fn predicted_sample(s_tilde: i64) -> i64 {
    // floor(s_tilde / 2)
    if s_tilde >= 0 {
        s_tilde / 2
    } else {
        (s_tilde - 1) / 2
    }
}

/// Mapped quantizer index (eq 55-56) for lossless mode.
fn mapped_quantizer_index(
    cfg: &Config,
    delta: i64,
    s_hat: i64,
    _t: usize,
) -> u64 {
    let s_min = cfg.s_min();
    let s_max = cfg.s_max();

    // theta (eq 56 with m_z(t)=0 for lossless)
    let theta = core::cmp::min(s_hat - s_min, s_max - s_hat);

    // eq 55 with m_z(t)=0
    if delta.abs() > theta {
        (delta.abs() + theta) as u64
    } else if delta >= 0 {
        2 * delta as u64
    } else {
        2 * (-delta) as u64 - 1
    }
}

/// Inverse mapped quantizer index → prediction residual.
fn unmap_quantizer_index(
    cfg: &Config,
    mapped: u64,
    s_hat: i64,
) -> i64 {
    let s_min = cfg.s_min();
    let s_max = cfg.s_max();
    let theta =
        core::cmp::min(s_hat - s_min, s_max - s_hat);

    if mapped as i64 > 2 * theta {
        let abs_val = mapped as i64 - theta;
        // theta = min(s_hat-s_min, s_max-s_hat).
        // If s_hat closer to s_min, large residual goes up (+).
        // If s_hat closer to s_max, large residual goes down (-).
        if s_hat - s_min <= s_max - s_hat {
            abs_val
        } else {
            -abs_val
        }
    } else if mapped % 2 == 0 {
        (mapped / 2) as i64
    } else {
        -(((mapped + 1) / 2) as i64)
    }
}

// ── GPO2 Codeword (§5.4.3.2.2) ──────────────────────────

/// Write GPO2 codeword R_k(j).
fn write_gpo2(
    w: &mut BitWriter,
    j: u64,
    k: u32,
    u_max: u32,
    d: u32,
) -> Result<(), Error> {
    let quotient = j >> k;
    if quotient < u_max as u64 {
        // quotient zeros, then a one, then k LSBs
        for _ in 0..quotient {
            w.write_bits(0, 1)?;
        }
        w.write_bits(1, 1)?;
        if k > 0 {
            w.write_bits(j & ((1u64 << k) - 1), k)?;
        }
    } else {
        // u_max zeros, then D-bit representation of j
        for _ in 0..u_max {
            w.write_bits(0, 1)?;
        }
        w.write_bits(j, d)?;
    }
    Ok(())
}

/// Read GPO2 codeword R_k(j).
fn read_gpo2(
    r: &mut BitReader,
    k: u32,
    u_max: u32,
    d: u32,
) -> Result<u64, Error> {
    let mut quotient = 0u64;
    loop {
        let bit = r.read_bits(1)?;
        if bit == 1 {
            break;
        }
        quotient += 1;
        if quotient == u_max as u64 {
            // Read D-bit value directly
            return r.read_bits(d);
        }
    }
    let remainder = if k > 0 { r.read_bits(k)? } else { 0 };
    Ok((quotient << k) | remainder)
}

// ── Compress ─────────────────────────────────────────────

/// Compress a 3D image using CCSDS 123.0-B-2.
///
/// `image` is in BSQ order: `image[z * ny * nx + y * nx + x]`.
/// `scratch` must have at least `scratch_len(nx, nz)` elements.
///
/// Returns the number of bytes written to `out`.
pub fn compress(
    cfg: &Config,
    image: &[u16],
    out: &mut [u8],
    scratch: &mut [i64],
) -> Result<usize, Error> {
    cfg.validate()?;

    let nx = cfg.nx as usize;
    let ny = cfg.ny as usize;
    let nz = cfg.nz as usize;
    let d = cfg.d();
    let needed = scratch_len(nx, nz);

    if scratch.len() < needed {
        return Err(Error::ScratchTooSmall);
    }
    if image.len() < nx * ny * nz {
        return Err(Error::Truncated);
    }

    // Zero output and scratch
    for b in out.iter_mut() {
        *b = 0;
    }
    for s in scratch[..needed].iter_mut() {
        *s = 0;
    }

    // Partition scratch: sr_buf | weights | accumulator
    let sr_len = nz * 2 * nx;
    let w_len = nz * MAX_CZ;
    let (sr_buf, rest) = scratch.split_at_mut(sr_len);
    let (weights, acc_buf) = rest.split_at_mut(w_len);
    // Store accumulator as i64, reinterpret
    let acc = &mut acc_buf[..nz];

    let mut bw = BitWriter::new(out);
    write_header(cfg, &mut bw)?;

    // Initialize weights
    for z in 0..nz {
        let cz = cfg.c_z(z);
        let wbase = z * MAX_CZ;
        let mut wslice = [0i64; MAX_CZ];
        init_weights_default(cfg, z, &mut wslice);
        for j in 0..cz {
            weights[wbase + j] = wslice[j];
        }
    }

    // Initialize entropy coder
    let mut counter = 1u64 << cfg.gamma_0;
    for z in 0..nz {
        let k_z_pp = match cfg.accum_init_k {
            Some(k) => k as u32,
            None => 0,
        };
        let k_z_prime = if k_z_pp <= 30 - d {
            k_z_pp
        } else {
            2 * k_z_pp + d - 30
        };
        acc[z] = ((3 * (1u64 << (k_z_prime + 6)) - 49)
            * counter) as i64
            >> 7;
    }

    for z in 0..nz {
        for y in 0..ny {
            let cur_row = y % 2;
            let prev_row = (y + 1) % 2;

            for x in 0..nx {
                let t = y * nx + x;
                let s = get_sample(image, nx, ny, nz, z, y, x);

                if t == 0 {
                    let coded = if cfg.signed_samples {
                        (s - cfg.s_min()) as u64
                    } else {
                        s as u64
                    };
                    bw.write_bits(coded, d)?;
                    sr_buf[z * 2 * nx + cur_row * nx + x] = s;
                    continue;
                }

                let sigma = local_sum_with_buf(
                    cfg, sr_buf, nx, z, y, x, cur_row,
                    prev_row,
                );

                let mut u_vec = [0i64; MAX_CZ];
                let cz = cfg.c_z(z);
                if cz > 0 {
                    build_u_with_buf(
                        cfg, sr_buf, sigma, nx, z, y, x,
                        cur_row, prev_row, &mut u_vec,
                    );
                }

                let wbase = z * MAX_CZ;
                let mut d_hat = 0i64;
                for j in 0..cz {
                    d_hat += weights[wbase + j] * u_vec[j];
                }

                let s_tilde = high_res_predicted_buf(
                    cfg, d_hat, sigma, t, z, sr_buf, nx,
                );
                let s_hat = predicted_sample(s_tilde);
                let delta = s - s_hat;
                let mapped =
                    mapped_quantizer_index(cfg, delta, s_hat, t);

                let mut acc_u = acc[z] as u64;
                encode_sample(
                    cfg, &mut bw, mapped, &mut acc_u,
                    &mut counter, t, z,
                )?;
                acc[z] = acc_u as i64;

                sr_buf[z * 2 * nx + cur_row * nx + x] = s;

                // Weight update
                if cz > 0 {
                    let e = 2 * s - s_tilde;
                    let rho = scaling_exponent(cfg, t);
                    let omega_min =
                        -(1i64 << (cfg.omega as i64 + 2));
                    let omega_max =
                        (1i64 << (cfg.omega as i64 + 2)) - 1;

                    for j in 0..cz {
                        let inc = (sgn_plus(e)
                            * shift_right(u_vec[j], rho)
                            + 1)
                            / 2;
                        weights[wbase + j] = clip(
                            weights[wbase + j] + inc,
                            omega_min,
                            omega_max,
                        );
                    }
                }
            }
        }
    }

    bw.flush()?;
    Ok(bw.bytes_written())
}

/// Arithmetic right-shift with floor: floor(x * 2^(-shift)).
/// For positive shift, right-shifts. For negative, left-shifts.
fn shift_right(x: i64, shift: i64) -> i64 {
    if shift >= 0 {
        x >> shift
    } else {
        x << (-shift)
    }
}

/// Local sum using the 2-row buffer layout.
fn local_sum_with_buf(
    cfg: &Config,
    sr_buf: &[i64],
    nx: usize,
    z: usize,
    y: usize,
    x: usize,
    cur_row: usize,
    prev_row: usize,
) -> i64 {
    let base = z * 2 * nx;
    let get_cur =
        |xx: usize| -> i64 { sr_buf[base + cur_row * nx + xx] };
    let get_prev =
        |xx: usize| -> i64 { sr_buf[base + prev_row * nx + xx] };
    // For previous band access:
    let get_prev_band = |zz: usize, row: usize, xx: usize| -> i64 {
        sr_buf[zz * 2 * nx + row * nx + xx]
    };

    match cfg.local_sum_type {
        LocalSumType::WideNeighbor => {
            if y > 0 && x > 0 && x < nx - 1 {
                get_cur(x - 1)
                    + get_prev(x - 1)
                    + get_prev(x)
                    + get_prev(x + 1)
            } else if y > 0 && x == 0 {
                2 * (get_prev(x) + get_prev(x + 1))
            } else if y > 0 && x == nx - 1 {
                get_cur(x - 1)
                    + get_prev(x - 1)
                    + 2 * get_prev(x)
            } else {
                // y == 0, x > 0
                4 * get_cur(x - 1)
            }
        }
        LocalSumType::NarrowNeighbor => {
            if y > 0 && x > 0 && x < nx - 1 {
                get_prev(x - 1)
                    + 2 * get_prev(x)
                    + get_prev(x + 1)
            } else if y == 0 && x > 0 && z > 0 {
                let prev_band_row = if y == 0 {
                    // At y=0, the "previous" row is the last row
                    // of previous band. But with 2-row buffer we
                    // only keep 2 rows. For BSQ order, at y=0 of
                    // band z, band z-1 is fully processed. Its
                    // last row is at (ny-1) % 2.
                    (cfg.ny as usize - 1) % 2
                } else {
                    prev_row
                };
                4 * get_prev_band(z - 1, prev_band_row, x - 1)
            } else if y > 0 && x == 0 {
                2 * (get_prev(x) + get_prev(x + 1))
            } else if y > 0 && x == nx - 1 {
                2 * (get_prev(x - 1) + get_prev(x))
            } else {
                // y == 0, x > 0, z == 0
                4 * cfg.s_mid()
            }
        }
        LocalSumType::WideColumn => {
            if y > 0 {
                4 * get_prev(x)
            } else {
                // y == 0, x > 0
                4 * get_cur(x - 1)
            }
        }
        LocalSumType::NarrowColumn => {
            if y > 0 {
                4 * get_prev(x)
            } else if y == 0 && x > 0 && z > 0 {
                let prev_band_row =
                    (cfg.ny as usize - 1) % 2;
                4 * get_prev_band(z - 1, prev_band_row, x - 1)
            } else {
                4 * cfg.s_mid()
            }
        }
    }
}

/// Build U vector using 2-row buffer layout.
fn build_u_with_buf(
    cfg: &Config,
    sr_buf: &[i64],
    sigma: i64,
    nx: usize,
    z: usize,
    y: usize,
    x: usize,
    cur_row: usize,
    prev_row: usize,
    u: &mut [i64],
) {
    let p_star = core::cmp::min(z, cfg.p as usize);
    let base = z * 2 * nx;
    let get_cur =
        |xx: usize| -> i64 { sr_buf[base + cur_row * nx + xx] };
    let get_prev =
        |xx: usize| -> i64 { sr_buf[base + prev_row * nx + xx] };

    match cfg.mode {
        PredictionMode::Full => {
            // dN
            u[0] = if y > 0 {
                4 * get_prev(x) - sigma
            } else {
                0
            };
            // dW
            u[1] = if y > 0 {
                if x > 0 {
                    4 * get_cur(x - 1) - sigma
                } else {
                    4 * get_prev(x) - sigma
                }
            } else {
                0
            };
            // dNW
            u[2] = if y > 0 {
                if x > 0 {
                    4 * get_prev(x - 1) - sigma
                } else {
                    4 * get_prev(x) - sigma
                }
            } else {
                0
            };
            // Spectral central local diffs from previous bands
            for i in 1..=p_star {
                let bz = z - i;
                // Need sigma and sr for band bz at (y,x)
                // Recompute sigma for band bz
                let sigma_prev = local_sum_with_buf(
                    cfg, sr_buf, nx, bz, y, x, cur_row,
                    prev_row,
                );
                let bbase = bz * 2 * nx;
                let sr_val =
                    sr_buf[bbase + cur_row * nx + x];
                u[2 + i] = 4 * sr_val - sigma_prev;
            }
        }
        PredictionMode::Reduced => {
            for i in 1..=p_star {
                let bz = z - i;
                let sigma_prev = local_sum_with_buf(
                    cfg, sr_buf, nx, bz, y, x, cur_row,
                    prev_row,
                );
                let bbase = bz * 2 * nx;
                let sr_val =
                    sr_buf[bbase + cur_row * nx + x];
                u[i - 1] = 4 * sr_val - sigma_prev;
            }
        }
    }
}

/// High-res predicted value using buffer layout.
fn high_res_predicted_buf(
    cfg: &Config,
    d_hat: i64,
    sigma: i64,
    t: usize,
    z: usize,
    sr_buf: &[i64],
    nx: usize,
) -> i64 {
    let omega = cfg.omega as u32;
    let r = cfg.register_size as u32;
    let s_mid = cfg.s_mid();
    let s_min = cfg.s_min();
    let s_max = cfg.s_max();

    if t > 0 {
        let inner = d_hat
            + (1i64 << omega) * (sigma - 4 * s_mid)
            + (1i64 << (omega + 2)) * s_mid
            + (1i64 << (omega + 1));
        clip(
            mod_star(inner, r),
            (1i64 << (omega + 2)) * s_min
                + (1i64 << (omega + 1)),
            (1i64 << (omega + 2)) * s_max
                + (1i64 << (omega + 1)),
        )
    } else if t == 0 && cfg.p > 0 && z > 0 {
        // s_{z-1}(0) at (y=0, x=0)
        2 * sr_buf[(z - 1) * 2 * nx]
    } else {
        2 * s_mid
    }
}

/// Encode a single mapped quantizer index using sample-adaptive
/// coder (§5.4.3.2).
fn encode_sample(
    cfg: &Config,
    w: &mut BitWriter,
    mapped: u64,
    sigma_acc: &mut u64,
    counter: &mut u64,
    _t: usize,
    _z: usize,
) -> Result<(), Error> {
    let d = cfg.d();
    let u_max = cfg.u_max as u32;
    let gamma_star = cfg.gamma_star as u64;

    // Select k (eq 62)
    let k = if 2 * *counter > *sigma_acc + (49 * *counter >> 7) {
        0u32
    } else {
        // Find largest k such that
        // counter * 2^k <= sigma + floor(49/128 * counter)
        let threshold = *sigma_acc + (49 * *counter >> 7);
        let mut k = 0u32;
        while k < d - 2 {
            if *counter * (1u64 << (k + 1)) > threshold {
                break;
            }
            k += 1;
        }
        k
    };

    // Write GPO2 codeword
    write_gpo2(w, mapped, k, u_max, d)?;

    // Update accumulator and counter (eq 60-61)
    if *counter < (1u64 << gamma_star) - 1 {
        *sigma_acc += mapped;
        *counter += 1;
    } else {
        *sigma_acc = (*sigma_acc + mapped + 1) / 2;
        *counter = (*counter + 1) / 2;
    }

    Ok(())
}

// ── Header ───────────────────────────────────────────────

fn write_header(
    cfg: &Config,
    w: &mut BitWriter,
) -> Result<(), Error> {
    let d = cfg.d();

    // Image Metadata Essential (12 bytes = 96 bits)
    w.write_bits(0, 8)?; // User-Defined Data
    w.write_bits(cfg.nx as u64, 16)?; // X Size
    w.write_bits(cfg.ny as u64, 16)?; // Y Size
    w.write_bits(cfg.nz as u64, 16)?; // Z Size
    let sample_type = if cfg.signed_samples { 1 } else { 0 };
    w.write_bits(sample_type, 1)?; // Sample Type
    w.write_bits(0, 1)?; // Reserved
    let large_d = if d > 16 { 1u64 } else { 0 };
    w.write_bits(large_d, 1)?; // Large Dynamic Range Flag
    w.write_bits(d as u64 % 16, 4)?; // Dynamic Range
    w.write_bits(0, 1)?; // Sample Encoding Order (BSQ)
    w.write_bits(0, 16)?; // Sub-Frame Interleaving Depth
    w.write_bits(0, 2)?; // Reserved
    // Output Word Size (1 byte = value 1, encoded mod 8)
    w.write_bits(1, 3)?;
    // Entropy Coder Type: 00 = sample-adaptive
    w.write_bits(0, 2)?;
    w.write_bits(0, 1)?; // Reserved
    // Quantizer Fidelity Control: 00 = lossless
    w.write_bits(0, 2)?;
    w.write_bits(0, 2)?; // Reserved
    // Supplementary Information Table Count: 0
    w.write_bits(0, 4)?;

    // Predictor Metadata Primary (5 bytes = 40 bits)
    w.write_bits(0, 1)?; // Reserved
    w.write_bits(0, 1)?; // Sample Representative Flag = 0
    w.write_bits(cfg.p as u64, 4)?; // Number of Prediction Bands
    let pred_mode = match cfg.mode {
        PredictionMode::Full => 0u64,
        PredictionMode::Reduced => 1,
    };
    w.write_bits(pred_mode, 1)?;
    w.write_bits(0, 1)?; // Weight Exponent Offset Flag = 0
    let ls = match cfg.local_sum_type {
        LocalSumType::WideNeighbor => 0u64,
        LocalSumType::NarrowNeighbor => 1,
        LocalSumType::WideColumn => 2,
        LocalSumType::NarrowColumn => 3,
    };
    w.write_bits(ls, 2)?;
    let r_enc = cfg.register_size as u64 % 64;
    w.write_bits(r_enc, 6)?;
    let omega_enc = (cfg.omega - 4) as u64;
    w.write_bits(omega_enc, 4)?;
    let t_inc_log = {
        let mut v = cfg.t_inc;
        let mut l = 0u32;
        while v > 1 {
            v >>= 1;
            l += 1;
        }
        l
    };
    w.write_bits((t_inc_log - 4) as u64, 4)?;
    w.write_bits((cfg.v_min + 6) as u64, 4)?;
    w.write_bits((cfg.v_max + 6) as u64, 4)?;
    w.write_bits(0, 1)?; // Weight Exponent Offset Table Flag=0
    w.write_bits(0, 1)?; // Weight Init Method = default
    w.write_bits(0, 1)?; // Weight Init Table Flag = 0
    w.write_bits(0, 5)?; // Weight Init Resolution = 0

    // Entropy Coder Metadata (sample-adaptive)
    w.write_bits(cfg.u_max as u64 % 32, 5)?;
    let gamma_star_enc = (cfg.gamma_star - 4) as u64;
    w.write_bits(gamma_star_enc, 3)?;
    w.write_bits(cfg.gamma_0 as u64 % 8, 3)?;
    match cfg.accum_init_k {
        Some(k) => {
            w.write_bits(k as u64, 4)?;
            w.write_bits(0, 1)?; // No table
        }
        None => {
            w.write_bits(0b1111, 4)?; // "all ones" = not specified
            w.write_bits(0, 1)?; // No table
        }
    }

    Ok(())
}

// ── Decompress ───────────────────────────────────────────

/// Decompress a CCSDS 123.0-B-2 compressed image.
///
/// `scratch` must have at least `scratch_len(nx, nz)` elements
/// (the header is read first to determine nx/nz, so callers
/// should pre-allocate generously or use `read_header_only`).
///
/// Returns the config and number of samples written.
pub fn decompress(
    data: &[u8],
    image: &mut [u16],
    scratch: &mut [i64],
) -> Result<(Config, usize), Error> {
    let mut br = BitReader::new(data);
    let cfg = read_header(&mut br)?;

    let nx = cfg.nx as usize;
    let ny = cfg.ny as usize;
    let nz = cfg.nz as usize;
    let d = cfg.d();
    let needed = scratch_len(nx, nz);

    if scratch.len() < needed {
        return Err(Error::ScratchTooSmall);
    }
    if image.len() < nx * ny * nz {
        return Err(Error::OutputFull);
    }

    for s in scratch[..needed].iter_mut() {
        *s = 0;
    }

    let sr_len = nz * 2 * nx;
    let w_len = nz * MAX_CZ;
    let (sr_buf, rest) = scratch.split_at_mut(sr_len);
    let (weights, acc_buf) = rest.split_at_mut(w_len);
    let acc = &mut acc_buf[..nz];

    // Initialize weights
    for z in 0..nz {
        let cz = cfg.c_z(z);
        let wbase = z * MAX_CZ;
        let mut wslice = [0i64; MAX_CZ];
        init_weights_default(&cfg, z, &mut wslice);
        for j in 0..cz {
            weights[wbase + j] = wslice[j];
        }
    }

    // Initialize entropy coder
    let mut counter = 1u64 << cfg.gamma_0;
    for z in 0..nz {
        let k_z_pp = match cfg.accum_init_k {
            Some(k) => k as u32,
            None => 0,
        };
        let k_z_prime = if k_z_pp <= 30 - d {
            k_z_pp
        } else {
            2 * k_z_pp + d - 30
        };
        acc[z] = ((3 * (1u64 << (k_z_prime + 6)) - 49)
            * counter) as i64
            >> 7;
    }

    for z in 0..nz {
        for y in 0..ny {
            let cur_row = y % 2;
            let prev_row = (y + 1) % 2;

            for x in 0..nx {
                let t = y * nx + x;

                if t == 0 {
                    let raw = br.read_bits(d)? as i64;
                    let s = if cfg.signed_samples {
                        raw + cfg.s_min()
                    } else {
                        raw
                    };
                    image[z * ny * nx + y * nx + x] = s as u16;
                    sr_buf[z * 2 * nx + cur_row * nx + x] = s;
                    continue;
                }

                let sigma = local_sum_with_buf(
                    &cfg, sr_buf, nx, z, y, x, cur_row,
                    prev_row,
                );

                let mut u_vec = [0i64; MAX_CZ];
                let cz = cfg.c_z(z);
                if cz > 0 {
                    build_u_with_buf(
                        &cfg, sr_buf, sigma, nx, z, y, x,
                        cur_row, prev_row, &mut u_vec,
                    );
                }

                let wbase = z * MAX_CZ;
                let mut d_hat = 0i64;
                for j in 0..cz {
                    d_hat += weights[wbase + j] * u_vec[j];
                }

                let s_tilde = high_res_predicted_buf(
                    &cfg, d_hat, sigma, t, z, sr_buf, nx,
                );
                let s_hat = predicted_sample(s_tilde);

                let mut acc_u = acc[z] as u64;
                let mapped = decode_sample(
                    &cfg, &mut br, &mut acc_u,
                    &mut counter, t, z,
                )?;
                acc[z] = acc_u as i64;

                let delta =
                    unmap_quantizer_index(&cfg, mapped, s_hat);
                let s = s_hat + delta;

                image[z * ny * nx + y * nx + x] = s as u16;
                sr_buf[z * 2 * nx + cur_row * nx + x] = s;

                if cz > 0 {
                    let e = 2 * s - s_tilde;
                    let rho = scaling_exponent(&cfg, t);
                    let omega_min =
                        -(1i64 << (cfg.omega as i64 + 2));
                    let omega_max =
                        (1i64 << (cfg.omega as i64 + 2)) - 1;

                    for j in 0..cz {
                        let inc = (sgn_plus(e)
                            * shift_right(u_vec[j], rho)
                            + 1)
                            / 2;
                        weights[wbase + j] = clip(
                            weights[wbase + j] + inc,
                            omega_min,
                            omega_max,
                        );
                    }
                }
            }
        }
    }

    Ok((cfg, nx * ny * nz))
}

/// Decode a single mapped quantizer index.
fn decode_sample(
    cfg: &Config,
    r: &mut BitReader,
    sigma_acc: &mut u64,
    counter: &mut u64,
    _t: usize,
    _z: usize,
) -> Result<u64, Error> {
    let d = cfg.d();
    let u_max = cfg.u_max as u32;
    let gamma_star = cfg.gamma_star as u64;

    let k = if 2 * *counter > *sigma_acc + (49 * *counter >> 7) {
        0u32
    } else {
        let threshold = *sigma_acc + (49 * *counter >> 7);
        let mut k = 0u32;
        while k < d - 2 {
            if *counter * (1u64 << (k + 1)) > threshold {
                break;
            }
            k += 1;
        }
        k
    };

    let mapped = read_gpo2(r, k, u_max, d)?;

    // Update accumulator and counter
    if *counter < (1u64 << gamma_star) - 1 {
        *sigma_acc += mapped;
        *counter += 1;
    } else {
        *sigma_acc = (*sigma_acc + mapped + 1) / 2;
        *counter = (*counter + 1) / 2;
    }

    Ok(mapped)
}

fn read_header(r: &mut BitReader) -> Result<Config, Error> {
    // Image Metadata Essential
    let _user_data = r.read_bits(8)?;
    let nx = r.read_bits(16)? as u16;
    let ny = r.read_bits(16)? as u16;
    let nz = r.read_bits(16)? as u16;
    let signed_samples = r.read_bits(1)? == 1;
    let _reserved = r.read_bits(1)?;
    let large_d = r.read_bits(1)?;
    let d_enc = r.read_bits(4)? as u8;
    let d = if large_d == 1 {
        d_enc as u8 + 16
    } else if d_enc == 0 {
        16
    } else {
        d_enc
    };
    let _encoding_order = r.read_bits(1)?; // BSQ=0
    let _sub_frame_depth = r.read_bits(16)?;
    let _reserved = r.read_bits(2)?;
    let b_enc = r.read_bits(3)?;
    let _output_word_size = if b_enc == 0 { 8 } else { b_enc };
    let entropy_type = r.read_bits(2)?;
    if entropy_type != 0 {
        return Err(Error::InvalidConfig); // Only sample-adaptive
    }
    let _reserved = r.read_bits(1)?;
    let fidelity = r.read_bits(2)?;
    if fidelity != 0 {
        return Err(Error::InvalidConfig); // Only lossless
    }
    let _reserved = r.read_bits(2)?;
    let _tau = r.read_bits(4)?;

    // Predictor Metadata Primary
    let _reserved = r.read_bits(1)?;
    let _sr_flag = r.read_bits(1)?;
    let p = r.read_bits(4)? as u8;
    let pred_mode_bit = r.read_bits(1)?;
    let mode = if pred_mode_bit == 0 {
        PredictionMode::Full
    } else {
        PredictionMode::Reduced
    };
    let _weo_flag = r.read_bits(1)?;
    let ls_enc = r.read_bits(2)?;
    let local_sum_type = match ls_enc {
        0 => LocalSumType::WideNeighbor,
        1 => LocalSumType::NarrowNeighbor,
        2 => LocalSumType::WideColumn,
        _ => LocalSumType::NarrowColumn,
    };
    let r_enc = r.read_bits(6)? as u8;
    let register_size = if r_enc == 0 { 64 } else { r_enc };
    let omega = r.read_bits(4)? as u8 + 4;
    let t_inc_enc = r.read_bits(4)? as u32;
    let t_inc = 1u16 << (t_inc_enc + 4);
    let v_min = r.read_bits(4)? as i8 - 6;
    let v_max = r.read_bits(4)? as i8 - 6;
    let _weo_table_flag = r.read_bits(1)?;
    let _weight_init_method = r.read_bits(1)?;
    let _weight_init_table = r.read_bits(1)?;
    let _weight_init_res = r.read_bits(5)?;

    // Entropy Coder Metadata (sample-adaptive)
    let u_max_enc = r.read_bits(5)? as u8;
    let u_max = if u_max_enc == 0 { 32 } else { u_max_enc };
    let gamma_star = r.read_bits(3)? as u8 + 4;
    let gamma_0_enc = r.read_bits(3)? as u8;
    let gamma_0 = if gamma_0_enc == 0 { 8 } else { gamma_0_enc };
    let k_enc = r.read_bits(4)? as u8;
    let accum_init_k =
        if k_enc == 0b1111 { None } else { Some(k_enc) };
    let _table_flag = r.read_bits(1)?;

    Ok(Config {
        nx,
        ny,
        nz,
        dynamic_range: d,
        signed_samples,
        p,
        mode,
        local_sum_type,
        omega,
        register_size,
        t_inc,
        v_min,
        v_max,
        u_max,
        gamma_0,
        gamma_star,
        accum_init_k,
    })
}

// ── Tests ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config(
        nx: u16,
        ny: u16,
        nz: u16,
    ) -> Config {
        Config {
            nx,
            ny,
            nz,
            dynamic_range: 8,
            signed_samples: false,
            p: core::cmp::min(nz.saturating_sub(1), 3) as u8,
            mode: if nx > 1 {
                PredictionMode::Full
            } else {
                PredictionMode::Reduced
            },
            local_sum_type: if nx > 1 {
                LocalSumType::WideNeighbor
            } else {
                LocalSumType::WideColumn
            },
            omega: 8,
            register_size: 32,
            t_inc: 64,
            v_min: -1,
            v_max: 3,
            u_max: 16,
            gamma_0: 3,
            gamma_star: 6,
            accum_init_k: None,
        }
    }

    // scratch_len(4,3) = 3*2*4 + 3*18 + 3 = 81
    // scratch_len(8,4) = 4*2*8 + 4*18 + 4 = 140
    const SCRATCH: usize = 256;

    fn roundtrip(cfg: &Config, image: &[u16]) {
        let mut compressed = [0u8; 2048];
        let mut scratch = [0i64; SCRATCH];

        let n = compress(
            cfg, image, &mut compressed, &mut scratch,
        )
        .unwrap();
        assert!(n > 0);

        let mut decoded = [0u16; 128];
        let n_samples = image.len();
        for s in scratch.iter_mut() {
            *s = 0;
        }
        let (_, count) = decompress(
            &compressed[..n],
            &mut decoded[..n_samples],
            &mut scratch,
        )
        .unwrap();
        assert_eq!(count, n_samples);
        assert_eq!(&decoded[..n_samples], image);
    }

    #[test]
    fn roundtrip_constant() {
        let cfg = default_config(4, 4, 1);
        roundtrip(&cfg, &[128u16; 16]);
    }

    #[test]
    fn roundtrip_ramp() {
        let cfg = default_config(4, 4, 1);
        let mut image = [0u16; 16];
        for i in 0..16 {
            image[i] = (i * 16) as u16;
        }
        roundtrip(&cfg, &image);
    }

    #[test]
    fn roundtrip_multiband() {
        let cfg = default_config(4, 4, 3);
        let mut image = [0u16; 48];
        for z in 0..3 {
            for i in 0..16 {
                image[z * 16 + i] =
                    ((z * 50 + i * 10) % 256) as u16;
            }
        }
        roundtrip(&cfg, &image);
    }

    #[test]
    fn roundtrip_reduced_mode() {
        let cfg = Config {
            mode: PredictionMode::Reduced,
            ..default_config(4, 4, 2)
        };
        let mut image = [0u16; 32];
        for i in 0..32 {
            image[i] = (i * 7 % 256) as u16;
        }
        roundtrip(&cfg, &image);
    }

    #[test]
    fn roundtrip_16bit() {
        let cfg = Config {
            dynamic_range: 16,
            register_size: 32,
            ..default_config(4, 4, 1)
        };
        let mut image = [0u16; 16];
        for i in 0..16 {
            image[i] = (i as u16 * 4000) + 100;
        }
        roundtrip(&cfg, &image);
    }

    #[test]
    fn roundtrip_column_oriented() {
        let cfg = Config {
            local_sum_type: LocalSumType::WideColumn,
            ..default_config(4, 4, 2)
        };
        let mut image = [0u16; 32];
        for i in 0..32 {
            image[i] = (i * 13 % 256) as u16;
        }
        roundtrip(&cfg, &image);
    }

    #[test]
    fn mapped_index_roundtrip() {
        let cfg = default_config(4, 4, 1);
        for s_hat in [0i64, 50, 127, 200, 255] {
            for delta in [-50i64, -1, 0, 1, 50] {
                let s = s_hat + delta;
                if s < cfg.s_min() || s > cfg.s_max() {
                    continue;
                }
                let mapped = mapped_quantizer_index(
                    &cfg, delta, s_hat, 1,
                );
                let recovered = unmap_quantizer_index(
                    &cfg, mapped, s_hat,
                );
                assert_eq!(
                    recovered, delta,
                    "s_hat={s_hat} delta={delta} \
                     mapped={mapped}"
                );
            }
        }
    }

    #[test]
    fn gpo2_roundtrip() {
        let mut buf = [0u8; 64];
        for k in 0..4u32 {
            for j in 0u64..20 {
                for b in buf.iter_mut() {
                    *b = 0;
                }
                let mut w = BitWriter::new(&mut buf);
                write_gpo2(&mut w, j, k, 16, 8).unwrap();
                let mut r = BitReader::new(&buf);
                let decoded =
                    read_gpo2(&mut r, k, 16, 8).unwrap();
                assert_eq!(decoded, j, "k={k} j={j}");
            }
        }
    }

    #[test]
    fn header_roundtrip() {
        let cfg = default_config(8, 8, 4);
        let mut buf = [0u8; 64];
        let mut w = BitWriter::new(&mut buf);
        write_header(&cfg, &mut w).unwrap();

        let mut r = BitReader::new(&buf);
        let decoded = read_header(&mut r).unwrap();

        assert_eq!(decoded.nx, cfg.nx);
        assert_eq!(decoded.ny, cfg.ny);
        assert_eq!(decoded.nz, cfg.nz);
        assert_eq!(decoded.dynamic_range, cfg.dynamic_range);
        assert_eq!(decoded.p, cfg.p);
        assert_eq!(decoded.omega, cfg.omega);
        assert_eq!(decoded.v_min, cfg.v_min);
        assert_eq!(decoded.v_max, cfg.v_max);
        assert_eq!(decoded.gamma_0, cfg.gamma_0);
        assert_eq!(decoded.gamma_star, cfg.gamma_star);
    }

    #[test]
    fn single_band_no_prediction() {
        let cfg = Config {
            p: 0,
            mode: PredictionMode::Reduced,
            local_sum_type: LocalSumType::WideColumn,
            ..default_config(4, 4, 1)
        };
        roundtrip(&cfg, &[100u16; 16]);
    }
}
