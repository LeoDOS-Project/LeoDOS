//! On-board data analysis algorithms for LEO earth observation.
//!
//! Lightweight, no-std-compatible algorithms for processing
//! multispectral and thermal imagery on resource-constrained
//! satellite platforms.
#![no_std]
#![forbid(unsafe_code)]

/// Radiometric calibration: DN to reflectance and brightness temperature.
pub mod calibration;
/// Change detection between multi-temporal images.
pub mod change;
/// Union-find clustering for spatial data.
pub mod cluster;
/// Cloud masking and pixel quality filtering.
pub mod cloud;
/// Geospatial coordinate transforms and utilities.
pub mod geo;
/// Spectral indices computed from multispectral bands.
pub mod indices;
/// Image statistics and histogram analysis.
pub mod stats;
/// Dual-band thermal image frame.
pub mod frame;
/// Thermal analysis and hotspot detection.
pub mod thermal;
/// Image tiling for distributed processing.
pub mod tile;
