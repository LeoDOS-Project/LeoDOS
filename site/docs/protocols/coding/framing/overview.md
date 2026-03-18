# Overview

The radio transmits a continuous stream of symbols with no inherent
boundaries. Framing inserts known marker patterns into the
bitstream so the receiver can locate the start of each coded block
and begin decoding at the correct position. Without framing, the
receiver would have no way to align itself to the incoming data.
CCSDS uses different framing for the TM (downlink) and TC (uplink)
directions.

- [ASM / CADU](asm-cadu) — TM direction frame synchronization (131.0-B-5)
- [CLTU](cltu) — TC direction command link transmission units (231.0-B-4)
