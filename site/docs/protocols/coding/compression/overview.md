# Overview

CCSDS source coding compresses payload data at the application layer
before packetization. These algorithms reduce the volume of data that
must traverse the communication stack, which is critical when downlink
bandwidth is the bottleneck. Unlike FEC and randomization (which
operate on transfer frames), source compression is applied by the
application to raw sensor data.

- [Rice](rice) — lossless sensor data compression (121.0-B-3)
- [DWT](dwt) — wavelet-based image compression (122.0-B-2)
- [Hyperspectral](hyperspectral) — multispectral/hyperspectral image compression (123.0-B-2)
