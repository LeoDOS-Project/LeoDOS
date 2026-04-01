//! Geo-located thermal camera.
//!
//! Combines a [`ThermalCamera`] with a [`Gps`] receiver to
//! produce geo-located frames with nadir and GSD metadata.

use crate::error::CfsError;
use crate::nos3::drivers::thermal_cam::ThermalCamera;
use leodos_analysis::frame::GeoFrame;
use leodos_analysis::geo::ground_sample_distance;

/// A thermal camera with geo-location metadata.
///
/// Captures thermal frames and wraps them with nadir position
/// and GSD. GPS can be provided externally via `set_nadir`.
pub struct GeoCamera {
    camera: ThermalCamera,
    nadir_lat: f32,
    nadir_lon: f32,
    timestamp_s: f64,
    altitude_m: f32,
    focal_length_mm: f32,
    pixel_pitch_um: f32,
}

#[bon::bon]
impl GeoCamera {
    #[builder]
    /// Creates a new geo-located thermal camera.
    pub fn new(
        device: &core::ffi::CStr,
        chip_select_line: u8,
        baudrate: u32,
        altitude_m: f32,
        focal_length_mm: f32,
        pixel_pitch_um: f32,
    ) -> Result<Self, CfsError> {
        let camera = ThermalCamera::builder()
            .device(device)
            .chip_select_line(chip_select_line)
            .baudrate(baudrate)
            .build()?;
        Ok(Self {
            camera,
            nadir_lat: 0.0,
            nadir_lon: 0.0,
            timestamp_s: 0.0,
            altitude_m,
            focal_length_mm,
            pixel_pitch_um,
        })
    }
}

impl GeoCamera {
    /// Sets the nadir position (from an external GPS source).
    pub fn set_nadir(&mut self, lat: f32, lon: f32, timestamp_s: f64) {
        self.nadir_lat = lat;
        self.nadir_lon = lon;
        self.timestamp_s = timestamp_s;
    }

    /// Captures a thermal frame with current geo-location.
    pub async fn capture<'a>(
        &mut self,
        mwir: &'a mut [f32],
        lwir: &'a mut [f32],
    ) -> Result<GeoFrame<'a>, CfsError> {
        let frame = self.camera.capture(mwir, lwir).await?;
        Ok(GeoFrame {
            frame,
            nadir_lat: self.nadir_lat,
            nadir_lon: self.nadir_lon,
            gsd: ground_sample_distance(self.altitude_m, self.focal_length_mm, self.pixel_pitch_um),
            timestamp_s: self.timestamp_s,
        })
    }
}
