# Constellation

SpaceCoMP operates on a Walker Delta constellation — a regular arrangement of satellites in equally spaced orbital planes that forms a 2D torus network topology.

## Walker Delta Geometry

A Walker Delta constellation has N orbital planes with M satellites per plane. All planes share the same inclination (e.g., 87°) and are separated by 360°/N in right ascension. Satellites within each plane are equally spaced by 360°/M in mean anomaly. The default LeoDOS configuration uses 20 orbital planes with 72 satellites per plane at 550 km altitude.

## 2D Torus Topology

The constellation maps to a grid where one axis is the orbital plane index and the other is the satellite index within its plane. Because both axes wrap around (plane 0 is adjacent to plane N-1, satellite 0 is adjacent to satellite M-1), the topology is a torus.

Each satellite has four neighbors:

- **North** — previous satellite in the same plane (intra-plane)
- **South** — next satellite in the same plane (intra-plane)
- **East** — same-position satellite in the next plane (cross-plane)
- **West** — same-position satellite in the previous plane (cross-plane)

### Labeling Convention

Satellites are identified by a grid coordinate `(orb, sat)` where `orb` is the orbital plane index and `sat` is the position within the plane. In diagrams, numbers (1, 2, 3, 4) label orbital planes and letters (A, B, C, D, E, F) label positions along an orbit. For example, A4 through F4 are 6 satellites evenly spaced around orbital plane 4.

## Ascending vs Descending

Each satellite is either ascending (moving toward the north pole) or descending (moving toward the south pole), creating a geographic split. One hemisphere contains all ascending satellites; the other contains all descending. The boundary between the two halves passes through both poles.

## Link Constraints

**Intra-plane links** (north/south, same orbit) always work. Satellites within the same plane maintain constant spacing — the distance between them does not change.

**Cross-plane links** (east/west, between orbits) vary with orbital position:

- When both satellites are in the same phase (both ascending or both descending), cross-plane links work normally. The distance between planes depends on the satellite's position along the orbit — minimum near the poles (where orbital planes converge) and maximum near the equator.
- At the ascending/descending boundary, adjacent planes move in opposite directions with relative velocities exceeding 15 km/s. Cross-plane links across this boundary are unreliable and change constantly — this is the "seam" of the torus.

The cross-plane ISL distance at a given orbital position is:

$$
D_{\text{cross}}(\theta) = R \cdot \sqrt{2(1 - \cos(2\pi/N))} \cdot \sqrt{\cos^2(\theta) + \cos^2(i) \cdot \sin^2(\theta)}
$$

where $R$ is the orbital radius, $\theta$ is the true anomaly (position along the orbit), $i$ is the inclination, and $N$ is the number of planes. For high-inclination orbits, this distance varies by roughly 40% between equator and poles.

## Constraints on Computation

The ascending/descending boundary imposes constraints on distributed computation:

- **Hemisphere restriction** — SpaceCoMP can restrict jobs to ascending-only or descending-only satellites, avoiding the unreliable seam. This works for localized computations where the area of interest falls within one hemisphere.
- **Time constraints** — if a satellite crosses the boundary mid-computation, it loses its cross-plane links. Computations must complete before boundary crossing, or the system must implement checkpointing and migration.

## Geographic Projection

SpaceCoMP converts between geographic coordinates (latitude/longitude of the area of interest) and grid coordinates (orbital plane, satellite index) using the Walker Delta geometry. For each satellite, the nadir point (sub-satellite ground track position) is computed from:

- RAAN (Right Ascension of Ascending Node) = `orb × 360°/N`
- True anomaly = `sat × 360°/M`
- Latitude = arcsin(sin(i) × sin(ν))
- Longitude = Ω + atan2(cos(i) × sin(ν), cos(ν))

This projection identifies which satellites cover a geographic area of interest, producing a grid-space bounding box that the [job planner](job-lifecycle) uses for collector and mapper selection.
