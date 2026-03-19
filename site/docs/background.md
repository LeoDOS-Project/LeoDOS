# Background

This page provides context for readers who are not from the space domain.

## Orbits

Satellites orbit Earth at different altitudes, each with different properties:

| Orbit | Altitude | Latency (one-way) | Orbital period | Example |
|---|---|---|---|---|
| **LEO** (Low Earth Orbit) | 200–2,000 km | ~1–5 ms | ~90–120 min | Starlink, ISS, Earth observation |
| **MEO** (Medium Earth Orbit) | 2,000–35,786 km | ~10–80 ms | ~2–12 hours | GPS, O3b |
| **GEO** (Geostationary) | 35,786 km | ~120 ms | 24 hours (stationary) | TDRSS, EDRS, TV broadcast |

LEO is where Earth observation happens. Satellites are close enough to capture high-resolution imagery, but they move fast — a ground station sees a LEO satellite for only 5–15 minutes per pass. GEO satellites appear stationary from the ground and provide continuous coverage, but are far away (high latency, weaker signal, lower resolution).

## The Downlink Wall

LEO Earth observation satellites generate 1–2 TB of sensor data per day. Ground contact windows allow only a fraction of this to be downlinked. This is the fundamental problem LeoDOS addresses: process data onboard and downlink only the results.

| What | Size | Example |
|---|---|---|
| Raw SAR strip | ~2 GB | Full resolution radar image over a dam |
| Alert packet | ~2 KB | "Displacement exceeds 5 mm at these coordinates" |
| Reduction factor | ~10⁶ | Processing onboard avoids downlinking data that isn't needed |

## Inter-Satellite Links

Satellites in a constellation communicate with each other via inter-satellite links (ISL). In a [Walker Delta constellation](/spacecomp/constellation), each satellite has four ISL neighbors forming a 2D torus mesh. Data can be routed across multiple hops to reach a satellite that has ground contact.

ISL types:
- **Optical** — laser links, 10–100+ Gbps, requires precise pointing
- **RF** — radio links, lower bandwidth but simpler pointing requirements

ISL latency within LEO is low (1–5 ms per hop), so the mesh behaves like a connected network most of the time.

## Cross-Orbit Communication

LEO satellites can communicate with GEO relay satellites to maintain near-continuous ground connectivity:

- **TDRSS** (NASA) — GEO relay satellites providing contact with LEO spacecraft (ISS, Hubble, science missions)
- **EDRS** (ESA) — European Data Relay System with optical laser terminals relaying LEO data to ground

A GEO relay adds ~250 ms round-trip latency (LEO→GEO→ground→GEO→LEO) but eliminates the ground contact gap. This is useful for commands and status (low bandwidth, always available) but does not solve the downlink wall — bulk sensor data still needs direct ground passes or ISL routing to a ground-visible satellite.

The architecture this suggests is two communication planes:
- **Control plane** — through GEO relay: commands, status, workflow uploads, coordination. Low bandwidth, near-continuous.
- **Data plane** — through ISL mesh and direct ground passes: sensor data, bulk file transfers, results. High bandwidth, intermittent.

## Ground Stations

Ground stations are fixed locations with antennas pointed at the sky. A LEO satellite passes over a ground station for a few minutes each orbit. Multiple ground stations at different locations increase total contact time. Common locations are chosen for high-latitude coverage (more passes per day):

- Kiruna, Sweden
- Svalbard, Norway
- Fairbanks, Alaska

## Flight Software

Satellites run real-time flight software on radiation-hardened processors. The software manages all onboard operations: attitude control, power management, communication, payload data processing. LeoDOS uses NASA's [Core Flight System](/cfs/overview) as the flight software framework, with applications written in Rust.
