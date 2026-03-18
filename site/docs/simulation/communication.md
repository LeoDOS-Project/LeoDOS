# Communication

The generic radio simulator replaces the physical RF transceiver with UDP sockets. Two link types are modeled:

- **Ground link** — uplink and downlink between a satellite and a ground station, each a UDP socket pair
- **Inter-satellite link (ISL)** — proximity radio connecting neighboring satellites in the [2D torus](/spacecomp/constellation). The [ISL router](/protocols/network/routing) forwards packets across multiple hops; [SRSPP](/protocols/transport/srspp) provides reliable delivery.

The simulation validates the full protocol stack — framing, routing, reliability, security — but does not model the physical RF channel:

| Gap | Reality |
|---|---|
| Propagation delay | Real ISL: 1–5 ms depending on distance |
| Line-of-sight gating | Satellites occluded by Earth cannot communicate |
| Bandwidth constraints | Real RF links have limited data rates |
| Path loss and link margin | Signal degrades with distance and atmosphere |
| Interference and noise | Real links share spectrum |

RF performance must be validated separately with link analysis tools or hardware-in-the-loop testing.
