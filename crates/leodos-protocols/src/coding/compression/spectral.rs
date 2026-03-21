//! CCSDS 122.1-B-1 Spectral Preprocessing Transform.
//!
//! Provides spectral decorrelation for multispectral and
//! hyperspectral images before 2D compression (CCSDS 122.0).
//!
//! The pipeline is:
//!   Input → Upshift → Spectral Transform → Downshift → 2D Encoder
//!
//! Three spectral transforms are defined:
//! - Identity (passthrough)
//! - IWT (Integer Wavelet Transform, CDF 5/3, 5 levels)
//! - POT (Pairwise Orthogonal Transform, approximates KLT)
//!
//! The IWT is applied independently along the spectral axis (z)
//! for each spatial pixel (x, y).

/// Upshift: multiply each sample by 2^u (Section 3.2.2, Eq. 11).
pub fn upshift(image: &mut [i32], u: u32) {
    for sample in image.iter_mut() {
        *sample <<= u;
    }
}

/// Downshift: multiply each sample by 2^-d, rounding to nearest
/// integer (Section 3.2.3, Eq. 13).
pub fn downshift(image: &mut [i32], d: u32) {
    if d == 0 {
        return;
    }
    let half = 1i32 << (d - 1);
    for sample in image.iter_mut() {
        *sample = (*sample + half) >> d;
    }
}

/// Number of IWT decomposition levels (Section 4.4.4).
pub const IWT_LEVELS: usize = 5;

/// Single-level forward IWT decomposition (CDF 5/3 lifting).
///
/// Splits input `x` of length `n` into low-frequency `l` and
/// high-frequency `h` coefficients (Section 4.4.3.1, Eq. 22-23).
pub fn iwt_forward_single(x: &[i32], l: &mut [i32], h: &mut [i32]) {
    let n = x.len();
    if n <= 1 {
        if n == 1 {
            l[0] = x[0];
        }
        return;
    }

    let p = (n + 1) / 2;
    let q = n / 2;

    for k in 0..q {
        let is_last = k == q - 1 && n % 2 == 0;
        h[k] = if is_last {
            x[2 * k + 1] - x[2 * k]
        } else {
            x[2 * k + 1] - ((x[2 * k] + x[2 * k + 2]) / 2)
        };
    }

    for k in 0..p {
        l[k] = if k == 0 {
            x[0] + ((h[0] + 1) / 2)
        } else if k == p - 1 && n % 2 != 0 {
            x[2 * k] + ((h[k - 1] + 1) / 2)
        } else {
            x[2 * k] + ((h[k - 1] + h[k] + 2) / 4)
        };
    }
}

/// Single-level inverse IWT decomposition (Section 4.4.3.2, Eq. 25-26).
pub fn iwt_inverse_single(l: &[i32], h: &[i32], x: &mut [i32]) {
    let p = l.len();
    let q = h.len();
    let n = p + q;

    if n <= 1 {
        if n == 1 {
            x[0] = l[0];
        }
        return;
    }

    for k in 0..p {
        x[2 * k] = if k == 0 {
            l[0] - ((h[0] + 1) / 2)
        } else if k == p - 1 && n % 2 != 0 {
            l[k] - ((h[k - 1] + 1) / 2)
        } else {
            l[k] - ((h[k - 1] + h[k] + 2) / 4)
        };
    }

    for k in 0..q {
        let is_last = k == q - 1 && n % 2 == 0;
        x[2 * k + 1] = if is_last {
            h[k] + x[2 * k]
        } else {
            h[k] + ((x[2 * k] + x[2 * k + 2]) / 2)
        };
    }
}

/// Five-level forward IWT decomposition (Section 4.4.4).
///
/// Output order: L5, H5, H4, H3, H2, H1 (Eq. 27).
/// `buf` must have length >= `n`. `scratch` must have length >= `n`.
pub fn iwt_forward_5(input: &[i32], output: &mut [i32], scratch: &mut [i32]) {
    let n = input.len();
    output[..n].copy_from_slice(input);

    let mut current_len = n;
    for _ in 0..IWT_LEVELS {
        if current_len <= 1 {
            break;
        }
        let p = (current_len + 1) / 2;
        let q = current_len / 2;

        let (src, _) = output.split_at(current_len);
        let src_copy: heapless::Vec<i32, 4096> = src.iter().copied().collect();

        let (scratch_l, scratch_h) = scratch.split_at_mut(p);
        iwt_forward_single(&src_copy[..current_len], scratch_l, &mut scratch_h[..q]);

        output[..p].copy_from_slice(&scratch[..p]);
        output[p..p + q].copy_from_slice(&scratch[p..p + q]);

        current_len = p;
    }
}

/// Five-level inverse IWT decomposition (Section 4.4.4.2).
pub fn iwt_inverse_5(input: &[i32], output: &mut [i32], scratch: &mut [i32]) {
    let n = input.len();
    output[..n].copy_from_slice(input);

    let mut sizes = [0usize; IWT_LEVELS + 1];
    sizes[0] = n;
    for i in 0..IWT_LEVELS {
        sizes[i + 1] = (sizes[i] + 1) / 2;
        if sizes[i] <= 1 {
            break;
        }
    }

    let levels_used = sizes.iter().take_while(|&&s| s > 1).count().min(IWT_LEVELS);

    for level in (0..levels_used).rev() {
        let current_len = sizes[level];
        let p = (current_len + 1) / 2;
        let q = current_len / 2;

        scratch[..p].copy_from_slice(&output[..p]);
        scratch[p..p + q].copy_from_slice(&output[p..p + q]);

        iwt_inverse_single(&scratch[..p], &scratch[p..p + q], &mut output[..current_len]);
    }
}

/// Spectral transform selection.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SpectralTransform {
    /// No spectral decorrelation (Section 4.3).
    Identity,
    /// Integer Wavelet Transform, 5 levels (Section 4.4).
    Iwt,
}

/// Apply the spectral transform along the z-axis for a 3D image.
///
/// Image layout: `image[z * nx * ny + y * nx + x]` (band-interleaved by pixel).
/// `nx` × `ny` spatial, `nz` spectral bands.
pub fn transform_spectral(
    image: &mut [i32],
    nx: usize,
    ny: usize,
    nz: usize,
    transform: SpectralTransform,
) {
    match transform {
        SpectralTransform::Identity => {}
        SpectralTransform::Iwt => {
            let mut col = [0i32; 4096];
            let mut out = [0i32; 4096];
            let mut scratch = [0i32; 4096];
            assert!(nz <= 4096);

            for y in 0..ny {
                for x in 0..nx {
                    for z in 0..nz {
                        col[z] = image[z * nx * ny + y * nx + x];
                    }
                    iwt_forward_5(&col[..nz], &mut out[..nz], &mut scratch[..nz]);
                    for z in 0..nz {
                        image[z * nx * ny + y * nx + x] = out[z];
                    }
                }
            }
        }
    }
}

/// Apply the inverse spectral transform along the z-axis.
pub fn inverse_transform_spectral(
    image: &mut [i32],
    nx: usize,
    ny: usize,
    nz: usize,
    transform: SpectralTransform,
) {
    match transform {
        SpectralTransform::Identity => {}
        SpectralTransform::Iwt => {
            let mut col = [0i32; 4096];
            let mut out = [0i32; 4096];
            let mut scratch = [0i32; 4096];
            assert!(nz <= 4096);

            for y in 0..ny {
                for x in 0..nx {
                    for z in 0..nz {
                        col[z] = image[z * nx * ny + y * nx + x];
                    }
                    iwt_inverse_5(&col[..nz], &mut out[..nz], &mut scratch[..nz]);
                    for z in 0..nz {
                        image[z * nx * ny + y * nx + x] = out[z];
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upshift_downshift_roundtrip() {
        let original = [10, -5, 127, 0, -128];
        let mut data: [i32; 5] = original;
        upshift(&mut data, 3);
        for (o, d) in original.iter().zip(data.iter()) {
            assert_eq!(*d, *o << 3);
        }
        downshift(&mut data, 3);
        assert_eq!(data, original);
    }

    #[test]
    fn downshift_rounds() {
        let mut data = [3i32, 5, 7, -3];
        downshift(&mut data, 1);
        assert_eq!(data, [2, 3, 4, -1]);
    }

    #[test]
    fn iwt_single_level_roundtrip() {
        let input = [10, 20, 30, 40, 50, 60, 70, 80];
        let n = input.len();
        let p = (n + 1) / 2;
        let q = n / 2;
        let mut l = [0i32; 4];
        let mut h = [0i32; 4];
        iwt_forward_single(&input, &mut l[..p], &mut h[..q]);

        let mut recovered = [0i32; 8];
        iwt_inverse_single(&l[..p], &h[..q], &mut recovered[..n]);
        assert_eq!(recovered, input);
    }

    #[test]
    fn iwt_single_level_odd_length() {
        let input = [10, 20, 30, 40, 50];
        let n = input.len();
        let p = (n + 1) / 2;
        let q = n / 2;
        let mut l = [0i32; 3];
        let mut h = [0i32; 2];
        iwt_forward_single(&input, &mut l[..p], &mut h[..q]);

        let mut recovered = [0i32; 5];
        iwt_inverse_single(&l[..p], &h[..q], &mut recovered[..n]);
        assert_eq!(recovered, input);
    }

    #[test]
    fn iwt_five_level_roundtrip() {
        let input: [i32; 32] = core::array::from_fn(|i| (i as i32) * 7 - 100);
        let mut output = [0i32; 32];
        let mut scratch = [0i32; 32];
        iwt_forward_5(&input, &mut output, &mut scratch);

        let mut recovered = [0i32; 32];
        iwt_inverse_5(&output, &mut recovered, &mut scratch);
        assert_eq!(recovered, input);
    }

    #[test]
    fn iwt_five_level_short_sequence() {
        let input = [42i32, -7, 100];
        let mut output = [0i32; 3];
        let mut scratch = [0i32; 3];
        iwt_forward_5(&input, &mut output, &mut scratch);

        let mut recovered = [0i32; 3];
        iwt_inverse_5(&output, &mut recovered, &mut scratch);
        assert_eq!(recovered, input);
    }

    #[test]
    fn spectral_transform_identity() {
        let mut image = [1, 2, 3, 4, 5, 6, 7, 8];
        let original = image;
        transform_spectral(&mut image, 2, 2, 2, SpectralTransform::Identity);
        assert_eq!(image, original);
    }

    #[test]
    fn spectral_transform_iwt_roundtrip() {
        let nx = 2;
        let ny = 2;
        let nz = 8;
        let mut image: [i32; 32] = core::array::from_fn(|i| (i as i32) * 3 + 1);
        let original = image;

        transform_spectral(&mut image, nx, ny, nz, SpectralTransform::Iwt);
        assert_ne!(image, original);

        inverse_transform_spectral(&mut image, nx, ny, nz, SpectralTransform::Iwt);
        assert_eq!(image, original);
    }

    #[test]
    fn iwt_decorrelates_constant_bands() {
        let input = [42i32; 16];
        let mut output = [0i32; 16];
        let mut scratch = [0i32; 16];
        iwt_forward_5(&input, &mut output, &mut scratch);
        assert_eq!(output[0], 42);
        for &h in &output[1..] {
            assert_eq!(h, 0);
        }
    }
}
