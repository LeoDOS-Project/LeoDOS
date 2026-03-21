//! Spectral indices for earth observation.
//!
//! Each index is a per-pixel computation from two or more
//! spectral bands. Values are typically in the range [-1, 1]
//! represented as f32.
//!
//! References:
//! - NDVI: Rouse et al. (1974), Monitoring vegetation systems
//!   in the Great Plains with ERTS.
//! - EVI: Huete et al. (2002), Overview of the radiometric and
//!   biophysical performance of the MODIS vegetation indices.
//! - SAVI: Huete (1988), A soil-adjusted vegetation index.
//! - NDWI: McFeeters (1996), The use of the Normalized
//!   Difference Water Index.
//! - NBR: Key & Benson (2006), Landscape Assessment.

/// Normalized difference: (a - b) / (a + b).
///
/// Returns 0.0 if both bands are zero. This is the building
/// block for NDVI, NDWI, NBR, NDSI, and other indices.
pub fn normalized_difference(a: f32, b: f32) -> f32 {
    let sum = a + b;
    if sum == 0.0 {
        return 0.0;
    }
    (a - b) / sum
}

/// NDVI — Normalized Difference Vegetation Index.
///
/// Measures vegetation health. Values near +1 indicate dense
/// green vegetation; values near 0 indicate bare soil; negative
/// values indicate water or cloud.
///
/// `nir` — near-infrared reflectance.
/// `red` — red-band reflectance.
pub fn ndvi(nir: f32, red: f32) -> f32 {
    normalized_difference(nir, red)
}

/// NDWI — Normalized Difference Water Index.
///
/// Detects open water surfaces. Positive values indicate water.
///
/// `green` — green-band reflectance.
/// `nir` — near-infrared reflectance.
pub fn ndwi(green: f32, nir: f32) -> f32 {
    normalized_difference(green, nir)
}

/// NBR — Normalized Burn Ratio.
///
/// Measures burn severity. Low (negative) values indicate
/// recently burned areas.
///
/// `nir` — near-infrared reflectance.
/// `swir` — shortwave infrared reflectance.
pub fn nbr(nir: f32, swir: f32) -> f32 {
    normalized_difference(nir, swir)
}

/// dNBR — Differenced Normalized Burn Ratio.
///
/// Change in NBR between pre-fire and post-fire images.
/// Positive values indicate burn severity.
pub fn dnbr(pre_nbr: f32, post_nbr: f32) -> f32 {
    pre_nbr - post_nbr
}

/// NDSI — Normalized Difference Snow Index.
///
/// Detects snow and ice. Positive values indicate snow cover.
///
/// `green` — green-band reflectance.
/// `swir` — shortwave infrared reflectance.
pub fn ndsi(green: f32, swir: f32) -> f32 {
    normalized_difference(green, swir)
}

/// EVI — Enhanced Vegetation Index.
///
/// More sensitive than NDVI in high-biomass regions and less
/// affected by atmospheric conditions.
///
/// `nir` — near-infrared reflectance.
/// `red` — red-band reflectance.
/// `blue` — blue-band reflectance.
pub fn evi(nir: f32, red: f32, blue: f32) -> f32 {
    let denom = nir + 6.0 * red - 7.5 * blue + 1.0;
    if denom == 0.0 {
        return 0.0;
    }
    2.5 * (nir - red) / denom
}

/// SAVI — Soil-Adjusted Vegetation Index.
///
/// Corrects NDVI for soil brightness in areas with low
/// vegetation cover.
///
/// `nir` — near-infrared reflectance.
/// `red` — red-band reflectance.
/// `soil_factor` — soil brightness correction (typically 0.5).
pub fn savi(nir: f32, red: f32, soil_factor: f32) -> f32 {
    let denom = nir + red + soil_factor;
    if denom == 0.0 {
        return 0.0;
    }
    (1.0 + soil_factor) * (nir - red) / denom
}

/// Compute a spectral index for an entire image.
///
/// `band_a` and `band_b` are parallel slices of pixel values.
/// The index function `f` is applied per-pixel.
pub fn compute_index(
    band_a: &[f32],
    band_b: &[f32],
    output: &mut [f32],
    f: fn(f32, f32) -> f32,
) {
    let n = band_a.len().min(band_b.len()).min(output.len());
    for i in 0..n {
        output[i] = f(band_a[i], band_b[i]);
    }
}

/// Threshold an index image into a binary mask.
///
/// Pixels with `index >= threshold` are set to `true`.
pub fn threshold(index: &[f32], threshold: f32, mask: &mut [bool]) {
    let n = index.len().min(mask.len());
    for i in 0..n {
        mask[i] = index[i] >= threshold;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndvi_vegetation() {
        let v = ndvi(0.8, 0.1);
        assert!(v > 0.7);
    }

    #[test]
    fn ndvi_water() {
        let v = ndvi(0.05, 0.3);
        assert!(v < 0.0);
    }

    #[test]
    fn ndvi_zero_bands() {
        assert_eq!(ndvi(0.0, 0.0), 0.0);
    }

    #[test]
    fn ndwi_water_positive() {
        let v = ndwi(0.3, 0.05);
        assert!(v > 0.0);
    }

    #[test]
    fn nbr_burned() {
        let pre = nbr(0.7, 0.2);
        let post = nbr(0.2, 0.5);
        let d = dnbr(pre, post);
        assert!(d > 0.0);
    }

    #[test]
    fn compute_index_batch() {
        let nir = [0.8, 0.1, 0.5];
        let red = [0.1, 0.3, 0.5];
        let mut out = [0.0f32; 3];
        compute_index(&nir, &red, &mut out, ndvi);
        assert!(out[0] > 0.5);
        assert!(out[1] < 0.0);
        assert_eq!(out[2], 0.0);
    }

    #[test]
    fn threshold_mask() {
        let idx = [0.1, 0.5, 0.8, -0.2];
        let mut mask = [false; 4];
        threshold(&idx, 0.3, &mut mask);
        assert_eq!(mask, [false, true, true, false]);
    }
}
