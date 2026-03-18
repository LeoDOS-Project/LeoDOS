# Overview

The communication stack is built from independent layers, each handling
a specific class of failure or transformation. These layers do not
operate in isolation — their behaviors interact in ways that affect
the overall system's reliability, timing, and security properties.

This section covers the cross-cutting concerns that span multiple
layers. Understanding these interactions is essential for configuring
the stack correctly: choosing the wrong combination of FEC strength,
retransmission policy, and timeout values can degrade performance
even when each layer works correctly in isolation.

- [Reliability](reliability) — how FEC, COP-1, and SRSPP recover from different failure classes
- [Security](security) — where SDLS sits in the pipeline and how it interacts with reliability and routing
- [Time Codes](time-codes) — CCSDS time formats used in headers and metadata across the stack
