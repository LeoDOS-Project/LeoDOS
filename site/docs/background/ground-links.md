# Ground ↔ Satellite

A ground station sees a LEO satellite only when it is above the local horizon. The **elevation angle** — the angle between the horizon and the satellite as seen from the station — determines link quality: higher elevation means shorter path through the atmosphere and better signal. A minimum elevation of 5–10° is typical; below that, atmospheric attenuation and multipath make the link unreliable.

## Contact Windows

A single pass lasts 5–15 minutes, depending on the satellite's altitude and the pass geometry (a pass directly overhead lasts longer than one near the horizon). At 550 km altitude, a satellite completes one orbit in ~96 minutes, so a ground station sees it roughly once every 1.5 hours — but not every orbit, because Earth rotates and the satellite's ground track shifts westward with each pass.

High-latitude ground stations (Kiruna, Svalbard, Fairbanks) see more passes per day for near-polar orbits because the orbital planes converge near the poles.

## Ground Link Throughput

The ground-to-satellite link is the bottleneck in the system. Unlike ISLs which operate in vacuum, the ground link must traverse Earth's atmosphere, which introduces losses that ISLs avoid:

- **Atmospheric attenuation** — the atmosphere absorbs and scatters the signal. The effect depends on frequency, weather, and the path length through the atmosphere (longer at low elevation angles).
- **Rain fade** — at higher RF frequencies (Ka-band and above), rain can severely attenuate the signal. Heavy rain can reduce throughput by an order of magnitude or cause link outages.
- **Spectrum regulation** — RF downlink frequencies are shared and regulated. Satellites cannot transmit at arbitrary power or bandwidth. Available spectrum is limited.

Typical ground link data rates for Earth observation missions:

- **X-band** (8 GHz) — 150–800 Mbps. The traditional workhorse for Earth observation downlink. Moderate bandwidth, relatively tolerant of weather.
- **Ka-band** (26 GHz) — 1–4 Gbps. Higher throughput but more susceptible to rain fade. Used by newer missions needing higher data rates.
- **Optical ground link** — 1–10+ Gbps. Highest throughput but requires clear skies — clouds block the laser entirely. Only viable at sites with high clear-sky availability.

For comparison, a single ISL achieves 10–100+ Gbps in vacuum with no weather dependence. The ground link is typically 10–100× slower than the ISL mesh. This asymmetry is the core of the [downlink wall](/background/threats#the-downlink-wall): the constellation can move data internally much faster than it can get data to the ground.

## Cross-Orbit Relay

LEO satellites can communicate with relay satellites in GEO to maintain near-continuous ground connectivity, bypassing the contact window limitation.

The relay path has two segments that use different technologies:

- **LEO → GEO** (inter-orbit uplink) — optical laser. EDRS uses a 1064 nm laser link at 1.8 Gbps between the LEO satellite and the GEO relay. The link operates in vacuum, so it has the same characteristics as an [ISL](/background/satellite-links) — no atmospheric effects, but requires precise pointing between orbits 35,000 km apart.
- **GEO → Ground** (relay downlink) — Ka-band RF (~26 GHz). Even systems that use laser for the inter-orbit leg switch to RF for the final hop to ground, because laser links through the atmosphere are blocked by clouds. Ka-band is reliable enough for continuous service.

| System | Agency | LEO ↔ GEO link | GEO ↔ Ground link | Data rate | Status |
|---|---|---|---|---|---|
| **TDRSS** | NASA | S-band and Ka-band RF | S-band and Ka-band RF | ~300 Mbps | Operational since 1983, 9 satellites |
| **EDRS** | ESA / Airbus | Optical laser (1064 nm) | Ka-band RF | 1.8 Gbps | Operational since 2016, 40 TB/day capacity |
| **LCRD** | NASA | Optical laser | Optical laser | 1.2 Gbps | Demonstration since 2021, testing optical ground links |

A GEO relay adds ~250 ms round-trip latency (LEO → GEO → ground → GEO → LEO) but eliminates the ground contact gap. This creates two communication planes:

- **Control plane** — through GEO relay: commands, status, workflow uploads. Low bandwidth, near-continuous.
- **Data plane** — through ISL mesh and direct ground passes: sensor data, file transfers, results. High bandwidth, intermittent.

The downlink wall — the fundamental mismatch between data generation rate and downlink capacity — is not solved by a GEO relay. Even EDRS at 1.8 Gbps cannot match the 1–2 TB/day that an Earth observation satellite generates. Bulk data still needs onboard processing and selective downlink.
