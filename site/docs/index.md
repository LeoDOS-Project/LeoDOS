---
slug: /
title: LeoDOS
---

# LeoDOS

LeoDOS is a communication stack and distributed computing platform for LEO satellite constellations. It runs on NASA's [Core Flight System](/cfs/overview) and implements [CCSDS protocols](/protocols/overview) for reliable multi-hop communication over inter-satellite links.

The system is designed around a single premise: satellites in a LEO constellation generate far more data than they can downlink. Rather than storing raw data and waiting for a ground pass, satellites process data onboard, route it across the mesh network, and deliver compact results to ground. The [SpaceCoMP](/spacecomp/overview) framework coordinates this distributed computation; the [protocol stack](/protocols/overview) provides the reliable transport it runs on.

## Documentation

- **[Background](/background/overview)** — orbits, constellations, links, and the space environment
- **[Building and Running](/building/overview)** — how to build, test, and run LeoDOS
- **[SpaceCoMP](/spacecomp/overview)** — distributed computation across the constellation: task allocation, workflows, and use cases
- **[LEO Communication Protocols](/protocols/overview)** — CCSDS communication stack from physical modulation to reliable transport
- **[Core Flight System](/cfs/overview)** — the cFS framework: architecture, mission structure, and the five cFE services
- **[Research](/research/overview)** — open problems in data stream processing and security for space systems
- **[Simulation](/simulation/overview)** — NOS3-based constellation simulator: orbits, sensors, communication, Earth observation
- **[ColonyOS](/colonyos/integration)** — external job orchestration for ground-initiated workflows
