# Overview

LeoDOS uses NASA's NOS3 (NASA Operational Simulator for Small Satellites) to simulate a full satellite constellation on a development machine. The same cFS flight software that runs in simulation also runs on the flight processor — only the hardware abstraction layer changes.

The simulation environment models five categories of spacecraft behavior:

- [Orbital Mechanics](orbital-mechanics) — orbit propagation, attitude dynamics, and the space environment
- [Sensors and Actuators](sensors) — attitude determination, navigation, power, propulsion, and imaging
- [Communication](communication) — ground links, inter-satellite links, and hardware bus emulation
- [Earth Observation](earth-observation) — synthetic sensor data for workflow testing
- [Infrastructure](infrastructure) — Docker setup, NOS Engine transport, and build workflow
