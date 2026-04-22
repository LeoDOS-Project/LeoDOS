#import "@preview/polylux:0.4.0": *
#import "@preview/fletcher:0.5.8" as fletcher: diagram as fdiagram, node, edge

// Toggle incremental slide transitions (#show: later, #uncover)
#let TRANSITIONS = false
#let later = if TRANSITIONS { later } else { x => x }
#let uncover = if TRANSITIONS { uncover } else { (_, body) => body }

#set page(paper: "presentation-16-9")
#set text(font: "Helvetica Neue", size: 20pt)

#let bg = white
#let fg = black
#let dim = luma(120)

#set page(
  fill: bg,
  footer: context [
    #set text(size: 10pt, fill: dim)
    #h(1fr)
    #counter(page).display()
  ],
)
#set text(fill: fg)

#let fn(n) = super(text(size: 10pt, fill: dim, str(n)))

#slide[
  #align(center + horizon)[
    #text(size: 44pt, weight: "bold")[SpaceCoMP: A Primer]
    #v(12pt)
    #text(size: 22pt, fill: dim)[Space Collect-MapReduce Processing]
    #v(24pt)
    #text(size: 16pt, fill: dim)[Design, Optimizations, and Research Directions]
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Outline]
  #v(12pt)

  #text(size: 18pt)[
    + *The Problem: The Downlink Wall*
    + *The Opportunity: The Lightspeed Mesh*
    + *The Challenge: No Cloud Above the Clouds*
    + *Our Approach: LeoDOS*
    + *The Programming Model: SpaceCoMP* — with three optimizations
    + *Limitations?*
    + *Research Directions* — stream processing, security
    + *The Reality* — when is SpaceCoMP useful?
    + *Beyond SpaceCoMP* — cFS, CCSDS, SRSPP, NOS3
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[The Problem: The Downlink Wall]
  #v(12pt)

  #text(size: 17pt)[
    - A single LEO satellite generates *1–2 TB/day* of imagery
    - Ground contact lasts *5–15 minutes* per ~95 min orbit
    - RF downlinks are shared spectrum, weather-dependent, \<1 Gbps
    - Mega-constellations (1,000–10,000+ satellites) produce *petabytes per day*
    - Most collected data never reaches Earth
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[The Opportunity: The Lightspeed Mesh]
  #v(6pt)

  #grid(
    columns: (1.2fr, 1fr),
    gutter: 16pt,
    [
      #text(size: 15pt)[
        + Modern satellites carry optical inter-satellite links (ISLs) — 10–100+ Gbps, always-on, lightspeed in vacuum
        #v(4pt)
        + Four links per satellite form a *+Grid* mesh (2D-Torus) — Any satellite can reach any other
        #v(4pt)
        + Onboard compute capacity is improving — ARM-class processors, FPGAs, even GPUs
        #v(4pt)
        + Launch costs are falling — Mega-constellations of 1,000–10,000+ satellites are now viable
        #v(4pt)
        + Data can be *processed in orbit* and compressed before downlink — TB become MB
      ]
    ],
    [
    #h(50pt)
      #text(size: 13pt)[
        #fdiagram(
          node-stroke: 0.5pt,
          node-corner-radius: 3pt,
          spacing: (20pt, 20pt),
          node((0, 0), [A1], name: <s00>),
          node((1, 0), [A2], name: <s10>),
          node((2, 0), [A3], name: <s20>),
          node((0, 1), [B1], name: <s01>),
          node((1, 1), text(weight: "bold")[B2], name: <s11>, stroke: 1pt),
          node((2, 1), [B3], name: <s21>),
          node((0, 2), [C1], name: <s02>),
          node((1, 2), [C2], name: <s12>),
          node((2, 2), [C3], name: <s22>),
          edge(<s00>, <s10>, "-"),
          edge(<s10>, <s20>, "-"),
          edge(<s01>, <s11>, "-"),
          edge(<s11>, <s21>, "-"),
          edge(<s02>, <s12>, "-"),
          edge(<s12>, <s22>, "-"),
          edge(<s00>, <s01>, "-"),
          edge(<s01>, <s02>, "-"),
          edge(<s10>, <s11>, "-"),
          edge(<s11>, <s12>, "-"),
          edge(<s20>, <s21>, "-"),
          edge(<s21>, <s22>, "-"),
        )
      ]
      #v(2pt)
      #text(size: 13pt)[
      #h(50pt)
        3 orbits x 3 sats per orbit.
      ]
    ],
  )
]

#slide[
  #text(size: 28pt, weight: "bold")[The Challenge: No Cloud Above the Clouds]
  #v(50pt)

  #text(size: 17pt)[
    - *Orbital mechanics* — A satellite over an AOI or in LOS of a ground station won't be there for long; the system must be delay tolerant, plan ahead, and execute autonomously
    #v(6pt)
    - *Constrained resources* — Enough compute to process data, but not enough to waste on abstraction layers; all software must be purpose-built, no virtualization
    #v(6pt)
    - *Real-time operating system* — Flight-critical systems need deterministic timing; no Linux/Docker/Kubernetes/JVM
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Our Approach: LeoDOS]
  #v(12pt)

  #text(size: 16pt)[
    - *LeoDOS* is a framework that makes the constellation programmable as a *single computer*
    - Provides reliable transport, multi-hop routing, and a safe interface to satellite hardware
    - CCSDS#fn(1)-compliant — Interoperable with existing space infrastructure
    - Implemented in Rust on NASA cFS#fn(2)
  ]

  #v(4pt)
  #text(size: 8pt, fill: dim)[
    #fn(1) Consultative Committee for Space Data Systems
    · #fn(2) core Flight System
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[The Programming Model: SpaceCoMP]
  #v(10pt)

  #text(size: 16pt)[
    *SpaceCoMP* is a MapReduce-inspired framework for Earth observation built on LeoDOS.

    #v(8pt)
    A ground station submits a *job* targeting a geographic *Area of Interest* (AOI). \
    The nearest visible satellite (LOS node) coordinates three phases:

    #v(8pt)
    + *Collect* — Satellites over the AOI acquire sensor data. Data stays local.
    + *Map* — Nearby satellites process each partition (e.g., thermal thresholding, InSAR).
    + *Reduce* — One satellite aggregates map outputs into a compact result for downlink.

    #v(8pt)
    *Optimizations:* distance-optimized routing, bipartite map task allocation, center-of-AOI reduce placement.
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[SpaceCoMP Examples]
  #v(20pt)
  #text(size: 16pt)[
    #text(size: 14pt)[
      - *Wildfire detection*
        - *Collect* — Capture thermal images over the AOI
        - *Map* — Threshold pixels above a brightness temperature; output hotspot coordinates
        - *Reduce* — Cluster hotspots into fire perimeters; downlink the polygons
    ]
    #text(size: 14pt)[
      #v(1pt)
      - *Oil spill detection*
        - *Collect* — Capture SAR imagery of the ocean surface
        - *Map* — Detect dark patches (oil dampens capillary waves)
        - *Reduce* — Stitch partial results into a single spill polygon
    ]
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Optimization 1: Distance-Optimized Routing]
  #v(4pt)
  #uncover("1-")[#text(size: 14pt)[*Question:* How should data be routed across the mesh?]]
  #uncover("2-")[#text(size: 14pt)[#v(4pt) *Naive:* Route by shortest hop count, ignoring physical distance.]]
  #uncover("3-")[#text(size: 14pt)[
    #v(4pt)
    *Problem:*
    + Signal power decays with the square of distance (2x → 4x loss, 10x → 100x loss)
    + More power loss → lower signal-to-noise ratio → lower bitrate → longer transmission time
    + Inter-plane link distances vary up to *40%* over each orbit
    + With a limited power budget, it is hard to increase transmit power
  ]]
  #uncover("4-")[#text(size: 14pt)[
    #v(4pt)
    *Optimization:*
    + Defer cross-plane hops until links are shorter
    + Same hop count, *8–21% shorter total distance*
  ]]
]

#slide[
  #text(size: 28pt, weight: "bold")[Optimization 2: Bipartite Matching]
  #v(6pt)

  #text(size: 15pt)[*Question:* Which mapper should process each collector's data?]
  #show: later
  #text(size: 15pt)[#v(6pt) *Naive:* Each collector sends to its nearest mapper (greedy).]
  #show: later
  #text(size: 15pt)[
    #v(6pt)
    *Problem:*
    + Greedy assignments are rarely optimal.
    + The cost of an assignment depends on processing time, hop count, transmission time, data volume, ...
    + A mapper can get overloaded by being assigned to multiple collectors.
  ]
  #show: later
  #text(size: 15pt)[
    #v(6pt)
    *Optimization:*
    + Model as a bipartite matching problem.
    + Solve with the Hungarian algorithm — O(k³), globally optimal.
    + *61–79%* over random, *18–28%* over greedy.
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Optimization 3: Center-of-AOI Reduce Placement]
  #v(12pt)

  #text(size: 16pt)[*Question:* Where should the reducer run?]
  #show: later
  #text(size: 16pt)[#v(6pt) *Naive:* Reducer at the LOS node (closest to ground station).]
  #show: later
  #text(size: 16pt)[
    #v(6pt)
    *Problem:*
    + The LOS node can potentially be far away from the mappers.
    + Mapper outputs can be large.
  ]
  #show: later
  #text(size: 16pt)[
    #v(6pt)
    *Optimization:*
    + Place the reducer at the *geometric center* of the mappers to decrease transfer distance
    + Only the reducer needs to send data to the LOS node for downlink.
    + Reducing data generally also reduces its volume, which means less data needs to be routed to the LOS node.
    + *67–72% cost reduction* — Benefit grows with compression ratio
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Limitations?]
  #v(3pt)

  #text(size: 16pt)[
    - *No streaming* — No support for continuous queries (e.g., live monitoring)
    - *No workflows* — No support for multi-stage jobs (e.g., collect-map-reduce-join)
    - *No dynamic AOI* — AOI cannot change during job execution
    - *No handover* — Jobs fail when collectors leave AOI
    - *No shuffle* — Each collector sends to one mapper
    - *No combiners* — All mappers send to a single reducer
    - *No ground-station routing* — Routing is only between satellites
    - *No security* — Assumes all nodes are trusted
    #show: later
    #v(3pt)
    - ...
    - *No dynamic cost model* — Cannot model load and contention
    - *No fault tolerance* — Jobs fail when satellites fail
    - *No reliabile transport* — Jobs fail when messages are lost
    - *No concurrent jobs* — Only one job can run at a time
    // - *No heterogeneity* — All satellites must have the same capabilities
    // - *No MEO-LEO synergy* —
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Research: Data Stream Processing]

  #text(size: 16pt)[
    - *Problem 1*: Cloud-based streaming systems do not model orbital mechanics:
      - *Reactive dataflow* — Operators process data as it arrives. In space, the entire observation schedule is known in advance from orbital mechanics (unlocks optimisations).
      - *Time-based windows* — Windows are defined over time only. In space, orbital position couples time and location.
      - *Heuristic delays* — Network delays are estimated from observed data. In space, delays are computable from orbital mechanics (unlocks exact watermarks).
    - *Research Question*: Can a new stream processing model be designed around orbital mechanics?
  ]
  #show: later
  #text(size: 16pt)[
    #v(1pt)
    - *Problem 2*: Cloud-based streaming systems are built on infrastructure that doesn't exist in space:
      - *JVM* — Dynamic heap allocation and garbage collection pauses break real-time guarantees
      - *IP/TCP* — Designed for terrestrial networks, not delay-tolerant space links
      - *Kubernetes/Docker* — Requires Linux, while satellites run RTOS
    - *Research Question*: Can streaming queries be compiled to native code with static memory for LeoDOS?
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Stream Processing Examples]
  #v(8pt)

  #text(size: 14pt)[
    A continuous wildfire monitoring query in Aqua-style syntax:
  ]

  #v(6pt)
  #show raw.where(lang: "aqua"): it => {
    show regex("\b(from|val|where|group|over|compute|select|into|in)\b"): kw => text(weight: "bold", fill: rgb("#0066cc"), kw)
    it
  }
  ```aqua
  from frame in thermal_camera_frames();
  where frame.location in BoundingBox(lat = 55..70, lon = 10..25)
  val hotspots = analyze(frame)
  where count(hotspots) > 0
  select Alert(perimeter = cluster(hotspots))
  into ground_station(lat = 30, lon = -40);
  ```
]

#slide[
  #text(size: 28pt, weight: "bold")[Research: Security]

  #text(size: 15pt)[
    - *Problem 1*: Current space data link security (SDLS) only provides per-hop AES-GCM encryption:
      - *No E2E security* — Each hop decrypts and re-encrypts (symmetrically). A compromised intermediate node can read and forge packets.
    - *Question*: Can E2E security work across a multi-hop satellite mesh? Can it use post-quantum primitives that are efficient enough for satellite hardware?
  ]
  #show: later
  #text(size: 15pt)[
    #v(1pt)
    - *Problem 2*: Symmetric key management does not scale E2E for large constellations:
      - *Pre-shared keys* — O(n²) keys for symmetric encryption; impractical for thousands of satellites
      - *Key rotation* — Must happen over high-latency links with intermittent ground contact
    - *Question*: How do you build PKI for a constellation?
  ]
  #show: later
  #text(size: 15pt)[
    #v(1pt)
    - *Problem 3*: Satellites are physically unreachable — you can never inspect or service them:
      - *No code signing* — No verification that deployed apps are authentic
      - *Byzantine nodes* — A compromised sat can lie, not just crash. Current systems only detect crash faults
    - *Question*: How do you manage trust in nodes you can never physically access?
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[The Reality: When is SpaceCoMP Useful?]
  #v(12pt)

  #text(size: 16pt)[
    - *Need mega constellations* with 1,000-10,000+ satellites for SpaceCoMP to be useful.
      - To have *one collector* (ascending) over Sweden at any time, you need \~2,300 satellites.
      - IRIS² is planning 268 LEO satellites
  ]
  #show: later
  #text(size: 16pt)[
    #v(1pt)
    - The constellations that have planned 1,000+ sats (Starlink, Kuiper, Guowang, Qianfan, TeraWave):
      - Are proprietary and primarily used for communication, not Earth observation
      - Generally use \~53 degrees inclination, which leaves out Sweden
  ]
  #show: later
  #text(size: 16pt)[
    #v(1pt)
    - ... But:
      - Lower launch costs and rideshare opportunities are making mega constellations more viable.
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Beyond SpaceCoMP: The LeoDOS Platform]
  #v(12pt)

  #text(size: 16pt)[
    - SpaceCoMP is one application — LeoDOS is a general-purpose platform
  ]
  #show: later
  #text(size: 16pt)[
    #v(1pt)
    - LeoDOS provides (and extends):
      + *NASA cFS*: A flight software framework
      + *CCSDS*: A reliable and secure communication stack
      + *NOS3*: A simulation environment for testing
  ]
  #show: later
  #text(size: 16pt)[
    #v(1pt)
    - Potential applications beyond SpaceCoMP:
      - Replicated data storage across the constellation
      - Peer-to-peer software updates
      - Pub-sub data dissemination (e.g., weather alerts)
      - Distributed task queues / load balancing
      - MPC (Multi-Party Computation) across the mesh
      - ...?
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[NASA cFS: The App Model]
  #v(1pt)

  #text(size: 14pt)[- Any user-level code that runs on a CFS satellite is an *app*.]
  #show: later
  #text(size: 14pt)[
    #v(1pt)
    - Apps are *shared libraries* (`.so`) loaded at runtime via `dlopen`
      - All apps share the same address space — No MMU on most flight processors
      - No heap allocation — All memory is bounded and known at build time
      - *Examples*: ISL router app, SpaceCoMP wildfire app.
  ]
  #show: later
  #text(size: 14pt)[
    #v(1pt)
    - Apps *communicate* internally within a satellite via:
      - *Software Bus*: A publish-subscribe zero-copy message bus
  ]
  #show: later
  #text(size: 14pt)[
    #v(1pt)
    - Apps can be *updated* at runtime using:
      - *Dynamic linking*: Apps can be uploaded, replaced, and added without restarting other apps
      - *Tables*: Runtime config uploadable from ground
  ]
  #show: later
  #text(size: 14pt)[
    #v(1pt)
    - Apps *store* data persistently in:
      - *Critical Data Store*: Small fast-access state that survives processor resets
      - *Filesystem*: Large slow-access state that survives processor resets and power-on resets
  ]
  #show: later
  #text(size: 14pt)[
    #v(1pt)
    - Apps additionally have access to tasks, timers, mutexes, clocks, logging, drivers, ...
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[Satellite Hardware: What it Guarantees]
  #v(8pt)

  #text(size: 15pt)[
    LeoDOS apps can rely on the following hardware guarantees:
    #v(4pt)
    - *Radiation tolerance* — Rad-hard processors survive single-event upsets and total ionizing dose
    - *ECC memory* — Bit flips from cosmic rays are detected and corrected automatically
    - *Watchdog timer* — A hardware timer forces a reset if software hangs; the system always recovers
    - *Deterministic execution* — Real-time processors guarantee bounded execution times for flight-critical tasks
    - *Persistent storage* — Battery-backed RAM and flash survive power cycles; data is not lost on reset
    - *Hardware power switches* — Subsystems can be independently powered down to manage the energy budget
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[CCSDS: How Satellites Communicate]
  #v(6pt)

  #text(size: 15pt)[
    - CCSDS is a family of standards for space communication protocols that form the stack:
    #v(4pt)
    - *Application Layer* — Compression, file transfer
    - *Transport Layer* — Reliable end-to-end delivery _(LeoDOS-specific)_
    - *Network Layer* — Multi-hop routing _(LeoDOS-specific)_
    - *Data Link Layer* — Spacepackets, framing, per-hop encryption, per-hop reliability
    - *Coding Layer* — Error correction, frame sync, randomization
    - *Physical Layer* — Modulation and hardware bus interfaces
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[SRSPP (Simple Reliable Space Packet Protocol)]
  #v(6pt)

  #text(size: 15pt)[
    - *Problem*: CCSDS has no reliable end-to-end transport for ISL mesh routing.

    - *SRSPP*: A LeoDOS-exclusive transport protocol that fills this gap

    - *SRSPP* is like TCP, but designed for space:
  ]
  #show: later
  #text(size: 15pt)[
    #v(1pt)
    - *Like TCP:*
      - *Segmentation* — Large messages split across packets, reassembled in order
      - *ACKs + retransmission* — Cumulative and selective ACKs, per-packet retransmit timers
  ]
  #show: later
  #text(size: 15pt)[
    #v(1pt)
    - *Unlike TCP:*
      - *No handshake* — Data sent immediately; a satellite may pass out of range before a handshake completes
      - *Orbit-aware retransmission* — Knows contact window schedules; holds packets until the link is available instead of retransmitting into the void
      - *Minimal overhead* — 3-byte header, reuses existing CCSDS Space Packet fields
  ]
]

#slide[
  #text(size: 28pt, weight: "bold")[NOS3: Simulation]
  #v(4pt)

  #text(size: 13pt)[
    How do you test flight software without launching a satellite?
  ]
  #show: later
  #text(size: 13pt)[
    #v(1pt)
    *NOS3 simulates:*
    - *Flight software* — Runs the exact cFS binary that would fly
    - *Orbital dynamics* — 42 propagates real orbits (position, attitude, sun angles)
    - *Synthetic sensors* — GPS, thermal camera, IMU, star tracker via simulated buses
    - *Radio links* — `generic_radio` forwards packets between satellites (also UDP/TCP)
    - *Multi-satellite constellations* — Sharing orbital dynamics and simulated time
  ]
  #show: later
  #text(size: 13pt)[
    #v(1pt)
    *NOS3 does not simulate:*
    - *RF physics* — ISL is raw UDP loopback; no fading, noise, or bit errors
    - *Time-varying topology* — ISL links permanently up; no neighbors entering/leaving range
    - *Hardware faults* — No single-event upsets or bit flips
  ]
  #show: later
  #text(size: 13pt)[
    #v(1pt)
    *The setup:* Docker Compose with services for 42 (dynamics), NOS Engine (bus), component sims, and one FSW container running N cFS processes (one per satellite).
  ]
  #show: later
  #text(size: 13pt)[
    #v(1pt)
    *Planned:* a topology sidecar that enforces line-of-sight gating, propagation delay, bandwidth limits, and ground contact windows.
  ]
]

#slide[
  #align(center + horizon)[
    #text(size: 32pt, weight: "bold")[Summary]
    #v(16pt)
    #text(size: 18pt)[
      The *downlink wall* limits value from space-based sensing. \
      *Optical ISLs* turn constellations into a lightspeed mesh computer. \
      #v(6pt)
      *SpaceCoMP* is a MapReduce-inspired programming model for Earth observation: \
      Collect · Map · Reduce — with three compounding optimizations. \
      #v(6pt)
      *LeoDOS* is the platform underneath: \
      CCSDS protocols and SRSPP transport, in Rust on NASA cFS, simulated with NOS3. \
      #v(6pt)
      *Research directions:* spatio-orbital stream processing, \
      end-to-end security, CRDTs for deep space.
    ]
  ]
]
