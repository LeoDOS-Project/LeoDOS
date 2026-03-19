# Orbits

A satellite's orbit is defined by a set of parameters that determine its path around a body. Understanding these parameters explains why constellations are shaped the way they are and why certain orbits are chosen for specific missions.

## Altitude Classes

| Orbit | Altitude | Orbital period | Latency (one-way) | Example |
|---|---|---|---|---|
| **LEO** (Low Earth Orbit) | 200–2,000 km | ~90–120 min | ~1–5 ms | Starlink, ISS, Earth observation |
| **MEO** (Medium Earth Orbit) | 2,000–35,786 km | ~2–12 hours | ~10–80 ms | GPS, O3b |
| **GEO** (Geostationary) | 35,786 km | 24 hours (stationary) | ~120 ms | TDRSS, EDRS, TV broadcast |

LEO is where Earth observation happens — close enough for high-resolution imagery, but the satellite moves fast and is only visible from a ground station for minutes per pass. GEO satellites appear stationary from the ground and provide continuous coverage of one hemisphere, but are far away (high latency, weaker signal, lower resolution).

## Orbital Parameters

Six parameters (Keplerian elements) fully describe an orbit:

- **Semi-major axis (a)** — the size of the orbit. For circular orbits, this is the radius. Determines the altitude and orbital period — higher orbits are slower.
- **Eccentricity (e)** — the shape of the orbit. 0 = perfect circle, 0–1 = ellipse. LEO constellations use near-circular orbits (e ≈ 0) for constant altitude.
- **Inclination (i)** — the tilt of the orbital plane relative to the equator. 0° = equatorial, 90° = polar, 97–98° = sun-synchronous. Determines which latitudes the satellite passes over.
- **RAAN (Ω)** — Right Ascension of the Ascending Node. The angle where the orbital plane crosses the equator going northward, measured from a fixed reference direction (the vernal equinox). Determines the orientation of the orbital plane in space.
- **Argument of periapsis (ω)** — where the lowest point of the orbit is within the orbital plane. Irrelevant for circular orbits (e = 0) but important for elliptical ones.
- **True anomaly (ν)** — the satellite's current position along the orbit, measured as an angle from periapsis. Changes continuously as the satellite moves.

For a circular LEO constellation, only three parameters matter in practice: altitude (from semi-major axis), inclination, and RAAN (which defines the orbital plane).

## Perturbations

Real orbits are not perfect Keplerian ellipses. Forces other than the central body's gravity cause the orbit to change over time:

- **J2 (Earth oblateness)** — Earth is not a perfect sphere; it bulges at the equator. This causes the orbital plane to rotate (RAAN drift) and the argument of periapsis to precess. The rate depends on altitude and inclination. J2 is the dominant perturbation in LEO.
- **Atmospheric drag** — at LEO altitudes (especially below 500 km), residual atmosphere slows the satellite, gradually lowering its orbit. Without periodic orbit-raising maneuvers, the satellite eventually reenters. Drag depends on altitude, solar activity (which heats and expands the upper atmosphere), and the satellite's ballistic coefficient (mass-to-area ratio).
- **Third-body gravity** — gravitational pull from the Moon and Sun causes small periodic perturbations. More significant at higher altitudes.
- **Solar radiation pressure** — photons from the Sun exert a small force on the satellite's surface. Relevant for large, lightweight structures (solar panels).

## Sun-Synchronous Orbits

A sun-synchronous orbit (SSO) is a special inclination (typically 97–98° for LEO altitudes) where the J2 precession rate exactly matches Earth's orbital motion around the Sun. The result: the orbital plane maintains a constant angle relative to the Sun throughout the year.

This means the satellite passes over any given ground location at the same local solar time on every pass. For Earth observation, this provides consistent illumination conditions — shadows fall the same way in every image, making change detection easier. Most Earth observation satellites (Sentinel-1, Sentinel-2, Landsat) use sun-synchronous orbits.

The required inclination depends on altitude and eccentricity. For a circular orbit at 550 km, the sun-synchronous inclination is approximately 97.6°.
