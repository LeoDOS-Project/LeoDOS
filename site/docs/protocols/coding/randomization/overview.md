# Overview

The receiver's clock recovery circuit tracks bit transitions in the
incoming signal to stay synchronized with the sender. Randomization
XORs the data with a pseudo-random sequence so that even uniform
payloads (e.g. a block of zeros) produce enough transitions to keep
the clock locked.

- [Randomization (131.0-B-5)](./pseudo-random) — CCSDS pseudo-random sequence XOR
