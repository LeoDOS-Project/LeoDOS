# Sensors and Actuators

NOS3 provides hardware simulators for each onboard device. Each simulator reads the physical state it needs from [42](orbital-mechanics) and responds to the same register-level protocol as the real hardware.

## Attitude

42 models full 6-DOF attitude dynamics per spacecraft — angular velocity, quaternion orientation, and reference frame transformations. The attitude control loop is closed in simulation: sensors read the computed state, flight software runs the control algorithm, and actuator commands feed torques back into 42's dynamics.

### Sensors

- **Coarse sun sensors (CSS)** — sun vector for coarse attitude estimation
- **Fine sun sensors (FSS)** — high-resolution sun vector
- **Inertial measurement unit (IMU)** — angular rate and acceleration
- **Star tracker** — attitude from star catalog matching (most accurate)
- **Magnetometer** — Earth's magnetic field vector

## Navigation

- **GPS receiver (NovAtel OEM615)** — position and velocity. Used by [SpaceCoMP workflows](/spacecomp/use-cases/overview) to detect when the satellite is over an area of interest.

## Power

- **Electrical power system (EPS)** — battery state (voltage, temperature, charge), solar array output driven by sun vector and eclipse status

### Actuators

- **Reaction wheels** (3 units) — momentum storage, torques fed back to 42
- **Magnetorquers** — desaturation torques against the magnetic field
- **Thrusters** — orbit and attitude maneuvers

## Imaging

- **Arducam (OV5640)** — visible-light camera
- **Thermal camera** — custom component for thermal IR Earth observation, serves synthetic frames from [eosim](earth-observation)

## Not Modeled

- Radiation effects (SEUs, total ionizing dose, latchups)
- Dynamic spacecraft thermal model (EPS uses fixed temperatures)
