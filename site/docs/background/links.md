# Links

Communication in a satellite constellation happens over three types of links: inter-satellite links between constellation members, ground links between satellites and ground stations, and cross-orbit relay links through satellites at higher altitudes.

## Inter-Satellite Links

Satellites in a constellation communicate directly with each other via inter-satellite links (ISL). In a [Walker Delta constellation](/spacecomp/constellation), each satellite maintains links to four neighbors forming a 2D torus mesh. Data can be routed across multiple hops to reach any satellite in the constellation.

Two ISL technologies exist:

- **Optical (laser)** — 10–100+ Gbps bandwidth, low power for the data rate, but requires precise pointing between moving platforms (beam divergence is very small). Used by Starlink and planned for most new constellations.
- **RF (radio)** — lower bandwidth (hundreds of Mbps to low Gbps) but wider beam width, making acquisition and tracking simpler. More tolerant of pointing errors.

ISL latency within LEO is 1–5 ms per hop, depending on the distance between satellites. A path across 10 hops takes tens of milliseconds — comparable to terrestrial internet latency.

### Distance and Throughput

ISL link quality degrades with distance. The signal weakens according to free-space path loss, which grows with the square of distance. A link that is twice as long has four times the path loss, resulting in lower signal-to-noise ratio and reduced channel capacity. This means longer links carry less data per second, not just with more delay.

In a Walker Delta constellation, cross-plane link distances vary by ~40% over each orbit — shortest near the poles (where orbital planes converge) and longest near the equator (where they diverge). This variation directly affects throughput: the same cross-plane hop carries more data near the poles than near the equator. [Distance-minimizing routing](/spacecomp/routing) exploits this by scheduling cross-plane hops when they are shortest.

## Ground Station Passes

A ground station sees a LEO satellite only when it is above the local horizon. The **elevation angle** — the angle between the horizon and the satellite as seen from the station — determines link quality: higher elevation means shorter path through the atmosphere and better signal. A minimum elevation of 5–10° is typical; below that, atmospheric attenuation and multipath make the link unreliable.

A single pass lasts 5–15 minutes, depending on the satellite's altitude and the pass geometry (a pass directly overhead lasts longer than one near the horizon). At 550 km altitude, a satellite completes one orbit in ~96 minutes, so a ground station sees it roughly once every 1.5 hours — but not every orbit, because Earth rotates and the satellite's ground track shifts westward with each pass.

High-latitude ground stations (Kiruna, Svalbard, Fairbanks) see more passes per day for near-polar orbits because the orbital planes converge near the poles.

## Cross-Orbit Relay

LEO satellites can communicate with relay satellites in higher orbits to maintain near-continuous ground connectivity:

- **TDRSS** (NASA) — GEO relay satellites providing contact with LEO spacecraft (ISS, Hubble, science missions)
- **EDRS** (ESA) — European Data Relay System with optical laser terminals relaying LEO data to ground

A GEO relay adds ~250 ms round-trip latency (LEO → GEO → ground → GEO → LEO) but eliminates the ground contact gap. This creates two communication planes:

- **Control plane** — through GEO relay: commands, status, workflow uploads. Low bandwidth, near-continuous.
- **Data plane** — through ISL mesh and direct ground passes: sensor data, file transfers, results. High bandwidth, intermittent.

The [downlink wall](environment#the-downlink-wall) — the fundamental mismatch between data generation rate and downlink capacity — is not solved by a GEO relay. Bulk data still needs direct passes or ISL routing.
