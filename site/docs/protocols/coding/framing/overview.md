# Overview

After FEC encoding, the coded data must be framed so that the
receiver can find the start of each block in the continuous
bitstream. CCSDS uses different framing for the TM (downlink) and
TC (uplink) directions.

- [ASM / CADU](asm-cadu) --- TM direction frame synchronization (131.0-B-5)
- [CLTU](cltu) --- TC direction command link transmission units (231.0-B-4)
