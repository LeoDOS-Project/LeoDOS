# Spectral Indices

Per-pixel computations from two or more spectral bands. All
normalized-difference indices output values in [-1, 1] as `f32`.

## Normalized Difference

The building block for NDVI, NDWI, NBR, and NDSI:

$$\text{ND}(a, b) = \frac{a - b}{a + b}$$

Returns 0.0 when both bands are zero.

## Index Reference

| Index | Formula | Bands | Measures | Reference |
|-------|---------|-------|----------|-----------|
| NDVI | ND(NIR, Red) | NIR (~0.86 um), Red (~0.66 um) | Vegetation health | Rouse et al. (1974) |
| NDWI | ND(Green, NIR) | Green (~0.56 um), NIR (~0.86 um) | Open water | McFeeters (1996) |
| NBR | ND(NIR, SWIR) | NIR (~0.86 um), SWIR (~2.2 um) | Burn severity | Key & Benson (2006) |
| dNBR | NBR_pre - NBR_post | (derived) | Burn change | Key & Benson (2006) |
| NDSI | ND(Green, SWIR) | Green (~0.56 um), SWIR (~1.6 um) | Snow/ice cover | Hall et al. (1995) |
| EVI | 2.5(NIR - Red) / (NIR + 6Red - 7.5Blue + 1) | NIR, Red, Blue | Vegetation (high-biomass) | Huete et al. (2002) |
| SAVI | (1 + L)(NIR - Red) / (NIR + Red + L) | NIR, Red | Vegetation (sparse, soil-corrected) | Huete (1988) |

## Typical Thresholds

| Index | Value | Interpretation |
|-------|-------|----------------|
| NDVI > 0.6 | Dense green vegetation |
| NDVI 0.2 -- 0.6 | Moderate vegetation |
| NDVI < 0.1 | Bare soil, water, or cloud |
| NDWI > 0.0 | Water |
| NBR < -0.25 | High burn severity |
| dNBR > 0.66 | High burn severity |
| dNBR 0.27 -- 0.66 | Moderate burn severity |
| NDSI > 0.4 | Snow |
| EVI > 0.2 | Vegetation present |
| SAVI > 0.5 | Dense vegetation (L = 0.5) |

## Batch Processing

`compute_index` applies any two-band index function across parallel
slices of pixel values. `threshold` converts an index image into a
binary mask at a given cutoff.

```rust
let mut ndvi_img = [0.0f32; 1024];
compute_index(&nir_band, &red_band, &mut ndvi_img, ndvi);

let mut veg_mask = [false; 1024];
threshold(&ndvi_img, 0.3, &mut veg_mask);
```
