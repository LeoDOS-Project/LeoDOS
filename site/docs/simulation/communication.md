# Communication

The simulation models communication between satellites and between satellites and ground stations.

## Supported

- **Ground-to-satellite links** — uplink (commands) and downlink (telemetry) between satellites and ground stations
- **Inter-satellite links** — point-to-point links between neighboring satellites in the [2D torus](/spacecomp/constellation), with multi-hop routing across the mesh
- **Line-of-sight visibility** — ground links are only available when a satellite is above the horizon relative to a ground station. Satellites below the minimum elevation angle cannot communicate with ground.
- **Link congestion** — when a link cannot drain fast enough, packets are dropped, triggering retransmission at the transport layer
- **Full protocol stack** — [transfer frames](/protocols/datalink/transfer-frame/overview), [COP-1](/protocols/datalink/reliability/cop1) reliability, [SDLS](/protocols/datalink/security/sdls) encryption, [coding](/protocols/coding/overview) (randomization, Reed-Solomon, framing), and [SRSPP](/protocols/transport/srspp) reliable transport

## Not Yet Supported

- **Propagation delay** — packets arrive instantly. Real ISL links have 1–5 ms delay depending on distance.
- **RF channel effects** — signal-to-noise ratio, path loss, link margin, interference, and noise
- **Modulation/demodulation** — the [physical layer](/protocols/physical/overview) (BPSK, QPSK, etc.) is bypassed in simulation
