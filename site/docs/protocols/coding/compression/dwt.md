# DWT

Wavelet-based image compression using the integer 5/3 discrete
wavelet transform (DWT). The DWT decomposes the image into
frequency sub-bands; a bit-plane encoder then transmits the most
significant bits first. Compression can be lossless (all bit
planes transmitted) or lossy (truncated at a target bit rate).
