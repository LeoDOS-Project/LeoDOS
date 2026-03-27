//! Geo-located thermal camera.
//!
//! Combines a [`ThermalCamera`] with a [`Gps`] receiver to
//! produce geo-located frames with nadir and GSD metadata.

use crate::error::CfsError;
use crate::nos3::drivers::novatel::Gps;
use crate::nos3::drivers::thermal_cam::ThermalCamera;
use leodos_analysis::frame::GeoFrame;
use leodos_analysis::geo::ground_sample_distance;

/// A thermal camera paired with GPS for geo-located captures.
pub struct GeoCamera {
    camera: ThermalCamera,
    gps: Gps,
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
        gps_device: &core::ffi::CStr,
        gps_baud: u32,
        altitude_m: f32,
        focal_length_mm: f32,
        pixel_pitch_um: f32,
    ) -> Result<Self, CfsError> {
        let camera = ThermalCamera::builder()
            .device(device)
            .chip_select_line(chip_select_line)
            .baudrate(baudrate)
            .build()?;
        let gps = Gps::builder().device(gps_device).baud(gps_baud).build()?;
        Ok(Self {
            camera,
            gps,
            altitude_m,
            focal_length_mm,
            pixel_pitch_um,
        })
    }
}

impl GeoCamera {
    /// Captures a GPS fix and thermal frame, returning a geo-located frame.
    ///
    /// ## Aruments
    ///
    /// - `mwir`: Mid-Wave Infrared buffer.
    /// - `lwir`: Long-Wave Infrared buffer.
    pub async fn capture<'a>(
        &mut self,
        mwir: &'a mut [f32],
        lwir: &'a mut [f32],
    ) -> Result<GeoFrame<'a>, CfsError> {
        let fix = self.gps.request_data().await?;
        let frame = self.camera.capture(mwir, lwir).await?;
        let gps_epoch_s = fix.weeks as f64 * 604_800.0
            + fix.seconds_into_week as f64
            + fix.fractions;
        Ok(GeoFrame {
            frame,
            nadir_lat: fix.lat,
            nadir_lon: fix.lon,
            gsd: ground_sample_distance(self.altitude_m, self.focal_length_mm, self.pixel_pitch_um),
            timestamp_s: gps_epoch_s,
        })
    }
}
