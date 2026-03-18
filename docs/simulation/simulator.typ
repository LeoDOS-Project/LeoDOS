#set page(paper: "a4", margin: 2.5cm)
#set text(font: "Helvetica Neue", size: 11pt)
#set heading(numbering: "1.1")
#set par(justify: true)

= LeoDOS Constellation Simulator

== Motivation

Existing open-source space simulators fall into two categories.
_Single-spacecraft simulators_ like NASA's NOS3 provide high-fidelity
dynamics, hardware-in-the-loop testing, and flight software execution,
but have only recently begun adding multi-satellite support (NOS3 has
experimental N-spacecraft capability on development branches, tested
with three spacecraft, though this has not been released). _Constellation simulators_ like
PASEOS model multi-satellite environments with orbital mechanics and
environmental constraints, but run abstract activities rather than real
flight software.

Neither category supports running real flight software on multiple
spacecraft within a physics-aware constellation. LeoDOS fills this gap
by combining the depth of NOS3 (cFS integration, hardware simulation,
6-DOF dynamics via 42) with the breadth of PASEOS (constellation-scale
orbital mechanics, environmental models, communication windows) --- in
Rust, with real inter-satellite networking.

== Existing Simulators

=== NOS3

The NASA Operational Simulator for Space Systems is a suite of tools
developed by NASA's Katherine Johnson Independent Verification and
Validation Facility. It provides a complete environment for flight
software development, integration and test, and mission operations
training.

NOS3 centres on a single spacecraft. The dynamics engine is _42_, a
6-DOF orbital and attitude simulator from NASA Goddard that computes
position, velocity, attitude, angular rates, sun vectors, and magnetic
field vectors. Twelve component simulators (GPS, IMU, sun sensors, star
tracker, magnetometer, reaction wheels, torquers, thrusters, EPS,
radio) connect to 42 over TCP and convert truth data into realistic
sensor readings. These readings are presented to the flight software
through NOS Engine, a C++ middleware that synchronises time across all
simulators and routes data over a shared bus.

The flight software is NASA's core Flight System (cFS), written in C.
The ground station interface is COSMOS. The entire stack runs inside a
Vagrant virtual machine.

NOS3's strengths are fidelity and closed-loop testing: the flight
software reads from simulated sensors, computes control outputs, and
commands simulated actuators, all against accurate dynamics.
Multi-satellite support exists as a proof-of-concept on development
branches: a three-spacecraft configuration in Earth-Moon NRHO orbit
has been demonstrated, with independent cFS instances per spacecraft,
a shared 42 dynamics instance, and a single NOS Time Driver
synchronising all stacks. This capability has not yet been merged to
the main release branch. NOS3's limitations are the nascent state of
multi-satellite support, the absence of physics-aware inter-satellite
networking (the proof-of-concept uses static Docker network aliases),
language (C/C++), and deployment complexity (Vagrant provisioning).

#figure(
  table(
    columns: (1.5fr, 3fr),
    stroke: none,
    inset: 5pt,
    fill: (_, y) => if calc.rem(y, 2) == 0 { luma(240) } else { white },
    [*Dynamics*], [NASA 42 --- 6-DOF orbit + attitude],
    [*Flight software*], [cFS (C)],
    [*Hardware models*], [12+ components via NOS Engine],
    [*Sensor chain*], [42 #sym.arrow sim #sym.arrow NOS Engine #sym.arrow cFS],
    [*Protocols*], [Basic CCSDS via cFS],
    [*Networking*], [Intra-spacecraft bus only],
    [*Constellation*], [N s/c (dev branch)],
    [*Ground station*], [COSMOS operator interface],
    [*Language*], [C / C++],
    [*Deployment*], [Vagrant VM],
  ),
  caption: [NOS3 characteristics],
)

==== 42 --- The Mostly Harmless Simulator

42 is a general-purpose, multi-body spacecraft simulation developed at
NASA Goddard Space Flight Centre. It computes the orbital and attitude
state of a spacecraft and the surrounding environment with sufficient
fidelity for guidance, navigation, and control (GNC) validation.

Orbital dynamics support two-body and three-body propagation with an
EGM96 gravity model up to 18th degree and order. Atmospheric drag
uses MSIS-86 and Jacchia-Roberts density models. Solar radiation
pressure is modelled. Planetary ephemerides use Meeus algorithms ---
adequate for GNC validation but not intended for mission planning.

Attitude dynamics are full rotational dynamics with configurable
actuators: three reaction wheels, three magnetic torque bars, and four
thrusters. The spacecraft is modelled as a multi-body system with
mass and inertia tensors per body.

The environmental model includes the planetary magnetic field (IGRF
to 10th order for Earth), sun position and eclipse detection, and
atmospheric density at the spacecraft altitude.

Sensor models produce realistic outputs with configurable noise, bias,
and quantization for gyroscopes, accelerometers, coarse and fine sun
sensors, star trackers, magnetometers, and GPS receivers.

In NOS3, 42 runs as a standalone process. Each component simulator
connects to 42 on an assigned TCP port and reads truth data at each
simulation timestep:

#figure(
  table(
    columns: (1fr, 2.5fr, 2fr),
    stroke: none,
    inset: 5pt,
    fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
    table.header(
      text(weight: "bold")[Port],
      text(weight: "bold")[Simulator],
      text(weight: "bold")[Data],
    ),
    [4245], [GPS], [Position, velocity],
    [4281], [IMU], [Gyro, accelerometer],
    [4227], [CSS], [Sun vector (coarse)],
    [4284], [FSS], [Sun vector (fine)],
    [4282], [Star tracker], [Attitude quaternion],
    [4234], [Magnetometer], [Magnetic field vector],
    [4277], [Reaction wheels], [Angular momentum],
    [4279], [Torquers], [Magnetic dipole],
    [4280], [Thrusters], [Thrust vector],
    [4283], [EPS], [Orbit/sun state],
    [4286], [Radio], [Position],
  ),
  caption: [42 TCP ports and component simulators],
)

=== PASEOS

PASEOS (PAseos Simulates the Environment for Operating multiple
Spacecraft) is a Python library developed by
#sym.Phi\-lab\@Sweden, a collaboration between AI Sweden and the
European Space Agency. It simulates the space environment for
operating multiple spacecraft with emphasis on onboard and operational
constraints.

PASEOS is fully decentralised: one instance runs per spacecraft. Each
instance manages _activities_ --- user-provided Python functions that
model arbitrary computation --- under simulated constraints. The
constraints include power budgets (solar panel charging, battery
discharge), thermal effects (single-node radiative equilibrium),
radiation (stochastic Poisson processes for single-event upsets,
restarts, and latch-ups), and communication windows (line-of-sight
with bandwidth modelling).

Orbital mechanics use PyKEP for Keplerian propagation and TLE/SGP4
support. Ground station positioning uses Skyfield for Earth rotation
and altitude angle calculations. Line-of-sight checks support both
spherical and mesh-based central body occlusion.

PASEOS's strengths are simplicity, Pythonic ergonomics, and
constellation-scale operation. Its limitations are the absence of
flight software execution (activities are abstract Python coroutines,
not real onboard code), no attitude dynamics, no hardware models, and
no real networking (communication is modelled as bandwidth windows,
not actual packet delivery).

#figure(
  table(
    columns: (1.5fr, 3fr),
    stroke: none,
    inset: 5pt,
    fill: (_, y) => if calc.rem(y, 2) == 0 { luma(240) } else { white },
    [*Dynamics*], [SGP4 / Keplerian (PyKEP)],
    [*Flight software*], [None --- abstract Python activities],
    [*Hardware models*], [None],
    [*Power*], [Simple linear charge / discharge],
    [*Thermal*], [Single-node radiative equilibrium],
    [*Radiation*], [Poisson process (abstract SEU/SEL)],
    [*Protocols*], [None],
    [*Networking*], [Modelled bandwidth windows],
    [*Constellation*], [Yes --- N spacecraft (lightweight)],
    [*Ground station*], [Positional actor with elevation mask],
    [*Language*], [Python],
    [*Deployment*], [`pip install paseos`],
  ),
  caption: [PASEOS characteristics],
)

=== StarryNet

StarryNet is an emulator for satellite Internet constellations. It
creates Docker containers for each satellite and ground station, then
establishes real network links between them with delay, bandwidth, and
loss derived from SGP4 orbital positions. OSPF runs as the intra-AS
routing protocol.

StarryNet's strength is real network emulation: actual TCP/IP traffic
flows between containers with physically accurate link conditions. It
supports fault injection (random link damage at a configurable ratio)
and recovery. Its limitations are the absence of any onboard
computation model (containers are network nodes, not spacecraft), no
environmental modelling (no power, thermal, or radiation), and
infrastructure requirements (Docker on CentOS/Ubuntu).

StarryNet fills a networking research niche --- testing routing
protocols, measuring end-to-end latency, and evaluating fault
tolerance in satellite networks --- but does not model what happens
_on_ the spacecraft.

=== Celestial

Celestial is an emulator for the LEO edge developed at TU Berlin.
Each satellite and ground station runs as a Firecracker microVM with a
custom kernel and filesystem. Network conditions between VMs are
modified in real time based on orbital geometry. A configurable
bounding box on Earth's surface determines which satellites are booted,
so only the portion of the constellation relevant to the experiment
consumes resources.

Celestial's strength is running _real applications_ (video
conferencing, databases) on emulated LEO infrastructure with realistic
network conditions. Its limitations are similar to StarryNet: no
onboard environmental modelling, no flight software integration, and
heavy infrastructure requirements (root access, dedicated servers,
Firecracker).

=== Summary

#figure(
  table(
    columns: (1.3fr, 1.3fr, 1.3fr, 1.3fr, 1.3fr),
    stroke: none,
    inset: 4pt,
    fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
    table.header(
      [],
      text(weight: "bold", size: 9pt)[NOS3],
      text(weight: "bold", size: 9pt)[PASEOS],
      text(weight: "bold", size: 9pt)[StarryNet],
      text(weight: "bold", size: 9pt)[Celestial],
    ),
    [*Dynamics*], [42 (6-DOF)], [SGP4/Kepler], [SGP4], [TLE],
    [*Flight SW*], [cFS (C)], [None], [None], [None],
    [*HW models*], [12+], [None], [None], [None],
    [*Attitude*], [Full], [None], [None], [None],
    [*Power*], [Via 42], [Linear], [None], [None],
    [*Thermal*], [Via 42], [Radiative], [None], [None],
    [*Radiation*], [None], [Poisson], [None], [None],
    [*Networking*], [Bus only], [Modelled], [Real (Docker)], [Real (microVM)],
    [*Scale*], [N s/c], [N s/c], [N s/c], [N s/c],
    [*Language*], [C/C++], [Python], [Python], [Go],
  ),
  caption: [Comparison of open-source space simulators],
)

The table reveals the gap: NOS3 provides single-spacecraft depth
(flight software, hardware, dynamics) while PASEOS, StarryNet, and
Celestial provide constellation breadth (multiple nodes, orbital
mechanics, networking). No existing tool provides both.

#pagebreak()

== LeoDOS Simulator Architecture

The LeoDOS simulator combines the vertical depth of NOS3 with the
horizontal breadth of PASEOS by composing four subsystems: the
constellation engine (walker-delta), the flight software framework
(leodos-libcfs), the communication stack (leodos-protocols), and the
NOS3 integration layer.

=== Constellation Engine: walker-delta

The walker-delta crate provides constellation-scale orbital mechanics
and visualisation. It serves the role that PASEOS's simulation core
and 42's dynamics engine serve in their respective systems --- but
across multiple spacecraft simultaneously.

==== Orbital Mechanics

Walker-delta generates satellite positions for Walker Delta and Walker
Star constellation patterns. Each satellite's state is propagated
using Keplerian elements with J2 perturbation:

The RAAN drift rate due to Earth's oblateness is:

$ dot(Omega) = -3/2 J_2 (R_"eq" / a)^2 n cos i $

where $J_2$ is the second zonal harmonic, $R_"eq"$ is the equatorial
radius, $a$ is the semi-major axis, $n$ is the mean motion, and $i$
is the inclination. This effect is critical for sun-synchronous
orbits and must be modelled for realistic constellation geometry over
time.

Position propagation solves Kepler's equation iteratively (Newton-
Raphson, 10 iterations) to convert mean anomaly to eccentric anomaly,
then transforms through the standard perifocal-to-inertial rotation.

For real satellite tracking, walker-delta integrates the `sgp4` crate
to propagate TLE element sets from CelesTrak, supporting 51 preset
satellite groups including Starlink, OneWeb, Iridium, Kuiper, GPS,
Galileo, GLONASS, Beidou, and debris catalogues.

==== Environmental Models

Walker-delta includes environmental models that exceed PASEOS in
fidelity:

- *Magnetic field*: International Geomagnetic Reference Field (IGRF)
  with 104 Gauss and Schmidt coefficients to degree 13, evaluated on
  a 91 #sym.times 181 grid with bilinear interpolation. PASEOS has no
  magnetic field model.

- *Radiation*: NASA AEP8/AP8 trapped particle model for the Van Allen
  belts, modelling both electron (AE8) and proton (AP8) fluxes across
  solar minimum and maximum conditions. PASEOS uses an abstract
  Poisson process with a user-specified event rate; walker-delta
  provides physically grounded flux values as a function of L-shell
  and magnetic field magnitude.

- *Debris and collisions*: Kessler syndrome modelling with debris
  fragment generation from satellite collisions. Fragments are
  assigned orbital elements derived from the collision geometry and
  ejection velocity distribution. PASEOS and NOS3 have no debris
  model.

- *Celestial bodies*: Physical and orbital properties of 28 bodies
  (8 planets, 12 moons, dwarf planets, Sun) with gravitational
  parameters, J2 coefficients, rotation periods, and flattening.
  Constellations can be placed around any body.

==== Inter-Satellite Links

Each satellite maintains links to neighbours in the constellation
grid. Link topology is configurable:

- *Plane-to-plane*: links to satellites in adjacent orbital planes
  ($plus.minus 1$ to $plus.minus N$ planes).
- *Intra-plane*: links to satellites along the same orbit track
  ($plus.minus 1$ to $plus.minus M$ slots).

For Walker Star constellations and partial-coverage configurations,
edge wrapping is disabled to prevent cross-seam links that would
traverse the entire constellation.

==== Additional Capabilities

- *Ground pass prediction*: elevation angle computation and AOS/LOS
  time prediction for ground station contacts.
- *Conjunction detection*: real-time spatial hashing for O(N)
  proximity detection and future conjunction prediction with adaptive
  time stepping.
- *Hohmann transfers*: interplanetary transfer orbit computation with
  launch window prediction.
- *Visualisation*: GPU-accelerated 3D globe rendering (egui/wgpu)
  with textured planets, ground tracks, ISL links, radiation belts,
  and multiple map projections.

=== Flight Software: leodos-libcfs

The leodos-libcfs crate provides safe, zero-cost Rust abstractions
over NASA's core Flight System APIs. It serves the same role as cFS
in NOS3 --- the flight software framework --- but in Rust.

==== cFE Bindings

The bindings cover the three cFE service layers:

- *Executive Services (ES)*: application lifecycle, task management,
  critical data store, memory pools, performance counters.
- *Software Bus (SB)*: RAII pipe handles, zero-copy send buffers,
  message ID routing.
- *Event Services (EVS)*: event registration and logging.
- *Table Services (TBL)*: static configuration tables.
- *Time Services*: mission elapsed time, UTC correlation.

==== OSAL Bindings

Operating System Abstraction Layer bindings provide platform-
independent access to tasks, queues, timers, mutexes, semaphores,
file systems, and network sockets.

==== NOS3 Hardware Bindings

Safe Rust wrappers for 13 NOS3 component simulators:

#figure(
  table(
    columns: (1.8fr, 1fr, 2.5fr),
    stroke: none,
    inset: 4pt,
    fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
    table.header(
      text(weight: "bold")[Component],
      text(weight: "bold")[Bus],
      text(weight: "bold")[Data],
    ),
    [ADCS], [I2C/UART], [Attitude quaternion, angular rates],
    [EPS], [I2C], [Battery voltage, current, state of charge],
    [IMU], [SPI/I2C], [Gyroscope, accelerometer],
    [Radio], [UART], [RF transceiver control],
    [Reaction wheel], [I2C], [Speed command, momentum readback],
    [Torquer], [I2C], [Magnetic dipole command],
    [Thruster], [GPIO/I2C], [Thrust command, valve state],
    [Star tracker], [UART], [Attitude quaternion, star catalogue],
    [Fine sun sensor], [I2C], [Sun vector (high precision)],
    [Coarse sun sensor], [I2C], [Sun vector (low precision)],
    [GPS (NovAtel)], [UART], [Position, velocity, time],
    [Magnetometer], [I2C], [Magnetic field vector],
    [Camera], [SPI], [Image capture],
  ),
  caption: [NOS3 component simulators with Rust wrappers],
)

==== NOS Engine Integration

The hardware abstraction layer (hwlib) connects to NOS Engine over
TCP. In simulation mode, each UART, I2C, and SPI handle maps to a
named NOS Engine bus via a hardcoded connection table:

```
Rust (UartChannel)
  -> leodos-libcfs UART wrapper
    -> hwlib C API (uart_read/write_port)
      -> NOS Engine SDK (NE_uart_open3)
        -> TCP to nos-engine-server:12000
          -> Component simulators
            -> 42 truth data
```

The same Rust flight software binary runs on real hardware (where
hwlib talks to physical device files) or in simulation (where hwlib
talks to NOS Engine over TCP) without recompilation. Only the linked
hwlib variant changes.

==== Async Runtime

LeoDOS includes a custom single-threaded async executor integrated
with the cFS scheduler. The executor polls a single future per cFS
cycle, providing zero-cost `async`/`await` in a `no_std` environment.
Synchronisation primitives (SPSC channels, oneshot channels) enable
concurrent tasks without heap allocation.

=== Communication Stack: leodos-protocols

The leodos-protocols crate implements the full CCSDS protocol stack
from physical layer to application layer. This is the subsystem that
has no equivalent in either NOS3 or PASEOS.

The stack is described in detail in the companion document
(_LeoDOS Communication Stack_). In the context of the simulator,
the key capability is real inter-satellite networking: packets
traverse the ISL mesh through routers at each node, with SRSPP
providing end-to-end reliable delivery and COP-1 providing per-hop
frame recovery.

=== Deployment

The NOS3 integration runs in Docker:

```
make nos3-build       # build Docker image with Rust FSW
make nos3-config      # configure NOS3
make nos3-build-sim   # compile C++ component simulators
make nos3-build-fsw   # compile cFS + Rust flight software
make nos3-launch      # start all containers
```

The `docker-compose.nos3.yml` defines four services: NOS Engine
server, 42 dynamics simulator, component simulators, and the Rust
flight software. All communicate over a Docker network.

#pagebreak()

== Comparison with Existing Simulators

#figure(
  table(
    columns: (1.5fr, 1.3fr, 1.3fr, 1.5fr),
    stroke: none,
    inset: 4pt,
    fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
    table.header(
      text(weight: "bold")[Capability],
      text(weight: "bold", size: 9pt)[NOS3],
      text(weight: "bold", size: 9pt)[PASEOS],
      text(weight: "bold", size: 9pt)[LeoDOS],
    ),
    [*Dynamics*], [42 (6-DOF)], [SGP4/Kepler], [42 + Kepler/J2/SGP4],
    [*Flight software*], [cFS (C)], [None], [cFS (Rust)],
    [*Hardware sims*], [12+ (C++)], [None], [13 (Rust wrappers)],
    [*Attitude*], [Full], [None], [Full (via 42)],
    [*Sensors*], [Full chain], [None], [Full chain],
    [*Power*], [Via 42/EPS], [Linear model], [Via NOS3 EPS],
    [*Thermal*], [Via 42], [Radiative eq.], [Via 42],
    [*Radiation*], [None], [Poisson], [AEP8/AP8 + IGRF],
    [*Magnetic field*], [Via 42 (IGRF)], [None], [IGRF (degree 13)],
    [*Protocols*], [Basic CCSDS], [None], [Full CCSDS stack],
    [*Networking*], [Bus only], [Modelled], [Real (SRSPP + ISL)],
    [*Constellation*], [Yes (dev branch)], [Yes], [Yes],
    [*Distributed compute*], [No], [Activities], [SpaceCoMP],
    [*WASM deployment*], [No], [No], [Yes (WAMR)],
    [*Ground station*], [COSMOS], [Actor], [Pass prediction],
    [*Visualisation*], [COSMOS], [matplotlib], [GPU 3D globe],
    [*Language*], [C/C++], [Python], [Rust],
    [*Deployment*], [Vagrant VM], [`pip install`], [Docker + native],
  ),
  caption: [Detailed comparison of NOS3, PASEOS, and LeoDOS],
)

=== What LeoDOS takes from NOS3

LeoDOS preserves NOS3's vertical integration: the same sensor chain
(42 #sym.arrow component simulator #sym.arrow NOS Engine #sym.arrow
flight software) runs unmodified. The Rust wrappers are safe
abstractions over the existing C/C++ interfaces, not reimplementations
of the simulator logic. 42 remains the dynamics truth source.

NOS3 has experimental multi-satellite support on development branches
(`435-42-support-multiple-satellites` and
`802-constellation-scenario-with-lunar-focus`). The proof-of-concept
demonstrates three spacecraft in an Earth-Moon NRHO orbit with
independent cFS instances, each commanded via COSMOS. LeoDOS's
multi-node architecture builds on this work (see @multi-node).

The key departures from NOS3 are:

- *Language*: Rust instead of C for flight software, providing memory
  safety, type safety, and zero-cost abstractions without sacrificing
  performance.
- *Deployment*: Docker instead of Vagrant, reducing setup friction.
- *Constellation networking*: NOS3's multi-satellite proof-of-concept
  uses a daisy-chain radio cross-link between adjacent spacecraft.
  LeoDOS replaces this with a full ISL mesh using SRSPP and
  physics-gated routing.

=== What LeoDOS takes from PASEOS

LeoDOS reproduces PASEOS's constellation-level capabilities through
walker-delta: orbital propagation, line-of-sight computation,
communication window prediction, and environmental modelling. In
several areas it exceeds PASEOS:

- *Radiation*: NASA AEP8/AP8 trapped particle flux vs. abstract
  Poisson process.
- *Magnetic field*: IGRF degree-13 model vs. none.
- *Communication*: Real packet delivery via SRSPP and ISL routing
  vs. modelled bandwidth windows.
- *Computation*: SpaceCoMP MapReduce with Hungarian/LAPJV assignment
  vs. abstract Python activities.

The key departure from PASEOS is that LeoDOS runs real flight
software, not abstract activities. Where PASEOS models an activity as
a Python coroutine with a power consumption parameter, LeoDOS
executes a cFS application that reads from simulated sensors, computes
outputs, and communicates over real network protocols.

=== What is new

Three capabilities have no equivalent in any of the compared systems:

+ *Real inter-satellite networking*: Packets traverse the
  constellation mesh using the ISL router with distance-minimising or
  Manhattan routing. SRSPP provides end-to-end reliable delivery.
  COP-1 provides per-hop frame recovery. No existing simulator
  combines real packet delivery with flight software execution.

+ *Distributed computing framework*: SpaceCoMP coordinates MapReduce
  jobs across the constellation with optimal role assignment
  (Hungarian or LAPJV algorithm). The data flows through the real
  protocol stack, not simulated bandwidth.

+ *On-orbit code deployment*: The WASM runtime (leodos-libwamr)
  enables deploying new computation to satellites at runtime, without
  recompiling or relinking the flight software.

#pagebreak()

== Multi-Node Constellation Simulation <multi-node>

The current system runs a single spacecraft against the full NOS3
stack. NOS3's development branches demonstrate that multi-satellite
simulation is architecturally supported. LeoDOS extends this with
constellation-scale networking and tiered fidelity.

=== NOS3 Multi-Satellite Architecture

NOS3's proof-of-concept (branch `435-42-support-multiple-satellites`)
establishes the multi-satellite pattern. Three design decisions are
central:

==== Single 42 instance, multiple spacecraft

42 natively supports multiple spacecraft in a single process. The
simulation input file declares N spacecraft, each with its own
orbital elements, mass properties, and sensor configuration:

```
3                               ! Number of Spacecraft
TRUE  0 SC_Gateway_0.txt
TRUE  0 SC_Gateway_1.txt
TRUE  0 SC_Gateway_2.txt
```

All spacecraft are propagated together in a single integration step,
ensuring time consistency without external coordination. Each
spacecraft has independent dynamics: different positions, velocities,
attitudes, and environmental conditions.

==== Isolated Docker networks per spacecraft

Each spacecraft receives its own Docker network (`nos3-sc01`,
`nos3-sc02`, etc.) containing its NOS Engine server, component
simulators (truth42sim, GPS, IMU, CSS, FSS, EPS, magnetometer,
three reaction wheels, radio, star tracker, thruster, torquer), and
cFS flight software instance. A shared `nos3-core` network hosts the
ground station and common services.

```
nos3-core   (ground station, shared services)
nos3-sc01   (NOS Engine + 16 sims + cFS for SC 1)
nos3-sc02   (NOS Engine + 16 sims + cFS for SC 2)
nos3-sc03   (NOS Engine + 16 sims + cFS for SC 3)
```

Each spacecraft runs approximately 17 containers: one NOS Engine
server, one truth42sim bridge, 13 component simulators, one cFS
flight software instance, and one CryptoLib instance.

==== Global time driver

A single NOS Time Driver process connects to all spacecraft networks
simultaneously and drives simulation time for every NOS Engine server
in lockstep:

```
nos-time-driver --network nos3-core
                --network nos3-sc01
                --network nos3-sc02
                --network nos3-sc03
```

This solves the time synchronisation problem: 42 advances all
spacecraft dynamics together, and the time driver ensures all NOS
Engine instances and flight software processes share the same
simulation epoch.

==== Inter-spacecraft communication

The proof-of-concept implements a daisy-chain radio link. Each
spacecraft's radio simulator is connected to the previous spacecraft's
network under the alias `next-radio`:

```
docker network connect --alias "next-radio"
    nos3-sc01 sc02-radio-sim
docker network connect --alias "next-radio"
    nos3-sc02 sc03-radio-sim
```

This is a minimal proof-of-concept: each radio can reach the next
spacecraft's network, but there is no mesh routing, no physics-based
link gating, and no topology beyond a linear chain.

=== LeoDOS Extensions

LeoDOS builds on the NOS3 multi-satellite architecture with three
additions: constellation-scale networking, tiered fidelity, and a
physics-gated network fabric.

==== Constellation networking

NOS3's daisy-chain radio link is replaced by the full LeoDOS
communication stack. Each spacecraft runs the ISL router, which
maintains links to its neighbours in the constellation torus mesh.
SRSPP provides end-to-end reliable delivery across multiple hops.
COP-1 provides per-hop frame recovery. Gossip broadcast enables
constellation-wide dissemination.

The ISL topology is derived from walker-delta's constellation
geometry. As spacecraft move, the set of reachable neighbours changes
and the router adapts. This is fundamentally different from NOS3's
static `next-radio` alias, which does not change with orbital
geometry.

==== Tiered fidelity

42 is computationally expensive. Each full-tier spacecraft requires
approximately 17 Docker containers. For larger constellations, a
tiered fidelity model mixes full and reduced nodes:

#figure(
  table(
    columns: (1fr, 2fr, 1.5fr, 1fr),
    stroke: none,
    inset: 5pt,
    fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
    table.header(
      text(weight: "bold")[Tier],
      text(weight: "bold")[Dynamics],
      text(weight: "bold")[Flight SW],
      text(weight: "bold")[Scale],
    ),
    [Full], [42 (6-DOF, sensors, actuators)], [cFS + all HW sims], [5--10],
    [Lite], [walker-delta (Kepler + J2)], [cFS + basic HW sims], [10--50],
    [Ghost], [walker-delta (orbit only)], [None], [50--1000+],
  ),
  caption: [Tiered fidelity levels],
)

Full-tier nodes run the complete NOS3 stack as described above. All
full-tier spacecraft are declared in a single 42 instance, sharing
time synchronisation automatically. Lite-tier nodes run cFS with
simplified sensor models driven by walker-delta's propagator instead
of 42 --- they do not require a NOS Engine server or component
simulators, only a flight software container that reads orbital state
from walker-delta over a lightweight protocol. Ghost-tier nodes exist
only as orbital positions in walker-delta for constellation topology
and routing --- they participate in the ISL mesh but do not execute
flight software.

The tiered model allows studying a subset of spacecraft at full
fidelity (with 6-DOF dynamics, realistic sensors, and closed-loop
ADCS) while the surrounding constellation provides realistic network
topology and traffic at lower cost.

Time advancement differs by tier. Full-tier nodes advance with the
NOS Time Driver in lockstep with 42. Lite and ghost nodes use
walker-delta's analytical propagation, which can evaluate any time
instant directly without stepping. When the simulation advances by
$Delta t$, full nodes must integrate through every intermediate step
while lite and ghost nodes jump to the new time analytically.

==== Network fabric

The network fabric is a proxy that sits between flight software
instances and applies physics-based constraints to inter-node
communication. It replaces NOS3's static Docker network aliases with
dynamic, orbital-geometry-aware link management.

When a flight software instance transmits a packet via its ISL radio
(through the protocol stack: SPP #sym.arrow ISL router #sym.arrow
SRSPP #sym.arrow COP-1 #sym.arrow coding #sym.arrow UART), the
packet exits the Docker container as a UDP datagram. The network
fabric intercepts it and queries walker-delta for the link state
between the source and destination nodes:

- *Line of sight*: is the central body between the two nodes? If
  occluded, the packet is dropped (blackout).
- *Propagation delay*: the distance between nodes divided by the
  speed of light. Applied as a buffer delay before forwarding.
- *Bandwidth cap*: the configured link data rate. Excess packets are
  queued or dropped.
- *Bit error rate*: optionally derived from the radiation model.
  Errors are injected into the packet before forwarding, allowing the
  coding layer's FEC to be exercised.

```
fsw-1 --UDP--> network-fabric --UDP--> fsw-2
                    |
                    | queries
                    v
              walker-delta
              (LOS? distance? BER?)
```

The fabric does not interpret packet contents. It operates at the
UDP datagram level, preserving the full protocol stack within each
node. From the flight software's perspective, the fabric is
invisible --- packets simply experience the delay, loss, and
bandwidth constraints that the orbital geometry dictates.

The fabric handles communication between all tiers. Full-to-full,
full-to-lite, and lite-to-lite links all pass through the same proxy.
Ghost nodes forward packets through the fabric on behalf of the ISL
mesh without running flight software --- the fabric itself performs
the store-and-forward at those hops.

=== Deployment

The multi-node deployment composes three layers:

```
docker-compose.constellation.yml:

  # Dynamics (one instance, all full-tier SC)
  fortytwo:
    image: nos3-42

  # Per-spacecraft stacks (full tier)
  nos-engine-sc01:
    image: nos3
    network: nos3-sc01
  sims-sc01:
    depends_on: [nos-engine-sc01, fortytwo]
  fsw-sc01:
    depends_on: [nos-engine-sc01]

  nos-engine-sc02:
    image: nos3
    network: nos3-sc02
  ...

  # Global services
  nos-time-driver:
    networks: [nos3-core, nos3-sc01, nos3-sc02, ...]

  network-fabric:
    networks: [nos3-sc01, nos3-sc02, ...]

  walker-delta:
    networks: [nos3-core]
```

Walker-delta runs alongside the Docker ensemble, providing the
constellation engine for lite/ghost nodes and the physics oracle for
the network fabric. The NOS Time Driver coordinates simulation time
across all full-tier stacks. The network fabric bridges the per-SC
Docker networks with physics-aware packet forwarding.
