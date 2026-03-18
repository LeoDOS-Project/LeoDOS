# Overview

LeoDOS uses NASA's NOS3 framework to simulate a satellite constellation on a development machine. The simulation runs the same cFS flight software as the real spacecraft — application code is identical in both environments, with only the hardware abstraction layer swapped.

The simulation models three domains: orbital mechanics, onboard hardware, and inter-satellite communication.

## Orbital Mechanics

Orbit simulation is provided by 42, NASA's spacecraft dynamics simulator. 42 propagates orbits for every satellite in the constellation and computes their physical state at each timestep. It handles large constellations — 10,000+ satellites have been propagated successfully.

**Orbit propagation** — multi-body dynamics with Keplerian elements. For the LeoDOS [Walker Delta constellation](/spacecomp/constellation), 42 maintains the correct relative geometry across all orbital planes as the constellation evolves.

**Attitude dynamics** — full 6-DOF model per spacecraft: angular velocity, quaternion orientation, and reference frame transformations. Actuator torques (reaction wheels, magnetorquers, thrusters) feed back into the dynamics.

**Space environment:**

| Model | What it provides |
|---|---|
| EGM96 gravity (18th order) | Orbit perturbations at LEO altitude |
| IGRF magnetic field (10th order) | Magnetometer readings, magnetorquer control |
| MSIS-86 / Jacchia-Roberts atmosphere | Aerodynamic drag |
| Solar geometry | Sun position, eclipse/sunlight status per spacecraft |
| Celestial bodies (sun, 9 planets, 45 moons) | Third-body perturbations |

At each timestep, 42 publishes spacecraft state (position, velocity, attitude, sun vector, magnetic field, eclipse flag) to the hardware simulators via shared memory.

## Onboard Hardware

NOS3 provides hardware simulators that register on virtual buses and respond to the same register-level protocols as real devices. Each simulator reads the physical state it needs from 42.

**Attitude determination:**

- Coarse sun sensors (CSS) — sun vector for coarse attitude
- Fine sun sensors (FSS) — high-resolution sun vector
- Inertial measurement unit (IMU) — angular rate and acceleration
- Star tracker — attitude from star catalog matching
- Magnetometer — Earth's magnetic field vector

**Navigation:**

- GPS receiver (NovAtel OEM615) — position and velocity. Used by [SpaceCoMP workflows](/spacecomp/use-cases/overview) to detect when the satellite is over an area of interest.

**Power:**

- Electrical power system (EPS) — battery state (voltage, temperature, charge), solar array output (driven by 42's sun vector and eclipse status)

**Attitude control:**

- Reaction wheels (3 units) — momentum storage, torques fed back to 42
- Magnetorquers — desaturation torques against the magnetic field
- Thrusters — orbit and attitude maneuvers

**Imaging:**

- Arducam (OV5640) — visible-light camera
- Thermal camera — serves synthetic brightness temperature frames for Earth observation workflows (see [Earth observation data](#earth-observation-data) below)

## Communication

The generic radio simulator replaces the physical RF transceiver with UDP sockets. Two link types are modeled:

- **Ground link** — uplink and downlink between a satellite and a ground station, each a UDP socket pair
- **Inter-satellite link** — proximity radio connecting neighboring satellites in the [2D torus](/spacecomp/constellation). The [ISL router](/protocols/network/routing) forwards packets across multiple hops; [SRSPP](/protocols/transport/srspp) provides reliable delivery.

The simulation validates the full protocol stack — framing, routing, reliability, security — but does not model the physical RF channel. Packets arrive instantly with no loss. The following are not simulated:

- Propagation delay (real ISL: 1–5 ms)
- Line-of-sight gating (Earth occultation)
- Bandwidth constraints
- Path loss and link margin
- Interference and noise

RF performance must be validated separately with link analysis tools or hardware-in-the-loop testing.

## Earth Observation Data

The NOS3 sensor suite covers attitude and navigation but not Earth observation payloads (SAR, thermal IR, multispectral). Three strategies inject synthetic observation data for [workflow testing](/spacecomp/use-cases/overview):

**Parametric** — no images generated. The simulator injects anomaly descriptors (coordinates + values) directly into the workflow's map phase. Tests workflow logic without image processing cost.

**Synthetic raster** — pre-generated sensor images loaded from disk when the satellite passes the AOI. The `eosim` tool generates thermal IR (brightness temperature with fire injection), SAR (displacement phase screens, dark spots), and multispectral (NDVI with deforestation polygons) rasters. The thermal camera simulator serves these over a virtual SPI bus.

**Real data replay** — archived Sentinel-1 (SAR), MODIS/VIIRS (thermal), or Landsat/Sentinel-2 (multispectral) data replayed through the pipeline for ground-truth validation.

| Strategy | What it tests | When to use |
|---|---|---|
| Parametric | Workflow logic, alert routing, state persistence | Integration tests, CI |
| Synthetic raster | Full pipeline including image processing | Algorithm development |
| Real data replay | Detection accuracy against ground truth | Pre-deployment validation |

## Limitations

| Domain | What is not modeled |
|---|---|
| RF channel | Propagation delay, LOS gating, bandwidth, path loss, interference |
| Radiation | Single-event upsets, total ionizing dose, latchups |
| Thermal | Dynamic spacecraft thermal model (EPS uses fixed temperatures) |
| Modulation | BPSK/QPSK demodulation (handled by real radio hardware) |
