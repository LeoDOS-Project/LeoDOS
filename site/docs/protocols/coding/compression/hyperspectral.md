# Hyperspectral

Lossless compression for multispectral and hyperspectral image cubes (CCSDS 123.0-B-2). A hyperspectral image is not a single photograph — it is a stack of images of the same scene, each captured at a different wavelength. A sensor might produce 200 bands ranging from visible light through shortwave infrared. This is a huge amount of data, but it is highly redundant: the same pixel across different bands is strongly correlated, and neighboring pixels within a band look similar.

This compressor exploits both types of redundancy to achieve compression ratios that algorithms designed for single images (like [DWT](dwt) or [Rice](rice)) cannot match on this type of data.

## Why It Works

Consider a pixel in a vegetation scene. Its reflectance in band 100 is almost entirely predictable from its reflectance in bands 99, 98, and 97 — the spectral signature changes smoothly across wavelengths. Similarly, a pixel's value is predictable from its spatial neighbors in the same band — adjacent ground pixels tend to have similar reflectance.

After predicting each pixel from its spectral and spatial neighbors, the prediction errors are small numbers near zero. Small numbers compress efficiently with the same Golomb-Rice coding used in [Rice](rice) compression.

## How It Works

The compressor processes the image cube one sample at a time, band by band:

1. **Predict** — for each pixel, compute a predicted value from its neighbors. The predictor uses a weighted combination of previous bands (spectral neighbors) and optionally surrounding pixels in the current band (spatial neighbors). The weights are not fixed — they adapt continuously as the compressor moves through the image, tracking changes in the local statistics.

2. **Compute residual** — subtract the prediction from the actual value. If the prediction is good, the residual is a small number near zero.

3. **Encode** — write the residual using an adaptive Golomb-Rice code. Each band maintains its own statistics, so the coder automatically adjusts to bands with different noise levels or dynamic ranges.

## Prediction Modes

Two modes control how much context the predictor uses:

- **Full** — uses both spatial neighbors (the pixels above, to the left, and diagonally above-left in the current band) and spectral neighbors (the same pixel in up to 15 previous bands). Most effective when the scene has spatial structure that varies across bands.
- **Reduced** — uses only spectral neighbors, ignoring spatial context. Faster and simpler, suitable for sensors where the spatial resolution is low or the scene is relatively uniform.

## Configuration

| Parameter | Range | Description |
|---|---|---|
| nx, ny, nz | — | Image dimensions (width, height, spectral bands) |
| Dynamic range | 2–16 bits | Bits per sample |
| Prediction bands | 0–15 | Number of previous bands used for spectral prediction |
| Prediction mode | Full / Reduced | Spatial + spectral or spectral only |
| Weight resolution | 4–19 | Precision of predictor weights |
| Weight update interval | 2⁴–2¹¹ | How often weights are rescaled |

## Limitations

Lossless compression only (no near-lossless quantization). Encoding order is band-sequential — each band is fully processed before the next. Band-interleaved orders (BIL, BIP) are not supported.

## Use in LeoDOS

Hyperspectral compression is used when a satellite carries a spectral imager — a sensor that captures tens to hundreds of wavelength bands. Without compression, a single hyperspectral scene can be gigabytes. The compressor reduces this to a fraction of the size with no information lost, making it feasible to store onboard and downlink during ground passes. The algorithm runs within the [bounded memory model](/cfs/mission/memory) — each band is processed sequentially with fixed working buffers.
