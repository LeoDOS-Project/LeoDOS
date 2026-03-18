# DWT

Wavelet-based image compression (CCSDS 122.0-B-2) for onboard use. DWT compresses images by separating them into coarse structure and fine detail, then encoding the most important information first. This means the compressed bitstream can be truncated at any point to produce a lower-quality but valid image — transmit more bits for higher quality, fewer bits when bandwidth is scarce.

## Why It Works

Images have structure at different scales. A photograph of a forest has large-scale features (the treeline against the sky) and fine-scale features (individual leaves). The large-scale features carry most of the visual information, while the fine details are often small values that compress well. DWT separates these scales, concentrating most of the image's information into a small number of important coefficients.

## How It Works

The compressor processes an image in three steps:

1. **Wavelet decomposition** — the image is split into four parts: a smaller, blurry version of the original (the approximation) and three detail images capturing horizontal edges, vertical edges, and diagonal texture. This splitting is repeated three times on the approximation, producing a hierarchy of 10 subbands. After decomposition, most of the image's energy is concentrated in the tiny top-level approximation — the detail subbands are mostly near-zero values.

2. **Bit-plane encoding** — the coefficients are encoded starting from the most significant bits. The first few bits transmitted reconstruct the overall structure of the image; subsequent bits refine the details. This progressive ordering is what makes truncation work — every prefix of the bitstream is a valid, lower-quality image.

3. **Segment processing** — the image is divided into strips of 8 rows, each compressed independently. This keeps the memory footprint fixed: each strip fits in a bounded working buffer on the flight [processor](/cfs/mission/processor), and a corrupted segment does not destroy the rest of the image.

## Lossless and Lossy

LeoDOS uses the integer (5,3) wavelet, which is perfectly invertible — no information is lost in the transform. The compressed output is lossless by default. For lossy compression at a target bit rate, the bitstream can simply be cut short: the progressive encoding ensures the image degrades gracefully rather than breaking.

## Use in LeoDOS

DWT is used for compressing onboard camera imagery before downlink. A raw image might be megabytes; the wavelet-compressed version is significantly smaller with no visible quality loss in lossless mode, or controlled quality loss if truncated to fit a bandwidth budget.
