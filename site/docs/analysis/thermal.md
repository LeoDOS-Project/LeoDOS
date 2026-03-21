# Fire Detection

Active fire and hotspot detection based on the MODIS Collection 6.1
algorithm (Giglio et al., 2003; Giglio et al., 2016). This is a
simplified implementation using 4 of the ~12 tests in the full
algorithm.

## Input

Two co-registered brightness temperature bands stored as flat
`&[f32]` arrays (row-major):

| Band | Wavelength | MODIS Equivalent | Role |
|------|-----------|------------------|------|
| T4 (MIR) | ~3.9 um | Band 21/22 | Fire-sensitive; strong emission from active fires |
| T11 (TIR) | ~11 um | Band 31 | Background temperature reference |

## Detection Pipeline

Each pixel passes through four sequential tests. A pixel must
pass all tests to be flagged as a hotspot.

### Test 1: Absolute MIR Threshold

Reject if T4 < `t4_abs`. Eliminates all cool pixels immediately.

### Test 2: Absolute Split-Window Threshold

Reject if (T4 - T11) < `t4_t11_abs`. Fire pixels have a large
MIR-TIR difference because fire radiates strongly at 3.9 um but
not proportionally at 11 um.

### Test 3: Contextual MIR Anomaly

Compute background statistics from a window of radius `bg_radius`
around the pixel (excluding the pixel itself and any pixel with
T4 > `bg_max_t4`). Reject if the MIR anomaly above the background
mean is not greater than max(`dt4_threshold`, 3 * std_T4).

### Test 4: Contextual Split-Window Anomaly

Using the same background window, reject if the split-window
anomaly (T4 - T11) above the background mean is not greater than
max(`dt4_t11_threshold`, 3 * std_dT).

If fewer than `min_bg_count` valid background pixels are available,
tests 3 and 4 are skipped and only the absolute thresholds apply,
with confidence reduced to 0.3.

## Thresholds

Default values follow MODIS Collection 6.1 (Giglio et al., 2016,
Table 2).

| Parameter | Day | Night | Unit |
|-----------|-----|-------|------|
| `t4_abs` | 310 | 305 | K |
| `dt4_threshold` | 10 | 10 | K |
| `dt4_t11_threshold` | 6 | 6 | K |
| `t4_t11_abs` | 10 | 10 | K |
| `min_bg_count` | 8 | 8 | pixels |
| `bg_radius` | 5 | 5 | pixels |
| `bg_max_t4` | 325 | 320 | K |

## Confidence Scoring

Confidence is the average of two terms, each clamped to [0, 1]:

$$\text{confidence} = \frac{\min(\Delta T4 / 50, 1) + \min(\Delta (T4{-}T11) / 30, 1)}{2}$$

Higher anomalies above the background produce higher confidence.
When the contextual test is skipped (insufficient background
pixels), confidence is fixed at 0.3.

## FRP Estimation

Fire Radiative Power is estimated from brightness temperature
using a simplified Wooster et al. (2003) approach:

$$\text{FRP} = \max\big((\Delta T4)^2 \times 10^{-6},\; 0\big) \quad \text{(MW)}$$

where $\Delta T4 = T4_{\text{fire}} - T4_{\text{background}}$.
This is a proxy; accurate FRP requires the actual MIR spectral
radiance (W/m^2/sr/um).

## Output

`detect_fire` returns a count and fills a caller-provided
`&mut [Hotspot]` buffer. Each `Hotspot` contains pixel coordinates,
T4, T11, anomaly values, FRP, and confidence.
