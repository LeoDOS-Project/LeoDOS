# Communication

The simulation replaces the physical RF transceiver with UDP sockets, allowing multiple satellites to communicate on a single machine. The entire [communication stack](/protocols/overview) — framing, encryption, routing, reliable transport — runs unmodified on top of these simulated links.

## Supported

- **Ground-to-satellite links** — uplink and downlink between satellites and ground stations
- **Inter-satellite links** — links between neighboring satellites in the [2D torus](/spacecomp/constellation), with multi-hop routing across the mesh
- **Multiple simultaneous satellites** — each satellite has its own radio simulator, so the full constellation mesh can operate concurrently

## Not Yet Supported

- **Propagation delay** — packets arrive instantly. Real ISL links have 1–5 ms delay depending on distance.
- **RF channel effects** — signal-to-noise ratio, path loss, link margin, interference, and noise
- **Modulation/demodulation** — the [physical layer](/protocols/physical/overview) (BPSK, QPSK, etc.) is bypassed in simulation
