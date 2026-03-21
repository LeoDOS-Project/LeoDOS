# Cloud Masking

Per-pixel cloud classification based on Fmask (Zhu & Woodcock,
2012) and the MODIS cloud mask (Ackerman et al., 1998). This is a
simplified implementation using 7 spectral/thermal tests; it does
not include cloud shadow detection or temporal consistency.

## Input

Each pixel is represented as a `PixelBands` struct with
top-of-atmosphere (TOA) reflectances in [0, 1] and optional
brightness temperature in Kelvin.

| Field | Wavelength | Required |
|-------|-----------|----------|
| `blue` | ~0.48 um | Yes |
| `green` | ~0.56 um | Yes |
| `red` | ~0.66 um | Yes |
| `nir` | ~0.86 um | Yes |
| `swir1` | ~1.6 um | Yes |
| `swir2` | ~2.2 um | Optional (0 if absent) |
| `cirrus` | ~1.38 um | Optional (0 if absent) |
| `bt` | Thermal BT (K) | Optional (0 if absent) |

## Classification Output

Each pixel is classified as one of four `CloudClass` values:

| Class | Meaning |
|-------|---------|
| `Clear` | Usable for analysis |
| `Cloud` | Cloud detected with high confidence |
| `Uncertain` | Thin cloud, haze, or cloud shadow |
| `Snow` | Snow or ice |

## Test Cascade

Tests are applied in order. The first test that triggers
determines the classification; remaining tests are skipped.

### Test 1: Cirrus Band

If `cirrus` > `cirrus_threshold` (default 0.02), classify as
**Cloud**. The 1.38 um band is absorbed by water vapor in the
lower atmosphere, so high reflectance indicates high-altitude
ice clouds.

### Test 2: NDSI Snow

Compute NDSI = ND(green, SWIR1). If NDSI > `ndsi_snow` (default
0.15) and NIR > 0.11 and SWIR1 < 0.15, classify as **Snow**.
The SWIR1 < 0.15 condition discriminates snow (low SWIR) from
cloud (high SWIR).

### Test 3: Brightness Temperature

If `bt` is available (> 0):
- BT < `bt_cold` (default 240 K): **Cloud** (cold cloud tops)
- BT < `bt_warm` (default 270 K): **Uncertain** (possible thin cloud)

### Test 4: Brightness

Compute mean visible reflectance = (blue + green + red) / 3.
If mean_vis > `brightness_high` (default 0.35), proceed to
the whiteness test.

### Test 5: Whiteness

Whiteness is the mean absolute deviation of visible bands from
their mean, normalized by the mean. Clouds are spectrally flat
(white), so whiteness < `whiteness_max` (default 0.7) combined
with high brightness triggers **Cloud**.

### Test 6: HOT (Haze Optimized Transformation)

HOT = blue - 0.5 * red - `hot_threshold` (default 0.08).
Positive HOT with mean_vis > 0.15 indicates haze or thin cloud:
**Uncertain**. Based on Zhang et al. (2002).

### Test 7: NDVI Vegetation

NDVI = ND(NIR, red). If NDVI > `ndvi_veg` (default 0.5), classify
as **Clear** (dense vegetation is not cloud).

Pixels that do not trigger any test default to **Clear**.

## Thresholds

All thresholds are configurable via `CloudThresholds`. Defaults:

| Parameter | Default | Unit |
|-----------|---------|------|
| `brightness_high` | 0.35 | reflectance |
| `whiteness_max` | 0.70 | ratio |
| `hot_threshold` | 0.08 | reflectance |
| `ndsi_snow` | 0.15 | index |
| `bt_cold` | 240 | K |
| `bt_warm` | 270 | K |
| `cirrus_threshold` | 0.02 | reflectance |
| `ndvi_veg` | 0.50 | index |

## Batch Processing

`classify_image` applies the cascade to a slice of `PixelBands`,
filling a parallel `&mut [CloudClass]` mask. `class_counts`
summarizes the mask, and `ClassCounts::cloud_fraction()` returns
the combined cloud + uncertain fraction.
