//! Radiometric calibration: convert raw sensor values to
//! physical units (reflectance, brightness temperature).
//!
//! Every imaging sensor outputs raw digital numbers (DN). Before
//! spectral indices or detection algorithms can be applied, DNs
//! must be converted to calibrated values:
//!
//! - **Reflectance** (TOA): for visible/NIR/SWIR bands
//! - **Brightness temperature** (K): for thermal bands
//!
//! Calibration coefficients are sensor-specific and typically
//! provided in the image metadata.

/// Calibration coefficients for a single spectral band.
#[derive(Debug, Copy, Clone)]
pub struct BandCalibration {
    /// Multiplicative gain (reflectance per DN, or radiance per DN).
    pub gain: f32,
    /// Additive offset.
    pub offset: f32,
}

impl BandCalibration {
    /// Creates calibration coefficients.
    pub fn new(gain: f32, offset: f32) -> Self {
        Self { gain, offset }
    }

    /// Convert a raw DN to calibrated value.
    ///
    /// `calibrated = gain * dn + offset`
    pub fn apply(&self, dn: f32) -> f32 {
        self.gain * dn + self.offset
    }
}

/// Thermal calibration coefficients for converting radiance
/// to brightness temperature using the inverse Planck function.
#[derive(Debug, Copy, Clone)]
pub struct ThermalCalibration {
    /// Radiance gain.
    pub radiance_gain: f32,
    /// Radiance offset.
    pub radiance_offset: f32,
    /// Planck constant K1 (W·m⁻²·sr⁻¹·μm⁻¹), sensor-specific.
    pub k1: f32,
    /// Planck constant K2 (Kelvin), sensor-specific.
    pub k2: f32,
}

impl ThermalCalibration {
    /// Convert raw DN to brightness temperature (K).
    ///
    /// 1. DN → radiance: `L = gain * DN + offset`
    /// 2. Radiance → BT: `BT = K2 / ln(K1/L + 1)`
    pub fn dn_to_bt(&self, dn: f32) -> f32 {
        let radiance = self.radiance_gain * dn + self.radiance_offset;
        if radiance <= 0.0 {
            return 0.0;
        }
        self.k2 / libm::logf(self.k1 / radiance + 1.0)
    }
}

/// Well-known sensor calibration presets.
pub mod sensors {
    use super::ThermalCalibration;

    /// Landsat 8 TIRS Band 10 thermal calibration constants.
    pub fn landsat8_band10() -> ThermalCalibration {
        ThermalCalibration {
            radiance_gain: 3.342e-4,
            radiance_offset: 0.1,
            k1: 774.89,
            k2: 1321.08,
        }
    }

    /// Landsat 8 TIRS Band 11 thermal calibration constants.
    pub fn landsat8_band11() -> ThermalCalibration {
        ThermalCalibration {
            radiance_gain: 3.342e-4,
            radiance_offset: 0.1,
            k1: 480.89,
            k2: 1201.14,
        }
    }
}

/// Calibrate an entire band from raw DN to physical values.
pub fn calibrate_reflectance(
    raw: &[f32],
    cal: &BandCalibration,
    output: &mut [f32],
) {
    let n = raw.len().min(output.len());
    for i in 0..n {
        output[i] = cal.apply(raw[i]);
    }
}

/// Calibrate an entire thermal band from DN to brightness temperature.
pub fn calibrate_thermal(
    raw: &[f32],
    cal: &ThermalCalibration,
    output: &mut [f32],
) {
    let n = raw.len().min(output.len());
    for i in 0..n {
        output[i] = cal.dn_to_bt(raw[i]);
    }
}

/// Solar zenith angle correction for TOA reflectance.
///
/// `reflectance_corrected = reflectance / cos(solar_zenith)`
///
/// This normalizes for illumination geometry. `solar_zenith_deg`
/// is in degrees. Returns the input unchanged if zenith >= 85°
/// (grazing illumination, correction is unreliable).
pub fn sun_angle_correction(reflectance: &mut [f32], solar_zenith_deg: f32) {
    if solar_zenith_deg >= 85.0 {
        return;
    }
    let cos_sz = libm::cosf(crate::geo::deg2rad(solar_zenith_deg));
    if cos_sz <= 0.0 {
        return;
    }
    for r in reflectance.iter_mut() {
        *r /= cos_sz;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_calibration() {
        let cal = BandCalibration::new(2e-5, -0.1);
        let v = cal.apply(10000.0);
        assert!((v - 0.1).abs() < 1e-5);
    }

    #[test]
    fn thermal_bt_reasonable() {
        let cal = sensors::landsat8_band10();
        let bt = cal.dn_to_bt(25000.0);
        assert!(bt > 250.0 && bt < 350.0);
    }

    #[test]
    fn thermal_zero_dn() {
        let cal = sensors::landsat8_band10();
        let bt = cal.dn_to_bt(0.0);
        assert!(bt >= 0.0);
    }

    #[test]
    fn sun_angle_increases_reflectance() {
        let mut data = [0.3f32; 4];
        let original = data;
        sun_angle_correction(&mut data, 45.0);
        for (c, o) in data.iter().zip(original.iter()) {
            assert!(*c > *o);
        }
    }

    #[test]
    fn sun_angle_near_horizon_noop() {
        let mut data = [0.3f32; 4];
        let original = data;
        sun_angle_correction(&mut data, 89.0);
        assert_eq!(data, original);
    }

    #[test]
    fn batch_calibrate() {
        let raw = [10000.0, 20000.0, 30000.0];
        let cal = BandCalibration::new(2e-5, -0.1);
        let mut out = [0.0f32; 3];
        calibrate_reflectance(&raw, &cal, &mut out);
        assert!((out[0] - 0.1).abs() < 1e-5);
        assert!((out[1] - 0.3).abs() < 1e-5);
        assert!((out[2] - 0.5).abs() < 1e-5);
    }
}
