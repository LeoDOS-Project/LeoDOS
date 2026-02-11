# SpaceCoMP Paper Analysis

Analysis of "Lightspeed Data Compute for the Space Era" (SpaceCoMP).

## Constellation Model

Walker Delta constellation:
- N orbital planes, M satellites per plane
- All planes at same inclination (e.g., 87°)
- Planes separated by 360/N degrees around Earth
- Forms a 2D torus network topology

### Labeling Convention

In our analysis:
- **Numbers (1, 2, 3, 4)** = different orbital planes
- **Letters (A, B, C, D, E, F)** = positions along each orbit

Example: A4, B4, C4, D4, E4, F4 are 6 satellites evenly spaced around orbit 4.

## Ascending vs Descending

Each satellite is either ascending (moving toward north pole) or descending (moving toward
south pole) at any moment. This creates a geographic split:

- One hemisphere contains all ascending satellites
- Other hemisphere contains all descending satellites
- The boundary passes through both poles

### Link Constraints

**Intra-plane links (same orbit):** Always work. Satellites in the same orbit maintain
formation and move together. A4 ↔ B4 ↔ C4 ↔ D4 ↔ E4 ↔ F4 are fixed neighbors.

**Inter-plane links (between orbits):**
- Within same phase (both ascending or both descending): Work normally
- Across the boundary (ascending ↔ descending): Dynamic, constantly changing

### Dynamic Cross-Boundary Links

Ascending and descending satellites pass by each other at the boundary. At any instant,
some pairs can communicate:

```
Time T:          Time T+1:
C4 ↔ D1          B4 ↔ D1
B4 ↔ E1          A4 ↔ E1
A4 ↔ F1          F4 ↔ F1
```

The specific pairing changes constantly as satellites orbit. This is a "sliding seam" -
always a connection available, but which satellites are paired keeps shifting.

## Network Connectivity

The network IS globally connected, but:

1. **Fixed links:** Intra-plane links and inter-plane links within same phase
2. **Dynamic links:** Cross-boundary links change constantly
3. **Routing is time-dependent:** Path from A to B depends on when you send the message

## Issues Not Addressed in Paper

### 1. Cross-Boundary Routing

The paper sidesteps global routing by restricting computations to one hemisphere:

> "it is only possible to select only ascending or only descending satellites for any
> computation, but not a mix"

This works for localized AOI computations but doesn't solve global routing.

### 2. Scheduling Time Constraints

If a satellite crosses the ascending/descending boundary mid-computation:
- It loses inter-plane links to former neighbors
- Data on that satellite becomes temporarily unreachable

**Implications:**
- Computations must complete before any involved satellite crosses the boundary
- Near poles: shorter time window (satellites cross quickly)
- Near equator: longer time window

A robust scheduler should:
- Estimate job duration
- Calculate when each satellite will cross the boundary
- Only schedule if job can complete in time
- Or implement checkpoint/migration before boundary crossing

### 3. The "Donut" Terminology

The paper describes the topology as "doughnut-shaped." This is misleading:

- **Physical shape:** Satellites are distributed around a sphere (Earth), converging at poles
- **Network topology:** The connectivity pattern forms a torus (donut)

The torus is an abstract description of how satellites connect (both dimensions wrap around),
not the physical arrangement.

## Possible Solutions (Not in Paper)

To improve global connectivity:

1. **Counter-rotating orbits:** Some planes at +87°, others at -87° (retrograde). This would
   place both ascending and descending satellites on each side of Earth.

2. **Multiple shells:** Different satellite groups at different inclinations (like Starlink).

3. **Equatorial orbits:** 0° inclination satellites could bridge the hemispheres.

4. **Ground relay:** Downlink, route on ground, uplink to other hemisphere.

## Visualization

Top-down view from North Pole:

```
                    ascending
        ┌────────────────────────────┐
        │    A3                      │
        │         B3                 │
        │  A2          C3            │
        │       B2          C4       │
        │            C2         D1   │
  A1────B1────C1────●────D1────E1────F1
        │            D2         E1   │
        │       D3          E2       │
        │  D4          E3            │
        │         E4                 │
        │    F4                      │
        └────────────────────────────┘
                   descending
```

All orbital planes converge at the center (pole). The diagonal line separates ascending
from descending. Cross-boundary links (like C4↔D1) exist but change constantly.
