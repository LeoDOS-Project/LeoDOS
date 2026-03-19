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

## Propagation Methods

Given the orbital parameters at one point in time, how do you compute where the satellite will be at a future time? Different methods trade off fidelity for complexity.

### Keplerian (Two-Body)

The simplest model. Assumes the satellite orbits a perfect point mass with no other forces. The orbit is a fixed ellipse that never changes — the satellite traces the same path forever.

Given a time, you can compute the satellite's position directly using Kepler's equation. This is an analytical solution: plug in a time, get a position. No stepping required. The computation is essentially free.

**What it captures:** orbital period, altitude, ground track geometry.

**What it misses:** everything else. The orbital plane does not drift, the orbit does not decay, and neighboring effects (gravity field shape, drag, third-body) are ignored. Over hours, the position error grows to hundreds of kilometers compared to reality.

**When to use:** quick visualization, initial constellation layout, rough ground track estimation.

### J2 (Analytical Perturbation)

Adds the dominant perturbation — Earth's equatorial bulge (the J2 term of the gravity field) — as an analytical correction on top of the Keplerian solution. J2 causes two effects: the orbital plane rotates around the polar axis (RAAN drift), and the argument of periapsis precesses within the plane.

Still an analytical formula — plug in a time, get a position. No numerical integration. The correction terms are simple trigonometric functions of the orbital elements.

**What it captures:** RAAN drift (critical for constellation maintenance — planes slowly spread apart or converge), sun-synchronous behavior (SSO works because J2 drift matches Earth's solar orbit rate), argument of periapsis precession.

**What it misses:** atmospheric drag, higher-order gravity harmonics, third-body effects, solar radiation pressure. Position error grows to tens of kilometers over days.

**When to use:** constellation design, ISL geometry analysis, ground pass prediction, [SpaceCoMP routing cost estimation](/spacecomp/routing). This is the propagator LeoDOS uses for orbital-aware routing decisions.

### SGP4 / TLE

SGP4 (Simplified General Perturbations 4) is the standard propagator used with Two-Line Element sets (TLEs) — the format in which NORAD publishes orbital data for all tracked objects. TLEs encode a satellite's orbital state at a specific epoch along with drag and perturbation terms fitted to observed tracking data.

SGP4 is an analytical propagator — plug in a time offset from the TLE epoch, get a position. It includes J2, J3, J4, atmospheric drag (using a simplified density model), and lunar/solar perturbations, all baked into a fixed set of formulas.

**What it captures:** real-world orbital behavior as observed by ground tracking networks. TLEs are updated daily or more frequently, so the propagator tracks reality closely near the epoch.

**What it misses:** accuracy degrades away from the epoch. After a few days, position errors grow to kilometers. TLEs also cannot represent maneuvers — after a thruster burn, a new TLE must be published.

**When to use:** tracking real satellites (Starlink, ISS, Iridium), collision conjunction assessment, comparing a simulated constellation against real-world positions.

### Numerical Integration

Solves the full equations of motion by stepping forward in time, computing all forces at each step and updating position and velocity. Can include any combination of forces: high-order gravity field (EGM96, up to 18th degree), atmospheric drag with detailed density models (MSIS, Jacchia-Roberts), third-body gravity (Moon, Sun, planets), solar radiation pressure, and thruster maneuvers.

This is not an analytical formula — you cannot jump to an arbitrary time. You must integrate step by step from the initial state, typically using a Runge-Kutta or similar method. The step size depends on the desired accuracy and the forces involved. Smaller steps give higher accuracy but require more computation.

**What it captures:** all modeled forces to arbitrary precision. Position accuracy can be meters over days with a good force model and initial state.

**What it misses:** only forces that are not included in the model. Practical limitations: initial state must be very accurate, atmospheric density models have uncertainty (especially during solar storms), and unmodeled maneuvers cause rapid divergence.

**When to use:** precise orbit determination, maneuver planning, high-fidelity simulation. The [42 simulator](/simulation/orbital-mechanics) uses numerical integration with the full force model.

### Comparison

| Method | Type | Accuracy (1 day) | Forces modeled |
|---|---|---|---|
| **Keplerian** | Analytical (closed-form) | ~100 km | Point-mass gravity only |
| **J2** | Analytical (closed-form) | ~10 km | Gravity + equatorial bulge |
| **SGP4/TLE** | Analytical (fitted) | ~1 km (near epoch) | Gravity + drag + lunar/solar (fitted to observations) |
| **Numerical** | Step-by-step integration | ~meters | All forces (configurable) |

Each level adds fidelity. For constellation design and routing, J2 is sufficient — it captures the orbital plane geometry that determines ISL distances and ground contact windows. For simulation with hardware-in-the-loop, numerical integration (via 42) provides the ground truth.

## Sun-Synchronous Orbits

A sun-synchronous orbit (SSO) is a special inclination (typically 97–98° for LEO altitudes) where the J2 precession rate exactly matches Earth's orbital motion around the Sun. The result: the orbital plane maintains a constant angle relative to the Sun throughout the year.

This means the satellite passes over any given ground location at the same local solar time on every pass. For Earth observation, this provides consistent illumination conditions — shadows fall the same way in every image, making change detection easier. Most Earth observation satellites (Sentinel-1, Sentinel-2, Landsat) use sun-synchronous orbits.

The required inclination depends on altitude and eccentricity. For a circular orbit at 550 km, the sun-synchronous inclination is approximately 97.6°.

## Attitude

A satellite's orbit describes *where* it is. Its attitude describes *which way it's pointing* — the orientation of the spacecraft body relative to a reference frame.

Attitude matters because spacecraft are not symmetric. Antennas must point at ground stations or ISL neighbors. Solar panels must face the Sun. Cameras must point at the Earth's surface. Thrusters must fire in specific directions. If the attitude is wrong, none of these work.

### Representation

Attitude is typically represented as a rotation from an inertial reference frame to the spacecraft body frame. Common representations:

- **Quaternion** — four numbers that describe a rotation without the singularities (gimbal lock) that Euler angles suffer from. The standard representation used in flight software.
- **Direction cosine matrix (DCM)** — a 3×3 rotation matrix. Mathematically equivalent to a quaternion but uses nine numbers instead of four.
- **Euler angles** — three angles (roll, pitch, yaw). Intuitive for humans but has mathematical problems at certain orientations. Used in ground displays, not usually in onboard computation.

### Determination

The satellite needs to know its current attitude before it can control it. Attitude determination uses sensor measurements:

- **Star tracker** — photographs the star field and matches patterns against a catalog to compute orientation in inertial space. Most accurate sensor (arcsecond-level) but needs a clear field of view and processing time.
- **Sun sensors** — measure the direction to the Sun. Coarse (degrees) but simple and reliable. Multiple sensors on different faces give a full sun vector.
- **Magnetometer** — measures Earth's magnetic field direction. By comparing against a model (IGRF) at the known orbital position, attitude can be estimated. Lower accuracy but works in all lighting conditions.
- **IMU (gyroscope)** — measures angular rate, not absolute orientation. Integrating the rate gives attitude change over time, but errors accumulate (gyro drift). Must be periodically corrected by an absolute sensor (star tracker or sun sensor).

In practice, an attitude determination algorithm fuses all sensor inputs — the star tracker provides periodic absolute fixes, the gyroscope propagates between fixes, and sun sensors and magnetometer provide backup when the star tracker is unavailable (e.g., blinded by the Sun or Earth).

### Control

Once the satellite knows its attitude, actuators change it:

- **Reaction wheels** — spinning flywheels that trade angular momentum with the spacecraft. Spinning a wheel faster in one direction rotates the spacecraft in the opposite direction. Precise, fast, but momentum accumulates over time from external torques (atmospheric drag, gravity gradient) and must be periodically removed.
- **Magnetorquers** — electromagnetic coils that push against Earth's magnetic field. Used to remove accumulated momentum from reaction wheels (desaturation). Low torque, but requires no propellant.
- **Thrusters** — produce torque by expelling propellant. Used for large attitude changes or when reaction wheels are saturated. Consumes finite propellant.

### Pointing Modes

A satellite typically operates in one of several pointing modes:

- **Nadir pointing** — the camera or antenna always points straight down at Earth. The most common mode for Earth observation.
- **Sun pointing** — solar panels face the Sun for maximum power. Used during safe mode or when the battery is low.
- **Target tracking** — the satellite slews to point at a specific ground target or another satellite (for ISL acquisition). Requires the attitude control system to follow a moving target.
- **Inertial hold** — the satellite maintains a fixed orientation relative to the stars. Used during star tracker calibration or specific science observations.
