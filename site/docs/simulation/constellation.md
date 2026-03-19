# Constellation Simulation

The [simulation overview](overview) shows a single satellite. A constellation simulation runs many satellites simultaneously, each with its own flight software instance, connected by inter-satellite links across a Docker network.

## How It Works

A constellation simulation has three components:

- **42** — a single instance propagates orbits for all spacecraft in the constellation. Each satellite gets its own orbital elements (inclination, RAAN, mean anomaly) derived from the [Walker Delta geometry](/spacecomp/constellation).
- **One Docker container per orbital plane** — each container runs multiple satellite processes. Satellites within the same orbit share a container; different orbits run in separate containers on a shared Docker bridge network.
- **Inter-satellite links over UDP** — radio simulators in different containers communicate via UDP across the Docker network. Each satellite has a unique set of ports for its ground link and ISL connections.

```
┌──────────────────────────────────────────────────────────┐
│                    Docker Network                         │
│                                                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
│  │  Orbit 0    │  │  Orbit 1    │  │  Orbit 2    │ ...  │
│  │             │  │             │  │             │      │
│  │ Sat 0  ←UDP→  Sat 0  ←UDP→  Sat 0       │      │
│  │ Sat 1       │  │ Sat 1       │  │ Sat 1       │      │
│  │ Sat 2       │  │ Sat 2       │  │ Sat 2       │      │
│  │  ...        │  │  ...        │  │  ...        │      │
│  └─────────────┘  └─────────────┘  └─────────────┘      │
│         ↕ UDP            ↕ UDP            ↕ UDP          │
│  ┌──────────────────────────────────────────────────┐    │
│  │              42 (all orbits)                      │    │
│  └──────────────────────────────────────────────────┘    │
│         ↕ UDP                                            │
│  ┌──────────────────────────────────────────────────┐    │
│  │           Ground Station                          │    │
│  └──────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────┘
```

Within each container, satellites run as separate cFS processes with unique spacecraft IDs. The ISL router in each satellite connects to its four torus neighbors (north, south, east, west) — intra-orbit neighbors are in the same container, cross-orbit neighbors are in adjacent containers.

## CLI

The `leodos-cli` tool orchestrates constellation simulation:

```
leodos-cli sim start 3 22     # start 3 orbits × 22 satellites = 66 satellites
leodos-cli sim stop           # stop all containers
leodos-cli sim shell 0        # open a shell in orbit-0 container
```

The `start` command generates a Docker Compose configuration, assigns spacecraft IDs and ports, and brings up all containers. Each satellite gets a unique ID derived from its grid position: `(orbit + 1) × 1000 + satellite_number`.

## Scaling

42 handles the orbital mechanics for the entire constellation in a single process — 10,000+ satellites have been propagated. The scaling bottleneck is the flight software: each satellite runs a full cFS instance with its own sensor simulators. A 66-satellite constellation (3 × 22) runs comfortably on a workstation; larger constellations require more memory and CPU.

For protocol-only testing without NOS3, the `leodos-protocols` demo crate runs all satellites as lightweight tokio tasks in a single process, connected by in-memory channels. This scales to thousands of nodes and is useful for testing routing and transport behavior across the full torus topology.
