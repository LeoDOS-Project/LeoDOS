//! Image statistics and histogram analysis.

/// Basic statistics for a data slice.
#[derive(Debug, Copy, Clone)]
pub struct Stats {
    /// Number of samples.
    pub count: usize,
    /// Minimum value.
    pub min: f32,
    /// Maximum value.
    pub max: f32,
    /// Arithmetic mean.
    pub mean: f32,
    /// Variance (population).
    pub variance: f32,
}

/// Compute basic statistics over a slice.
pub fn compute(data: &[f32]) -> Stats {
    if data.is_empty() {
        return Stats {
            count: 0,
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            variance: 0.0,
        };
    }

    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut sum = 0.0f64;

    for &v in data {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
        sum += v as f64;
    }

    let n = data.len() as f64;
    let mean = sum / n;

    let mut var_sum = 0.0f64;
    for &v in data {
        let d = v as f64 - mean;
        var_sum += d * d;
    }

    Stats {
        count: data.len(),
        min,
        max,
        mean: mean as f32,
        variance: (var_sum / n) as f32,
    }
}

/// Compute a histogram with `n_bins` uniform bins over [min, max].
pub fn histogram(data: &[f32], min: f32, max: f32, bins: &mut [u32]) {
    let n_bins = bins.len();
    for b in bins.iter_mut() {
        *b = 0;
    }
    if n_bins == 0 || max <= min {
        return;
    }
    let range = max - min;
    for &v in data {
        let idx = ((v - min) / range * n_bins as f32) as usize;
        let idx = idx.min(n_bins - 1);
        bins[idx] += 1;
    }
}

/// Compute the percentile value from a pre-computed histogram.
///
/// `percentile` is in the range [0.0, 1.0].
pub fn percentile_from_histogram(
    bins: &[u32],
    min: f32,
    max: f32,
    percentile: f32,
) -> f32 {
    let total: u32 = bins.iter().sum();
    if total == 0 {
        return min;
    }
    let target = (percentile * total as f32) as u32;
    let mut cumulative = 0u32;
    let n_bins = bins.len();
    let bin_width = (max - min) / n_bins as f32;

    for (i, &count) in bins.iter().enumerate() {
        cumulative += count;
        if cumulative >= target {
            return min + (i as f32 + 0.5) * bin_width;
        }
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_stats() {
        let data = [1.0, 2.0, 3.0, 4.0, 5.0];
        let s = compute(&data);
        assert_eq!(s.count, 5);
        assert_eq!(s.min, 1.0);
        assert_eq!(s.max, 5.0);
        assert!((s.mean - 3.0).abs() < 1e-5);
        assert!((s.variance - 2.0).abs() < 1e-5);
    }

    #[test]
    fn empty_stats() {
        let s = compute(&[]);
        assert_eq!(s.count, 0);
    }

    #[test]
    fn histogram_uniform() {
        let data = [0.0, 0.25, 0.5, 0.75, 1.0];
        let mut bins = [0u32; 4];
        histogram(&data, 0.0, 1.0, &mut bins);
        assert_eq!(bins[0], 1);
        assert_eq!(bins[1], 1);
        assert_eq!(bins[2], 1);
        assert_eq!(bins[3], 2);
    }

    #[test]
    fn percentile_median() {
        let mut bins = [0u32; 10];
        let data: [f32; 100] = core::array::from_fn(|i| i as f32);
        histogram(&data, 0.0, 100.0, &mut bins);
        let median = percentile_from_histogram(&bins, 0.0, 100.0, 0.5);
        assert!((median - 50.0).abs() < 10.0);
    }
}
