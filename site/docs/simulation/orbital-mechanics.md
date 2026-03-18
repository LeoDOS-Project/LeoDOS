# Orbital Mechanics

Orbit simulation is provided by 42, NASA's spacecraft dynamics simulator. 42 propagates orbits for every satellite in the constellation and models the space environment. It handles large constellations — 10,000+ satellites have been propagated successfully.

## Orbit Propagation

42 supports multi-body orbital dynamics with seamless transition between two-body and three-body models. Orbits are defined using standard Keplerian elements. For the LeoDOS [Walker Delta constellation](/spacecomp/constellation), 42 propagates all satellites simultaneously, maintaining the correct relative geometry as the constellation evolves.

## Space Environment

| Model | What it provides |
|---|---|
| EGM96 gravity (18th order) | Orbit perturbations at LEO altitude |
| IGRF magnetic field (10th order) | Magnetometer readings, magnetorquer control |
| MSIS-86 / Jacchia-Roberts atmosphere | Aerodynamic drag |
| Solar geometry | Sun position, eclipse/sunlight status per spacecraft |
| Celestial bodies (sun, 9 planets, 45 moons) | Third-body perturbations |

## Published State

At each timestep, 42 publishes to the hardware simulators:

- Date and time
- Spacecraft position and velocity (inertial and rotating frames)
- Attitude quaternion and angular velocity
- Sun vector (inertial frame)
- Magnetic field vector at the spacecraft
- Eclipse/sunlight flag
- Angular momentum

Each simulator reads the subset it needs — GPS reads position, the magnetometer reads the magnetic field, sun sensors read the sun vector.
