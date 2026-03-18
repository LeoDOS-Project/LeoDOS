# Overview

Transfer frames are the fixed-format containers that carry Space
Packets across a single point-to-point link. They add framing,
addressing, and sequencing so the receiver can extract packets from
the continuous bitstream and detect lost or misordered data.

Each point-to-point link uses one transfer frame protocol. The
choice depends on the link direction and type.

- [TM](tm) — Telemetry Transfer Frame (132.0-B-3) for downlink
- [TC](tc) — Telecommand Transfer Frame (232.0-B-4) for uplink
- [AOS](aos) — Advanced Orbiting Systems (732.0-B-4) for high-rate downlink
- [Proximity-1](proximity1) — short-range inter-spacecraft links (211.2-B-3)
- [USLP](uslp) — Unified Space Data Link Protocol (732.1-B-3)
