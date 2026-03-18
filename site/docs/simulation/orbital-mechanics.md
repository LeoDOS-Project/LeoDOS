# Orbital Mechanics

Orbital mechanics simulation is provided by 42, NASA's spacecraft dynamics simulator. 42 propagates orbits for every satellite in the constellation and provides position, velocity, and attitude data to the hardware simulators and flight software below.

## Orbit Propagation

42 supports multi-body orbital dynamics with seamless transition between two-body and three-body models. Orbits can be defined using standard Keplerian elements (semi-major axis, eccentricity, inclination, RAAN, argument of periapsis, true anomaly). For the LeoDOS [Walker Delta constellation](/spacecomp/constellation), 42 propagates all satellites simultaneously, maintaining the correct relative geometry as the constellation evolves.

42 can handle large constellations — 10,000+ satellites have been propagated successfully.

## Attitude Dynamics

Each spacecraft has a full 6-DOF attitude model. 42 computes angular velocity, quaternion orientation, and the direction cosine matrices between inertial, body, and rotating reference frames. Attitude sensors (sun sensors, star tracker, IMU, magnetometer) read their measurements from 42's computed state, and attitude actuators (reaction wheels, magnetorquers, thrusters) feed torques back into the dynamics.

## Space Environment

42 models the environment that affects spacecraft behavior:

- **Gravity** — Earth gravity field up to 18th order and degree (EGM96). Higher-order terms affect orbit perturbations, especially at low altitude.
- **Magnetic field** — International Geomagnetic Reference Field (IGRF) up to 10th order. Used by the magnetometer simulator and magnetorquer control.
- **Atmospheric density** — MSIS-86 and Jacchia-Roberts models for aerodynamic drag at LEO altitudes.
- **Solar geometry** — sun position, eclipse/sunlight status for each spacecraft. Drives solar array power and thermal models.
- **Celestial bodies** — sun, 9 planets, 45 major moons. Relevant for third-body perturbations and reference frame computations.

## What 42 Provides to Simulators

42 publishes the following data over TCP/IP sockets (via NOS Engine shared memory) at each simulation timestep:

- Date and time
- Spacecraft position and velocity in inertial and rotating frames
- Attitude quaternion and angular velocity
- Sun vector in the inertial frame
- Magnetic field vector at the spacecraft
- Eclipse/sunlight flag
- Angular momentum

Each hardware simulator reads the subset it needs — the GPS simulator reads position, the magnetometer reads the magnetic field, the sun sensors read the sun vector, and so on.
