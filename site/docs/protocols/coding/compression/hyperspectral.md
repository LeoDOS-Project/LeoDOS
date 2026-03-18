# Hyperspectral

Lossless compression for multispectral and hyperspectral image cubes (CCSDS 123.0-B-2). Unlike [DWT](dwt) which compresses a single 2D image, this algorithm operates on 3D data — a stack of spectral bands where each band is a 2D image of the same scene at a different wavelength. It exploits both spatial correlation (neighboring pixels in the same band look similar) and spectral correlation (the same pixel across bands is highly correlated) to achieve compression ratios that single-band algorithms cannot match.

## Prediction

The compressor predicts each sample from its spatial and spectral neighbors, then encodes the prediction error (residual). Small residuals mean high compression.

Two prediction modes are available:

- **Full** — uses both directional weights (3 spatial neighbors: north, west, northwest) and spectral weights (up to 15 previous bands). This is the most effective mode when spatial structure varies across bands.
- **Reduced** — uses spectral weights only (previous bands, no spatial neighbors). Simpler and faster, suitable when spatial correlation is weak or when minimizing computation.

The predictor maintains a weight vector per band that is refined with each sample — the weights adapt to the local statistics of the image as compression proceeds.

## Local Sum Types

The predictor computes a local sum (a reference value) from neighboring samples. Four variants control which neighbors are used:

- **Wide neighbor** — uses horizontal neighbors on both sides
- **Narrow neighbor** — uses only the left neighbor
- **Wide column** — uses spectral neighbors on both sides
- **Narrow column** — uses only the previous band

The choice depends on the image structure and the encoding order.

## Entropy Coding

Prediction residuals are encoded using a sample-adaptive Golomb–Rice coder. Each spectral band maintains its own accumulator and counter that track the statistics of recent residuals. The coder adapts its parameter (the number of bits used for the remainder) on every sample, so it tracks changes in image statistics without needing a separate training pass.

## Configuration

| Parameter | Range | Description |
|---|---|---|
| nx, ny, nz | — | Image dimensions (width, height, spectral bands) |
| Dynamic range | 2–16 bits | Bits per sample |
| Prediction bands (p) | 0–15 | Number of previous bands used for spectral prediction |
| Prediction mode | Full / Reduced | Spatial + spectral or spectral only |
| Local sum type | Wide/Narrow Neighbor/Column | Which neighbors contribute to the reference value |
| Weight resolution (omega) | 4–19 | Precision of predictor weights |
| Weight update interval | 2⁴–2¹¹ | How often weights are rescaled |

## Limitations

The LeoDOS implementation supports lossless compression only (no near-lossless quantization). Encoding order is band-sequential (BSQ) — each band is fully processed before the next. Band-interleaved orders (BIL, BIP) are not supported.
