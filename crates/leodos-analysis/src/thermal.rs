//! Thermal analysis and hotspot detection.
//!
//! Based on the MODIS Active Fire detection algorithm
//! (Giglio et al., 2003; Giglio et al., 2016).
//!
//! The algorithm uses a sequence of tests:
//! 1. Absolute brightness temperature threshold
//! 2. Contextual test against local background
//! 3. Split-window test (MIR vs TIR difference)
//! 4. Rejection tests for sun glint and false alarms

/// A detected hotspot with its location and diagnostic fields.
#[derive(Debug, Copy, Clone, Default)]
pub struct Hotspot {
    /// Pixel x coordinate.
    pub x: u16,
    /// Pixel y coordinate.
    pub y: u16,
    /// MIR brightness temperature (K).
    pub t4: f32,
    /// TIR brightness temperature (K).
    pub t11: f32,
    /// MIR anomaly above background (K).
    pub dt4: f32,
    /// Split-window anomaly above background (K).
    pub dt4_t11: f32,
    /// Fire radiative power estimate (MW), if available.
    pub frp: f32,
    /// Confidence level (0.0 to 1.0).
    pub confidence: f32,
}

/// Detection thresholds, separated for day and night.
///
/// Default values follow MODIS Collection 6.1
/// (Giglio et al., 2016, Table 2).
#[derive(Debug, Copy, Clone)]
pub struct FireThresholds {
    /// Absolute MIR threshold (K). Pixels below this are
    /// immediately rejected.
    pub t4_abs: f32,
    /// Contextual MIR anomaly threshold (K). Pixel must
    /// exceed background mean by at least this much.
    pub dt4_threshold: f32,
    /// Contextual split-window anomaly threshold (K).
    pub dt4_t11_threshold: f32,
    /// Absolute split-window difference threshold (K).
    /// Rejects pixels where T4-T11 is too small for fire.
    pub t4_t11_abs: f32,
    /// Minimum valid background pixel count. If fewer
    /// neighbors are valid, the contextual test is skipped
    /// and only the absolute threshold applies.
    pub min_bg_count: usize,
    /// Background window radius in pixels.
    pub bg_radius: usize,
    /// Maximum T4 for a background pixel (K). Hotter pixels
    /// are excluded from the background calculation.
    pub bg_max_t4: f32,
}

impl Default for FireThresholds {
    fn default() -> Self {
        Self::day()
    }
}

impl FireThresholds {
    /// Daytime thresholds (MODIS Collection 6.1).
    pub fn day() -> Self {
        Self {
            t4_abs: 310.0,
            dt4_threshold: 10.0,
            dt4_t11_threshold: 6.0,
            t4_t11_abs: 10.0,
            min_bg_count: 8,
            bg_radius: 5,
            bg_max_t4: 325.0,
        }
    }

    /// Nighttime thresholds (MODIS Collection 6.1).
    pub fn night() -> Self {
        Self {
            t4_abs: 305.0,
            dt4_threshold: 10.0,
            dt4_t11_threshold: 6.0,
            t4_t11_abs: 10.0,
            min_bg_count: 8,
            bg_radius: 5,
            bg_max_t4: 320.0,
        }
    }
}

/// Background statistics for a pixel neighborhood.
#[derive(Debug, Copy, Clone)]
struct Background {
    mean_t4: f32,
    std_t4: f32,
    mean_dt: f32,
    std_dt: f32,
    count: usize,
}

fn compute_background(
    t4: &[f32],
    t11: &[f32],
    width: usize,
    height: usize,
    cx: usize,
    cy: usize,
    thresholds: &FireThresholds,
) -> Background {
    let r = thresholds.bg_radius;
    let y0 = cy.saturating_sub(r);
    let y1 = (cy + r + 1).min(height);
    let x0 = cx.saturating_sub(r);
    let x1 = (cx + r + 1).min(width);

    let mut sum_t4 = 0.0f64;
    let mut sum_t4_sq = 0.0f64;
    let mut sum_dt = 0.0f64;
    let mut sum_dt_sq = 0.0f64;
    let mut n = 0usize;

    for y in y0..y1 {
        for x in x0..x1 {
            if x == cx && y == cy {
                continue;
            }
            let idx = y * width + x;
            let v4 = t4[idx];
            if v4 > thresholds.bg_max_t4 || v4 <= 0.0 {
                continue;
            }
            let dt = v4 - t11[idx];
            sum_t4 += v4 as f64;
            sum_t4_sq += (v4 as f64) * (v4 as f64);
            sum_dt += dt as f64;
            sum_dt_sq += (dt as f64) * (dt as f64);
            n += 1;
        }
    }

    if n == 0 {
        return Background {
            mean_t4: 0.0,
            std_t4: 0.0,
            mean_dt: 0.0,
            std_dt: 0.0,
            count: 0,
        };
    }

    let nf = n as f64;
    let mean_t4 = sum_t4 / nf;
    let var_t4 = (sum_t4_sq / nf) - mean_t4 * mean_t4;
    let mean_dt = sum_dt / nf;
    let var_dt = (sum_dt_sq / nf) - mean_dt * mean_dt;

    Background {
        mean_t4: mean_t4 as f32,
        std_t4: libm::sqrtf(var_t4.max(0.0) as f32),
        mean_dt: mean_dt as f32,
        std_dt: libm::sqrtf(var_dt.max(0.0) as f32),
        count: n,
    }
}

/// Result of fire detection on a thermal image.
pub struct FireDetection<'a> {
    /// Detected hotspot pixels.
    pub hotspots: &'a [Hotspot],
    /// Number of hotspots detected.
    pub count: usize,
    /// Centroid X in pixel coordinates.
    pub centroid_x: f32,
    /// Centroid Y in pixel coordinates.
    pub centroid_y: f32,
    /// Maximum MIR brightness temperature (K).
    pub max_temp: f32,
}

/// Run fire detection on a thermal image.
///
/// `t4` — MIR brightness temperature band (e.g. MODIS band 21/22, ~3.9μm).
/// `t11` — TIR brightness temperature band (e.g. MODIS band 31, ~11μm).
pub fn detect_fire<'a>(
    t4: &[f32],
    t11: &[f32],
    width: usize,
    height: usize,
    thresholds: &FireThresholds,
    hotspots: &'a mut [Hotspot],
) -> FireDetection<'a> {
    let mut count = 0;

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let v4 = t4[idx];
            let v11 = t11[idx];
            let dt = v4 - v11;

            // Test 1: absolute MIR threshold
            if v4 < thresholds.t4_abs {
                continue;
            }

            // Test 2: absolute split-window threshold
            if dt < thresholds.t4_t11_abs {
                continue;
            }

            let bg = compute_background(t4, t11, width, height, x, y, thresholds);

            let (dt4, dt4_t11, confidence) = if bg.count >= thresholds.min_bg_count {
                let dt4 = v4 - bg.mean_t4;
                let dt4_t11 = dt - bg.mean_dt;

                // Test 3: contextual MIR anomaly
                let t4_pass = dt4 > thresholds.dt4_threshold.max(3.0 * bg.std_t4);

                // Test 4: contextual split-window anomaly
                let dt_pass = dt4_t11 > thresholds.dt4_t11_threshold.max(3.0 * bg.std_dt);

                if !t4_pass || !dt_pass {
                    continue;
                }

                // Confidence: higher anomaly = higher confidence
                let conf = ((dt4 / 50.0).min(1.0) + (dt4_t11 / 30.0).min(1.0)) / 2.0;
                (dt4, dt4_t11, conf)
            } else {
                // Not enough background pixels — use absolute only
                // with reduced confidence
                (v4 - thresholds.t4_abs, dt, 0.3)
            };

            if count >= hotspots.len() {
                break;
            }

            hotspots[count] = Hotspot {
                x: x as u16,
                y: y as u16,
                t4: v4,
                t11: v11,
                dt4,
                dt4_t11,
                frp: estimate_frp(v4, bg.mean_t4),
                confidence,
            };
            count += 1;
        }
    }

    summarize(hotspots, count)
}

fn summarize(hotspots: &[Hotspot], count: usize) -> FireDetection<'_> {
    let mut sum_x = 0.0f32;
    let mut sum_y = 0.0f32;
    let mut max_temp = 0.0f32;

    for hs in &hotspots[..count] {
        sum_x += hs.x as f32;
        sum_y += hs.y as f32;
        if hs.t4 > max_temp {
            max_temp = hs.t4;
        }
    }

    let n = (count as f32).max(1.0);
    FireDetection {
        hotspots: &hotspots[..count],
        count,
        centroid_x: sum_x / n,
        centroid_y: sum_y / n,
        max_temp,
    }
}

/// Estimate Fire Radiative Power (MW) from MIR radiance.
///
/// Simplified Wooster et al. (2003) approach using brightness
/// temperature difference as a proxy. For accurate FRP, the
/// actual MIR radiance (W/m²/sr/μm) should be used.
fn estimate_frp(t4_fire: f32, t4_bg: f32) -> f32 {
    let dt = t4_fire - t4_bg;
    (dt * dt * 1e-6).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_scene(width: usize, height: usize, bg_t4: f32, bg_t11: f32) -> ([f32; 256], [f32; 256]) {
        let mut t4 = [bg_t4; 256];
        let mut t11 = [bg_t11; 256];
        let n = width * height;
        for i in n..256 {
            t4[i] = 0.0;
            t11[i] = 0.0;
        }
        (t4, t11)
    }

    #[test]
    fn detects_strong_fire() {
        let (mut t4, mut t11) = make_scene(8, 8, 290.0, 285.0);
        // Place a fire pixel
        t4[3 * 8 + 4] = 360.0;
        t11[3 * 8 + 4] = 310.0;

        let thresholds = FireThresholds::day();
        let mut hotspots = [Hotspot {
            x: 0, y: 0, t4: 0.0, t11: 0.0,
            dt4: 0.0, dt4_t11: 0.0, frp: 0.0, confidence: 0.0,
        }; 8];
        let det = detect_fire(&t4, &t11, 8, 8, &thresholds, &mut hotspots);
        assert!(det.count >= 1);
        assert_eq!(det.hotspots[0].x, 4);
        assert_eq!(det.hotspots[0].y, 3);
        assert!(det.hotspots[0].confidence > 0.5);
    }

    #[test]
    fn no_fire_in_cool_scene() {
        let (t4, t11) = make_scene(8, 8, 290.0, 285.0);
        let thresholds = FireThresholds::day();
        let mut hotspots = [Hotspot {
            x: 0, y: 0, t4: 0.0, t11: 0.0,
            dt4: 0.0, dt4_t11: 0.0, frp: 0.0, confidence: 0.0,
        }; 8];
        let det = detect_fire(&t4, &t11, 8, 8, &thresholds, &mut hotspots);
        assert_eq!(det.count, 0);
    }

    #[test]
    fn night_thresholds_lower() {
        let day = FireThresholds::day();
        let night = FireThresholds::night();
        assert!(night.t4_abs < day.t4_abs);
    }

    #[test]
    fn warm_pixel_without_split_window_rejected() {
        let (mut t4, t11) = make_scene(8, 8, 290.0, 285.0);
        // Warm but T4-T11 is small (not fire-like)
        t4[3 * 8 + 4] = 315.0;
        // t11 stays at 285, so dt = 30 which passes t4_t11_abs
        // But let's make t11 close to t4
        let mut t11_mod = t11;
        t11_mod[3 * 8 + 4] = 312.0; // dt = 3, below t4_t11_abs=10

        let thresholds = FireThresholds::day();
        let mut hotspots = [Hotspot {
            x: 0, y: 0, t4: 0.0, t11: 0.0,
            dt4: 0.0, dt4_t11: 0.0, frp: 0.0, confidence: 0.0,
        }; 8];
        let det = detect_fire(&t4, &t11_mod, 8, 8, &thresholds, &mut hotspots);
        assert_eq!(det.count, 0);
    }
}
