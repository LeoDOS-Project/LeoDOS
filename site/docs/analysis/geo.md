# Geospatial Utilities

Coordinate transforms and sensor geometry calculations for
geolocating image pixels. Uses WGS-84 mean Earth radius
(6,371,000 m) and `libm` for trigonometry (`no_std`-compatible).

## Haversine Distance

Great-circle distance between two `LatLon` points in meters:

$$d = 2R \arcsin\sqrt{\sin^2\!\frac{\Delta\phi}{2} + \cos\phi_1 \cos\phi_2 \sin^2\!\frac{\Delta\lambda}{2}}$$

```rust
let sthlm = LatLon::new(59.33, 18.07);
let gbg = LatLon::new(57.71, 11.97);
let d = haversine_distance(sthlm, gbg); // ~398 km
```

## Ground Sample Distance

GSD (meters per pixel) from orbit and sensor parameters:

$$\text{GSD} = \frac{H \cdot p}{f}$$

where $H$ is altitude (m), $p$ is pixel pitch (m), and $f$ is
focal length (m). The function takes pitch in micrometers and
focal length in millimeters.

| Parameter | Unit |
|-----------|------|
| `altitude_m` | meters |
| `focal_length_mm` | millimeters |
| `pixel_pitch_um` | micrometers |

Example: 500 km altitude, 50 mm focal length, 5.5 um pixel
pitch gives GSD = 55 m/px.

## Swath Width

Cross-track footprint from altitude and field of view:

$$W = 2H\tan\!\frac{\theta}{2}$$

| Parameter | Unit |
|-----------|------|
| `altitude_m` | meters |
| `fov_deg` | degrees |

## Pixel to LatLon

`pixel_to_latlon` converts image pixel coordinates to geographic
coordinates using a nadir approximation. The image center is
assumed to be at the sub-satellite point (`nadir`), and pixels
are uniformly spaced at `gsd` meters.

| Parameter | Description |
|-----------|-------------|
| `px`, `py` | Pixel coordinates |
| `center_px`, `center_py` | Image center in pixel coordinates |
| `nadir` | Sub-satellite point (LatLon) |
| `gsd` | Ground sample distance (m/px) |

The approximation is valid for narrow fields of view where
Earth curvature within the image footprint is negligible.

## Coordinate Types

`LatLon` holds latitude (-90 to 90) and longitude (-180 to 180)
in degrees. `deg2rad` and `rad2deg` convert between units.
