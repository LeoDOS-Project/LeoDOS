# DWT

Wavelet-based image data compression (CCSDS 122.0-B-2) for onboard image compression. The algorithm decomposes an image into frequency subbands using a discrete wavelet transform, then encodes the coefficients progressively from the most significant bits to the least significant. This progressive encoding means that truncating the bitstream at any point produces a lower-quality but valid image — the more bits transmitted, the higher the quality.

## Integer 5/3 Wavelet Transform

LeoDOS implements the integer (5,3) lifting wavelet, which is lossless — the transform is perfectly invertible with no rounding error. The transform is applied in two dimensions (rows then columns) and repeated three times, producing a 3-level decomposition. Each level splits the image into four subbands:

- **LL** — low-frequency approximation (a smaller version of the original image)
- **HL** — horizontal detail (vertical edges)
- **LH** — vertical detail (horizontal edges)
- **HH** — diagonal detail

After three levels, the image is represented as 10 subbands: LL3, HL3, LH3, HH3, HL2, LH2, HH2, HL1, LH1, HH1. Most of the image energy is concentrated in the LL3 subband; the detail subbands are sparse, which is what makes compression effective.

## Bit-Plane Encoder

The wavelet coefficients are encoded using a bit-plane encoder (BPE) that processes coefficients from the most significant bit to the least. The image is divided into segments (strips of 8 rows), and each segment is encoded independently. For each coefficient, the sign and magnitude bits are written progressively.

This segment-based approach is important for onboard processing: each segment can be compressed independently within a fixed memory budget, fitting within the [bounded memory model](/cfs/mission/memory).

## Configuration

| Parameter | Range | Description |
|---|---|---|
| Bits per sample | 2–16 | Dynamic range of the input image |
| Segment size | 8-row strips | Independent compression units |
| Signed samples | bool | Whether input values are signed |

Image width and height must be multiples of 8.

## Limitations

The LeoDOS implementation is lossless only. The CCSDS 122.0-B-2 standard also defines a lossy mode using the (9,7) CDF wavelet with floating-point coefficients, which is not implemented. For lossy compression at a target bit rate, the lossless bitstream can be truncated — the progressive encoding ensures graceful degradation.
