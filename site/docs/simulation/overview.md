# Overview

The LeoDOS simulator runs a full constellation — flight software, communication stack, and sensor payloads — on a development machine using NASA's NOS3 (NASA Operational Simulator for Small Satellites) framework. Each simulated satellite runs real cFS apps inside a Docker container, connected to hardware simulators through the NOS Engine transport layer. The same Rust binary that runs in simulation also runs on the flight processor — only the [PSP](/cfs/psp) changes.

Simulating a 100-satellite constellation at full fidelity would require 100 instances of cFS, each with its own sensor suite. This is impractical on a single machine. LeoDOS uses a tiered fidelity model that runs full simulation on a handful of satellites and lightweight proxies for the rest, keeping resource usage manageable while preserving realistic network behavior.

- [Architecture](architecture) — NOS3 components, how cFS apps connect to simulated hardware
- [Tiered Fidelity](tiered-fidelity) — Full, Lite, and Ghost node tiers for scalable constellation simulation
- [Sensor Simulation](sensor-simulation) — synthetic sensor data generation and injection for workflow testing
