# Constellations

A satellite constellation is a group of satellites working together to provide coverage or services that a single satellite cannot. The arrangement of orbital planes, the number of satellites per plane, and the relative spacing between them determine the constellation's coverage, revisit time, and network topology.

## Walker Constellations

Most LEO constellations use a Walker pattern — a regular arrangement where all orbital planes share the same inclination and altitude, and satellites are evenly spaced. Two variants exist:

- **Walker Delta** — orbital planes are spread evenly around the full 360° of RAAN. This provides global coverage. Used by most LEO constellations including Starlink, Iridium, and the LeoDOS default configuration.
- **Walker Star** — orbital planes are spread over 180° of RAAN (half the circle). Satellites in adjacent planes travel in opposite directions at the poles, creating a "seam" where relative velocities are very high. Used when coverage is needed primarily at specific latitudes.

A Walker constellation is described by three numbers: **T/P/F** where T = total satellites, P = number of planes, and F = the phasing parameter (how satellites in adjacent planes are offset from each other). For example, Iridium is 66/6/2.

## Constellation Parameters

- **Number of planes** — how many orbital planes. More planes provide better coverage between planes but require more RAAN spread.
- **Satellites per plane** — how many satellites in each plane. More satellites per plane reduce the gap between consecutive passes over the same ground point.
- **Altitude** — higher orbits see more of Earth's surface per satellite (larger footprint) but with lower resolution. Also affects orbital period, atmospheric drag, and radiation exposure.
- **Inclination** — determines which latitudes are covered. A 53° inclination covers latitudes up to ±53°. A 97° (near-polar) inclination covers the entire globe.
- **Phasing (F)** — the relative angular offset between satellites in adjacent planes. Affects how evenly the constellation covers the ground. F=0 means satellites in adjacent planes are aligned; F=1 means they are offset by one satellite spacing.
- **RAAN spacing** — the angular separation between orbital planes. In a Walker Delta with N planes, this is 360°/N by default, but can be customized.

## Real Constellations

| Constellation | Operator | Satellites | Planes | Altitude | Inclination | Purpose |
|---|---|---|---|---|---|---|
| **Starlink** | SpaceX | ~6,000+ | Multiple shells | 540–570 km | 53°, 70°, 97° | Internet |
| **OneWeb** | OneWeb | 648 | 12 | 1,200 km | 87.9° | Internet |
| **Iridium** | Iridium | 66 | 6 | 780 km | 86.4° | Voice/data |
| **Kuiper** | Amazon | 3,236 | Multiple shells | 590–630 km | 30°, 42°, 52° | Internet |
| **Iris²** | EU/ESA | ~290 | TBD | MEO + LEO | TBD | Secure comms |
| **Telesat** | Telesat | 298 | 6 + 5 | 1,015–1,325 km | 99°, 37° | Internet |

LeoDOS's default configuration uses 20 planes × 72 satellites at 550 km and 87° inclination — similar in scale to OneWeb and Iridium.

## Topology

A Walker Delta constellation forms a [2D torus network](/spacecomp/constellation). Each satellite has four ISL neighbors (north, south within the same plane; east, west to adjacent planes). The topology wraps around in both dimensions. This regular structure enables efficient [routing](/spacecomp/routing) with predictable hop counts.
