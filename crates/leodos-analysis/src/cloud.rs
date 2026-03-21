//! Cloud masking and pixel quality filtering.
//!
//! Based on Fmask (Zhu & Woodcock, 2012) and the MODIS cloud
//! mask algorithm (Ackerman et al., 1998). Applies a series of
//! spectral and thermal tests to classify each pixel.

/// Cloud mask result for a single pixel.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CloudClass {
    /// Clear sky — pixel is usable for analysis.
    Clear,
    /// Cloud detected with high confidence.
    Cloud,
    /// Thin cloud, cloud shadow, or haze.
    Uncertain,
    /// Snow or ice (may be confused with cloud).
    Snow,
}

/// Per-pixel spectral input for cloud detection.
///
/// All reflectance values are top-of-atmosphere (TOA),
/// range [0, 1]. Brightness temperatures are in Kelvin.
#[derive(Debug, Copy, Clone, Default)]
pub struct PixelBands {
    /// Blue band reflectance (~0.48 μm).
    pub blue: f32,
    /// Green band reflectance (~0.56 μm).
    pub green: f32,
    /// Red band reflectance (~0.66 μm).
    pub red: f32,
    /// NIR band reflectance (~0.86 μm).
    pub nir: f32,
    /// SWIR1 band reflectance (~1.6 μm).
    pub swir1: f32,
    /// SWIR2 band reflectance (~2.2 μm), optional.
    pub swir2: f32,
    /// Cirrus band reflectance (~1.38 μm), optional (0 if absent).
    pub cirrus: f32,
    /// Thermal brightness temperature (K), optional (0 if absent).
    pub bt: f32,
}

/// Cloud detection thresholds.
#[derive(Debug, Copy, Clone)]
pub struct CloudThresholds {
    /// Brightness threshold: cloud if any visible band exceeds this.
    pub brightness_high: f32,
    /// Whiteness threshold: cloud if visible bands are spectrally flat.
    pub whiteness_max: f32,
    /// HOT (Haze Optimized Transformation) threshold.
    pub hot_threshold: f32,
    /// NDSI threshold for snow: snow if NDSI > this and NIR > 0.11.
    pub ndsi_snow: f32,
    /// Thermal cold threshold (K): cloud if BT < this.
    pub bt_cold: f32,
    /// Thermal warm threshold (K): uncertain if BT < this.
    pub bt_warm: f32,
    /// Cirrus band threshold: cloud if cirrus > this.
    pub cirrus_threshold: f32,
    /// NDVI threshold: clear vegetation if NDVI > this.
    pub ndvi_veg: f32,
}

impl Default for CloudThresholds {
    fn default() -> Self {
        Self {
            brightness_high: 0.35,
            whiteness_max: 0.7,
            hot_threshold: 0.08,
            ndsi_snow: 0.15,
            bt_cold: 240.0,
            bt_warm: 270.0,
            cirrus_threshold: 0.02,
            ndvi_veg: 0.5,
        }
    }
}

/// Classify a single pixel.
pub fn classify_pixel(pixel: &PixelBands, thresholds: &CloudThresholds) -> CloudClass {
    // Test 1: Cirrus band (strong indicator of thin/high cloud)
    if pixel.cirrus > thresholds.cirrus_threshold {
        return CloudClass::Cloud;
    }

    // Test 2: NDSI snow test — must run before thermal test
    // to avoid misclassifying cold snow as cloud.
    // Snow has high NDSI, high NIR, and low SWIR1.
    // Clouds also have high NDSI but high SWIR1.
    let ndsi = crate::indices::normalized_difference(pixel.green, pixel.swir1);
    if ndsi > thresholds.ndsi_snow && pixel.nir > 0.11 && pixel.swir1 < 0.15 {
        return CloudClass::Snow;
    }

    // Test 3: Brightness temperature (cold cloud tops)
    if pixel.bt > 0.0 {
        if pixel.bt < thresholds.bt_cold {
            return CloudClass::Cloud;
        }
        if pixel.bt < thresholds.bt_warm {
            return CloudClass::Uncertain;
        }
    }

    // Test 4: Brightness test (clouds are bright in visible)
    let mean_vis = (pixel.blue + pixel.green + pixel.red) / 3.0;
    if mean_vis > thresholds.brightness_high {
        // Test 5: Whiteness test (clouds are spectrally flat)
        let whiteness = visible_whiteness(pixel.blue, pixel.green, pixel.red, mean_vis);
        if whiteness < thresholds.whiteness_max {
            return CloudClass::Cloud;
        }
    }

    // Test 6: HOT (Haze Optimized Transformation)
    // HOT = blue - 0.5 * red - 0.08; positive values indicate cloud/haze
    let hot = pixel.blue - 0.5 * pixel.red - thresholds.hot_threshold;
    if hot > 0.0 && mean_vis > 0.15 {
        return CloudClass::Uncertain;
    }

    // Test 7: NDVI vegetation test — high NDVI means clear land
    let ndvi = crate::indices::normalized_difference(pixel.nir, pixel.red);
    if ndvi > thresholds.ndvi_veg {
        return CloudClass::Clear;
    }

    CloudClass::Clear
}

/// Whiteness: how spectrally uniform the visible bands are.
///
/// Returns 0 for perfectly flat spectrum (white cloud),
/// higher values for colored surfaces. Defined as the
/// mean absolute deviation from the visible mean.
fn visible_whiteness(blue: f32, green: f32, red: f32, mean: f32) -> f32 {
    if mean == 0.0 {
        return 1.0;
    }
    let d_b = libm::fabsf(blue - mean) / mean;
    let d_g = libm::fabsf(green - mean) / mean;
    let d_r = libm::fabsf(red - mean) / mean;
    (d_b + d_g + d_r) / 3.0
}

/// Classify an entire image.
pub fn classify_image(
    pixels: &[PixelBands],
    thresholds: &CloudThresholds,
    mask: &mut [CloudClass],
) {
    let n = pixels.len().min(mask.len());
    for i in 0..n {
        mask[i] = classify_pixel(&pixels[i], thresholds);
    }
}

/// Count pixels of each class.
pub fn class_counts(mask: &[CloudClass]) -> ClassCounts {
    let mut counts = ClassCounts {
        clear: 0,
        cloud: 0,
        uncertain: 0,
        snow: 0,
    };
    for &c in mask {
        match c {
            CloudClass::Clear => counts.clear += 1,
            CloudClass::Cloud => counts.cloud += 1,
            CloudClass::Uncertain => counts.uncertain += 1,
            CloudClass::Snow => counts.snow += 1,
        }
    }
    counts
}

/// Counts of each cloud class.
#[derive(Debug, Copy, Clone, Default)]
pub struct ClassCounts {
    /// Number of clear pixels.
    pub clear: usize,
    /// Number of cloud pixels.
    pub cloud: usize,
    /// Number of uncertain pixels.
    pub uncertain: usize,
    /// Number of snow/ice pixels.
    pub snow: usize,
}

impl ClassCounts {
    /// Cloud cover fraction (cloud + uncertain) / total.
    pub fn cloud_fraction(&self) -> f32 {
        let total = self.clear + self.cloud + self.uncertain + self.snow;
        if total == 0 {
            return 0.0;
        }
        (self.cloud + self.uncertain) as f32 / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_land() -> PixelBands {
        PixelBands {
            blue: 0.05,
            green: 0.08,
            red: 0.07,
            nir: 0.40,
            swir1: 0.15,
            bt: 295.0,
            ..Default::default()
        }
    }

    fn thick_cloud() -> PixelBands {
        PixelBands {
            blue: 0.55,
            green: 0.54,
            red: 0.53,
            nir: 0.50,
            swir1: 0.30,
            bt: 225.0,
            ..Default::default()
        }
    }

    fn snow_pixel() -> PixelBands {
        PixelBands {
            blue: 0.80,
            green: 0.85,
            red: 0.82,
            nir: 0.70,
            swir1: 0.10,
            bt: 265.0,
            ..Default::default()
        }
    }

    #[test]
    fn classify_clear_land() {
        let t = CloudThresholds::default();
        assert_eq!(classify_pixel(&clear_land(), &t), CloudClass::Clear);
    }

    #[test]
    fn classify_thick_cloud() {
        let t = CloudThresholds::default();
        assert_eq!(classify_pixel(&thick_cloud(), &t), CloudClass::Cloud);
    }

    #[test]
    fn classify_snow() {
        let t = CloudThresholds::default();
        assert_eq!(classify_pixel(&snow_pixel(), &t), CloudClass::Snow);
    }

    #[test]
    fn classify_cirrus() {
        let mut p = clear_land();
        p.cirrus = 0.05;
        let t = CloudThresholds::default();
        assert_eq!(classify_pixel(&p, &t), CloudClass::Cloud);
    }

    #[test]
    fn cloud_fraction() {
        let counts = ClassCounts {
            clear: 70,
            cloud: 20,
            uncertain: 10,
            snow: 0,
        };
        assert!((counts.cloud_fraction() - 0.3).abs() < 0.01);
    }

    #[test]
    fn batch_classify() {
        let pixels = [clear_land(), thick_cloud(), snow_pixel()];
        let mut mask = [CloudClass::Clear; 3];
        let t = CloudThresholds::default();
        classify_image(&pixels, &t, &mut mask);
        assert_eq!(mask[0], CloudClass::Clear);
        assert_eq!(mask[1], CloudClass::Cloud);
        assert_eq!(mask[2], CloudClass::Snow);
    }
}
