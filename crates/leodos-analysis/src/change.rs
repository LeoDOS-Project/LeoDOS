//! Change detection between two co-registered images.
//!
//! Compares two images of the same area taken at different times
//! to identify pixels that changed significantly. Used for:
//! - Deforestation (NDVI decrease)
//! - Flood extent (NDWI increase)
//! - Fire spread (NBR decrease / hotspot growth)
//! - Urban expansion
//!
//! Based on standard image differencing with adaptive thresholding
//! (Singh, 1989; Lu et al., 2004).

/// A detected change pixel.
#[derive(Debug, Copy, Clone)]
pub struct ChangePixel {
    /// Pixel x coordinate.
    pub x: u16,
    /// Pixel y coordinate.
    pub y: u16,
    /// Value in the earlier image.
    pub before: f32,
    /// Value in the later image.
    pub after: f32,
    /// Signed difference (after - before).
    pub delta: f32,
}

/// Change detection result.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ChangeClass {
    /// No significant change.
    NoChange,
    /// Positive change (value increased).
    Increase,
    /// Negative change (value decreased).
    Decrease,
}

/// Compute per-pixel difference between two images.
///
/// `delta[i] = after[i] - before[i]`
pub fn difference(before: &[f32], after: &[f32], delta: &mut [f32]) {
    let n = before.len().min(after.len()).min(delta.len());
    for i in 0..n {
        delta[i] = after[i] - before[i];
    }
}

/// Classify change using fixed thresholds.
///
/// Pixels with delta > `pos_threshold` are Increase,
/// delta < `neg_threshold` (negative) are Decrease.
pub fn classify_fixed(
    delta: &[f32],
    pos_threshold: f32,
    neg_threshold: f32,
    classes: &mut [ChangeClass],
) {
    let n = delta.len().min(classes.len());
    for i in 0..n {
        classes[i] = if delta[i] >= pos_threshold {
            ChangeClass::Increase
        } else if delta[i] <= neg_threshold {
            ChangeClass::Decrease
        } else {
            ChangeClass::NoChange
        };
    }
}

/// Classify change using adaptive thresholding (mean ± k·σ).
///
/// Computes mean and standard deviation of the difference image,
/// then flags pixels outside mean ± `k_sigma` standard deviations.
/// Typical k values: 1.5-2.5.
pub fn classify_adaptive(delta: &[f32], k_sigma: f32, classes: &mut [ChangeClass]) {
    let n = delta.len().min(classes.len());
    if n == 0 {
        return;
    }

    let stats = crate::stats::compute(delta);
    let std_dev = libm::sqrtf(stats.variance);
    let pos_threshold = stats.mean + k_sigma * std_dev;
    let neg_threshold = stats.mean - k_sigma * std_dev;

    classify_fixed(delta, pos_threshold, neg_threshold, classes);
}

/// Extract changed pixels with their coordinates.
pub fn extract_changes(
    before: &[f32],
    after: &[f32],
    width: usize,
    classes: &[ChangeClass],
    output: &mut [ChangePixel],
) -> usize {
    let n = before.len().min(after.len()).min(classes.len());
    let mut count = 0;
    for i in 0..n {
        if classes[i] == ChangeClass::NoChange {
            continue;
        }
        if count >= output.len() {
            break;
        }
        output[count] = ChangePixel {
            x: (i % width) as u16,
            y: (i / width) as u16,
            before: before[i],
            after: after[i],
            delta: after[i] - before[i],
        };
        count += 1;
    }
    count
}

/// Ratio-based change detection.
///
/// Computes `after[i] / before[i]` for each pixel. Useful when
/// absolute values vary across the scene but relative change is
/// meaningful. Avoids division by zero.
pub fn ratio(before: &[f32], after: &[f32], output: &mut [f32]) {
    let n = before.len().min(after.len()).min(output.len());
    for i in 0..n {
        output[i] = if before[i].abs() < 1e-10 {
            0.0
        } else {
            after[i] / before[i]
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difference_basic() {
        let before = [0.5, 0.8, 0.3, 0.6];
        let after = [0.5, 0.2, 0.9, 0.6];
        let mut delta = [0.0f32; 4];
        difference(&before, &after, &mut delta);
        assert!((delta[0]).abs() < 1e-5);
        assert!((delta[1] - (-0.6)).abs() < 1e-5);
        assert!((delta[2] - 0.6).abs() < 1e-5);
        assert!((delta[3]).abs() < 1e-5);
    }

    #[test]
    fn classify_fixed_thresholds() {
        let delta = [0.0, -0.5, 0.7, 0.1, -0.1];
        let mut classes = [ChangeClass::NoChange; 5];
        classify_fixed(&delta, 0.3, -0.3, &mut classes);
        assert_eq!(classes[0], ChangeClass::NoChange);
        assert_eq!(classes[1], ChangeClass::Decrease);
        assert_eq!(classes[2], ChangeClass::Increase);
        assert_eq!(classes[3], ChangeClass::NoChange);
        assert_eq!(classes[4], ChangeClass::NoChange);
    }

    #[test]
    fn adaptive_detects_outliers() {
        let mut delta = [0.0f32; 100];
        delta[50] = 5.0;
        delta[75] = -5.0;
        let mut classes = [ChangeClass::NoChange; 100];
        classify_adaptive(&delta, 2.0, &mut classes);
        assert_eq!(classes[50], ChangeClass::Increase);
        assert_eq!(classes[75], ChangeClass::Decrease);
        assert_eq!(classes[0], ChangeClass::NoChange);
    }

    #[test]
    fn extract_changes_coords() {
        let before = [0.5, 0.5, 0.5, 0.5];
        let after = [0.5, 0.5, 0.5, 1.0];
        let classes = [
            ChangeClass::NoChange,
            ChangeClass::NoChange,
            ChangeClass::NoChange,
            ChangeClass::Increase,
        ];
        let mut output = [ChangePixel {
            x: 0, y: 0, before: 0.0, after: 0.0, delta: 0.0,
        }; 4];
        let n = extract_changes(&before, &after, 2, &classes, &mut output);
        assert_eq!(n, 1);
        assert_eq!(output[0].x, 1);
        assert_eq!(output[0].y, 1);
    }

    #[test]
    fn ratio_basic() {
        let before = [1.0, 2.0, 0.0, 0.5];
        let after = [2.0, 1.0, 5.0, 0.5];
        let mut out = [0.0f32; 4];
        ratio(&before, &after, &mut out);
        assert!((out[0] - 2.0).abs() < 1e-5);
        assert!((out[1] - 0.5).abs() < 1e-5);
        assert_eq!(out[2], 0.0);
        assert!((out[3] - 1.0).abs() < 1e-5);
    }
}
