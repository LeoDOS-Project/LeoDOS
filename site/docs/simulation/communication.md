# Communication

The generic radio simulator replaces the physical RF transceiver with UDP sockets. Two link types are modeled:

- **Ground link** — uplink and downlink between a satellite and a ground station, each a UDP socket pair
- **Inter-satellite link (ISL)** — proximity radio connecting neighboring satellites in the [2D torus](/spacecomp/constellation). The [ISL router](/protocols/network/routing) forwards packets across multiple hops; [SRSPP](/protocols/transport/srspp) provides reliable delivery.

## Supported

**Line-of-sight gating** — the routing layer computes satellite positions in ECEF coordinates and checks elevation angles against a minimum threshold (default 5°). Ground station links are only established when a satellite is visible above the horizon. The gateway table resolves which satellite currently has line of sight to each ground station (Kiruna, Svalbard, Fairbanks in the default configuration) and routes ground-bound traffic through it.

**Bounded queues** — the router maintains a fixed-size output buffer per direction (north, south, east, west, ground). When a buffer fills, incoming packets are dropped rather than queued indefinitely. This provides a structural bandwidth constraint — a link that cannot drain fast enough will cause packet loss, exercising the [SRSPP](/protocols/transport/srspp) retransmission and [backpressure](/cfs/mission/communication) mechanisms.

**Full protocol stack** — the simulation runs the complete communication stack: [transfer frames](/protocols/datalink/transfer-frame/overview), [COP-1](/protocols/datalink/reliability/cop1) link reliability, [SDLS](/protocols/datalink/security/sdls) encryption, [coding layer](/protocols/coding/overview) (randomization, Reed-Solomon, framing), and [SRSPP](/protocols/transport/srspp) transport. All protocol logic is exercised identically to flight.

## Not Yet Supported

**Propagation delay** — packets arrive instantly over UDP. Real ISL links have 1–5 ms propagation delay depending on inter-satellite distance.

**RF channel modeling** — signal-to-noise ratio, path loss, link margin, interference, and noise are not simulated. The radio simulator provides a clean channel with no errors. RF performance must be validated separately with link analysis tools or hardware-in-the-loop testing.

**Real modulation/demodulation** — the [physical layer](/protocols/physical/overview) (BPSK, QPSK, etc.) is handled by real radio hardware in flight. In simulation, the data bypasses the RF chain entirely.
