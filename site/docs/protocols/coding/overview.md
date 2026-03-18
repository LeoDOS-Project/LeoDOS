# Overview

The coding layer protects transfer frames against bit errors
introduced by the RF channel. Without forward error correction,
even a single flipped bit would corrupt the frame and force a
retransmission at a higher layer. The coding layer applies three
operations in sequence: randomization, forward error correction,
and framing.

- [Randomization](randomization/overview) --- pseudo-random sequence XOR for clock recovery
- [Forward Error Correction](fec/overview) --- RS, LDPC, and convolutional codes
- [Framing](framing/overview) --- ASM/CADU and CLTU frame synchronization
- [Data Compression](compression/overview) --- Rice, DWT, and hyperspectral compression
