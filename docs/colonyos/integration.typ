#import "@preview/polylux:0.4.0": *
#import "@preview/fletcher:0.5.8" as fletcher: diagram as fdiagram, node, edge

#set page(paper: "presentation-16-9")
#set text(font: "Helvetica Neue", size: 20pt)

#let bg = white
#let fg = black
#let dim = luma(120)

#set page(fill: bg)
#set text(fill: fg)

#let fn(n) = super(text(size: 10pt, fill: dim, str(n)))

// ============================================================
// Title
// ============================================================

#slide[
  #align(center + horizon)[
    #text(size: 44pt, weight: "bold")[SpaceCoMP & ColonyOS Integration]
    #v(12pt)
    #text(size: 22pt, fill: dim)[SpaceCoMP · ColonyOS · LeoDOS]
    #v(24pt)
    #text(size: 16pt, fill: dim)[Distributed Computing in LEO Constellations]
  ]
]

// ============================================================
// The Downlink Wall
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[The Downlink Wall]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Why process data in orbit instead of on the ground?]
  #v(8pt)

  #text(size: 17pt)[
    LEO#fn(1) Earth observation satellites generate *1–2 TB/day* each,
    but ground contact windows (*5–15 min per ~95 min orbit*) can only
    transfer a fraction. Most data never reaches the ground.
  ]

  #v(8pt)
  #text(size: 16pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 20pt,
      [
        *The downlink wall:*
        - Sensors produce data continuously
        - Ground contact is intermittent and brief
        - Downlink bandwidth is the bottleneck
        - Traditional operations: hours to days of latency
      ],
      [
        *Processing in orbit:*
        - Process data near where it's collected
        - Compress terabytes into megabytes before downlink
        - But how do you *program* a distributed computation across multiple satellites?
        - Need a programming model, not just bandwidth
      ],
    )
  ]

  #v(4pt)
  #text(size: 10pt, fill: dim)[
    #fn(1) Low Earth Orbit
    · #fn(2) Inter-Satellite Link
  ]
]

// ============================================================
// ISL Network Topology
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[ISL Network Topology]
  #v(2pt)
  #text(size: 13pt, fill: dim)[+Grid (2D-Torus) — four links per satellite]
  #v(6pt)

  #grid(
    columns: (1fr, 1.2fr),
    gutter: 16pt,
    [
      #text(size: 11pt)[
        #fdiagram(
          node-stroke: 0.5pt,
          node-corner-radius: 3pt,
          spacing: (20pt, 20pt),
          node((0, 0), [S], name: <s00>),
          node((1, 0), [S], name: <s10>),
          node((2, 0), [S], name: <s20>),
          node((0, 1), [S], name: <s01>),
          node((1, 1), text(weight: "bold")[S], name: <s11>, stroke: 1pt),
          node((2, 1), [S], name: <s21>),
          node((0, 2), [S], name: <s02>),
          node((1, 2), [S], name: <s12>),
          node((2, 2), [S], name: <s22>),
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
      #text(size: 10pt, fill: dim)[
        Each satellite has 4 ISL neighbors. \
        Routing uses Manhattan distance on the torus.
      ]
    ],
    [
      #text(size: 14pt)[
        *Four links per satellite:*
        - *North / South:* same orbital plane (ahead/behind)
        - *East / West:* adjacent orbital planes

        #v(6pt)
        *Properties:*
        - 10–100+ Gbps optical ISL bandwidth
        - 5–40 ms hop latency within LEO
        - Cross-plane distances vary over each orbital period (converge at poles, diverge at equator)
        - Positions are deterministic (SGP4/TLE#fn(1))
      ]
    ],
  )

  #v(2pt)
  #text(size: 8pt, fill: dim)[
    #fn(1) Simplified General Perturbations / Two-Line Element
  ]
]

// ============================================================
// Satellite Capabilities
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Satellite Capabilities]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Sensors and compute available on LEO Earth observation satellites]
  #v(6pt)

  #text(size: 12pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 14pt,
      [
        *Sensors:*
        #v(3pt)
        #table(
          columns: (0.8fr, 2fr),
          stroke: none,
          inset: 3pt,
          fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
          table.header(text(weight: "bold")[Type], text(weight: "bold")[Description]),
          [SAR#fn(1)], [Active microwave radar. Works through clouds/night. 1–5 m resolution, 80–250 km swath per pass. InSAR detects mm-scale ground displacement via phase comparison.],
          [Multispectral], [Passive optical, multiple wavelength bands. Vegetation, mineral, water analysis. 10–60 m (Sentinel-2) to sub-meter. 290 km swath.],
          [Optical], [High-resolution visible-light cameras. 30 cm – 3 m (commercial). Narrower swath (10–20 km) due to high resolution.],
        )
      ],
      [
        *Compute:*
        #v(3pt)
        #table(
          columns: (0.9fr, 2fr),
          stroke: none,
          inset: 3pt,
          fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
          table.header(text(weight: "bold")[Platform], text(weight: "bold")[Description]),
          [Rad-hard], [Traditional radiation-hardened processors. Far slower than ground hardware.],
          [COTS#fn(2)], [ARM-class processors with radiation shielding. Sometimes GPU accelerators (Nvidia Jetson on CubeSats).],
          [FPGA], [Specialized, low-power processing for specific algorithms.],
        )

        #v(4pt)
        Performance ranges from hundreds of MHz to low GHz, with limited RAM (MBs to low GBs). Constrained by radiation, power, and thermal limits.
      ],
    )
  ]

  #v(2pt)
  #text(size: 8pt, fill: dim)[
    #fn(1) Synthetic Aperture Radar
    · #fn(2) Commercial Off-The-Shelf
  ]
]

// ============================================================
// SpaceCoMP Model
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[SpaceCoMP — Collect · Map · Reduce]
  #v(2pt)
  #text(size: 13pt, fill: dim)[A programming model for distributed satellite computing]
  #v(6pt)

  #text(size: 15pt)[
    A ground station submits a job targeting an *Area of Interest* (AOI#fn(1)) — a geographic bounding box. SpaceCoMP extends MapReduce with a *Collect* phase. Processing happens in three phases:

    #v(6pt)
    + *Collect* — satellites whose ground footprints intersect the AOI acquire sensor data. Data stays local.
    + *Map* — nearby satellites process each collected partition, producing intermediate results.
    + *Reduce* — one satellite aggregates all map outputs into a final result.

  ]

  #v(6pt)
  #text(size: 14pt, weight: "bold")[How it works:]
  #v(2pt)
  #text(size: 13pt)[
    - *LOS coordinator:* the satellite visible to the ground station orchestrates the job
    - *Distance-optimized routing:* exploits time-varying inter-plane link geometry to minimize transmission distance (8–21% over standard +Grid routing)
    - *Bipartite matching:* assigns collector data to mappers via the Hungarian algorithm, minimizing transfer cost (61–79% over random)
    - *Center-of-AOI reduce placement:* reducer at the geometric center of mappers, not the LOS node (67–72% cost reduction)
  ]

  #v(4pt)
  #text(size: 10pt, fill: dim)[#fn(1) Area of Interest]
]

// ============================================================
// SpaceCoMP Examples
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[SpaceCoMP Examples]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Mining industry applications]
  #v(8pt)

  #text(size: 13pt)[
    - *Tailings dam monitoring:* detect mm-scale ground displacement near tailings dams before failure. *Collect:* radar strips over the AOI. *Map:* subtract phase data from a stored master. *Reduce:* aggregate pixels exceeding a displacement threshold.
    - *Mineral prospecting:* identify high-probability deposits over large areas. *Collect:* multispectral imagery. *Map:* compute band ratios per pixel, discard below threshold. *Reduce:* merge candidate pixel coordinates across partitions.
    - *Spill detection:* locate chemical spills across multiple satellite footprints. *Collect:* multispectral imagery. *Map:* run a water index to find liquid where it shouldn't be. *Reduce:* stitch partial results into a single spill polygon.
  ]
]

// ============================================================
// Vision
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[The Vision: Integrate SpaceCoMP with ColonyOS]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Use ColonyOS to orchestrate SpaceCoMP jobs across a satellite constellation]
  #v(12pt)

  #text(size: 16pt)[
    SpaceCoMP needs an orchestrator to assign roles, track progress, and recover from failures. Use ColonyOS.

    #v(10pt)
    - Submit SpaceCoMP jobs through ColonyOS, executed autonomously in orbit
    - Combine space and ground compute in a single job — one orchestrator for both
  ]
]

// ============================================================
// Constraints
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Orbital Constraints]
  #v(2pt)
  #text(size: 13pt, fill: dim)[What makes satellite orchestration different from cloud computing]
  #v(8pt)

  #text(size: 15pt)[
    - *Intermittent ground contact* — 5–15 min LOS per ~95 min orbit. No continuous ground connectivity.
    - *Multi-node jobs* — a single job requires multiple satellites (collectors, mappers, reducer).
    - *Split originator/completer* — Earth rotates during the job. Results route through whichever satellite has LOS at completion.
    - *Position-dependent assignment* — phases target satellites by orbital position relative to the AOI, not arbitrary workers.
    - *Embedded execution* — no HTTP, `no_std`, cFS. On-board protocols must fit flight software constraints.
    - *Deterministic orbits* — positions predictable via SGP4/TLE. Assignments can be planned ahead of time.
  ]
]

// ============================================================
// ColonyOS Model
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[ColonyOS — Meta-OS for Distributed Computing]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Coordinates heterogeneous workers across machines, clusters, or environments]
  #v(8pt)

  #text(size: 15pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 20pt,
      [
        *Core concepts:*
        #v(4pt)
        - *Pull-based:* executors call `assign()` when ready, receive a process spec, run it, report the result
        - *Identity:* every colony, executor, and user has an ECDSA#fn(1) key pair — messages are signed and verified
        - *Server-centric:* a central server maintains the process queue and tracks executor health
      ],
      [
        *Fault model:*
        #v(4pt)
        - If an executor stops sending keepalives, the server considers it *dead*
        - The process is made available for another executor to pick up
        - This works well when "unreachable = failed"
      ],
    )
  ]

  #v(2pt)
  #text(size: 10pt, fill: dim)[
    #fn(1) Elliptic Curve Digital Signature Algorithm
  ]
]

// ============================================================
// Design A1: Direct
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Design A1 — Direct Communication]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Ground server · Every satellite is a ColonyOS executor]
  #v(6pt)

  #grid(
    columns: (1fr, 1.2fr),
    gutter: 16pt,
    [
      #text(size: 11pt)[
        #fdiagram(
          node-stroke: 0.5pt,
          node-corner-radius: 3pt,
          spacing: (24pt, 18pt),
          node((1, 0), align(center)[*ColonyOS Server* \ (cloud)], name: <server>),
          node((1, 1), align(center)[*LOS Gateway* \ (satellite)], name: <gw>),
          node((0, 2), [Sat A], name: <a>),
          node((1, 2), [Sat B], name: <b>),
          node((2, 2), [Sat C], name: <c>),
          edge(<server>, <gw>, "<->", label: text(size: 9pt)[ground link]),
          edge(<gw>, <a>, "<->", label-side: left, label: text(size: 8pt)[ISL]),
          edge(<gw>, <b>, "<->"),
          edge(<gw>, <c>, "<->", label-side: right, label: text(size: 8pt)[ISL]),
        )
      ]
    ],
    [
      #text(size: 13pt)[
        *Pull:* each satellite holds a blocking `assign()` connection. Hundreds of connections routed through one LOS gateway.

        *Push:* server pushes to specific satellites. ColonyOS doesn't natively support push.

        #v(6pt)
        *Issues:*
        #v(4pt)
        - *Keepalives:* satellites go out of LOS predictably. Server constantly considers executors dead.
        - *Ground round-trips:* every phase transition bounces through the ground. Data that could flow satellite-to-satellite over ISL instead goes ISL → LOS → ground → LOS → ISL.
      ]
    ],
  )
]

// ============================================================
// Design A2: Satellite Coordinator
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Design A2 — Satellite Coordinator]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Ground server · One satellite is a ColonyOS executor]
  #v(6pt)

  #grid(
    columns: (1fr, 1.2fr),
    gutter: 16pt,
    [
      #text(size: 11pt)[
        #fdiagram(
          node-stroke: 0.5pt,
          node-corner-radius: 3pt,
          spacing: (24pt, 18pt),
          node((1, 0), align(center)[*ColonyOS Server* \ (cloud)], name: <server>),
          node((1, 1), align(center)[*Coordinator Sat* \ (executor)], name: <coord>, stroke: 1pt),
          node((0, 2), [Sat A], name: <a>),
          node((1, 2), [Sat B], name: <b>),
          node((2, 2), [Sat C], name: <c>),
          edge(<server>, <coord>, "<->", label: text(size: 9pt)[ground link]),
          edge(<coord>, <a>, "<->", label-side: left, label: text(size: 8pt)[ISL]),
          edge(<coord>, <b>, "<->"),
          edge(<coord>, <c>, "<->", label-side: right, label: text(size: 8pt)[ISL]),
          edge(<a>, <b>, "<->"),
          edge(<b>, <c>, "<->"),
        )
      ]
    ],
    [
      #text(size: 13pt)[
        *Pull:* the coordinator holds one `assign()` connection to the server.

        *Push:* server pushes to the coordinator.

        #v(6pt)
        *Improvement:*
        - Data flows directly between satellites over ISL
        - ColonyOS sees one job in, one result out
        - No per-phase ground round-trips

        #v(6pt)
        *Remaining issue:*
        - The coordinator also goes in and out of LOS
        - Same keepalive problem — server considers it dead when unreachable
      ]
    ],
  )
]

// ============================================================
// Design A3: Ground Coordinator
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Design A3 — Ground Coordinator]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Ground server · Ground process is the ColonyOS executor]
  #v(6pt)

  #grid(
    columns: (1fr, 1.2fr),
    gutter: 16pt,
    [
      #text(size: 11pt)[
        #fdiagram(
          node-stroke: 0.5pt,
          node-corner-radius: 3pt,
          spacing: (24pt, 18pt),
          node((1, 0), align(center)[*ColonyOS Server* \ (cloud)], name: <server>),
          node((1, 1), align(center)[*Ground Coordinator* \ (executor)], name: <ground>, stroke: 1pt),
          node((0.5, 2), align(center)[*LOS Satellite*], name: <los>),
          node((0, 3), [Sat A], name: <a>),
          node((1, 3), [Sat B], name: <b>),
          edge(<server>, <ground>, "<->"),
          edge(<ground>, <los>, "<->", label: text(size: 8pt)[ground link]),
          edge(<los>, <a>, "<->", label-side: left, label: text(size: 8pt)[ISL]),
          edge(<los>, <b>, "<->", label-side: right, label: text(size: 8pt)[ISL]),
          edge(<a>, <b>, "<->"),
        )
      ]
    ],
    [
      #text(size: 13pt)[
        *Pull:* ground executor holds one `assign()` connection. Always reachable — pull works naturally.

        *Push:* server pushes to the ground executor. Requires ColonyOS push support.

        #v(6pt)
        *Advantages:*
        - *No keepalive issue:* ground executor is always reachable
        - Satellite communication handled separately, outside ColonyOS's model

        #v(6pt)
        *This is the recommended design.* \
        The following slides assume this approach.
      ]
    ],
  )
]

// ============================================================
// Job Flow: Ground-originated
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Ground-Originated Job Flow]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Design A3 — user submits a job via ColonyOS]
  #v(6pt)

  #text(size: 14pt)[
    #table(
      columns: (0.3fr, 2.5fr),
      stroke: none,
      inset: 5pt,
      fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
      table.header(text(weight: "bold")[Step], text(weight: "bold")[Description]),
      [1], [User submits a SpaceCoMP job to ColonyOS (AOI, algorithm, parameters).],
      [2], [Ground coordinator pulls the job via `assign()`.],
      [3], [Coordinator computes the plan: which satellites are collectors, mappers, reducer (using SGP4/TLE orbital predictions).],
      [4], [Coordinator uploads assignments to the constellation via the current LOS satellite.],
      [5], [Satellites execute autonomously over ISL. Data flows directly between satellites.],
      [6], [Result routes back through whichever satellite has LOS to the ground station.],
      [7], [Ground coordinator reports the result to ColonyOS.],
    )
  ]

  #v(4pt)
  #text(size: 14pt)[
    The ground coordinator decouples ColonyOS from orbital mechanics. ColonyOS sees
    a single executor that always responds to keepalives.
  ]
]

// ============================================================
// Job Flow: Satellite-originated
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Satellite-Originated Job Flow]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Design A3 — a satellite triggers a job autonomously]
  #v(8pt)

  #text(size: 14pt)[
    #table(
      columns: (0.3fr, 2.5fr),
      stroke: none,
      inset: 5pt,
      fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
      table.header(text(weight: "bold")[Step], text(weight: "bold")[Description]),
      [1], [A satellite detects an anomaly in sensor data (e.g., unexpected displacement).],
      [2], [It routes a job request through the ISL mesh to the LOS satellite.],
      [3], [LOS satellite relays the request to the ground coordinator.],
      [4], [Ground coordinator submits to ColonyOS, pulls it back, and orchestrates as usual.],
      [5], [Latency is the ground link round-trip.],
    )
  ]

  #v(8pt)
  #text(size: 16pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 20pt,
      [
        *Variant: in-orbit coordinator* \
        For time-critical cases, a satellite could coordinate directly in orbit — planning and assigning roles without involving the ground.
      ],
      [
        *Tradeoff:* \
        Eliminates the ground round-trip but requires on-board planning logic and cannot leverage ColonyOS for orchestration.
      ],
    )
  ]
]

// ============================================================
// Design B: Satellite Server
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Design B — Satellite Server]
  #v(2pt)
  #text(size: 13pt, fill: dim)[ColonyOS server running on a satellite — future direction]
  #v(8pt)

  #text(size: 16pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 20pt,
      [
        *Concept:*
        - ColonyOS server runs on a satellite
        - Executors (other satellites) communicate entirely over ISL
        - No ground round-trips for orchestration
        - Keepalive problem reduced — satellites can always reach the server over ISL
      ],
      [
        *Challenges:*
        - ColonyOS is implemented in Go — requires a full OS environment
        - Running on a satellite would require an embedded reimplementation (`no_std`, cFS)
        - Single satellite server = single point of failure
        - Significant compute resources needed on the server satellite
      ],
    )
  ]

]

// ============================================================
// Design Comparison
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Design Comparison]
  #v(12pt)

  #text(size: 14pt)[
    #table(
      columns: (1.4fr, 1fr, 1fr, 1fr, 1fr),
      stroke: none,
      inset: 5pt,
      fill: (_, y) => if y == 0 { luma(200) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
      table.header(
        text(weight: "bold")[],
        text(weight: "bold")[A1: Direct],
        text(weight: "bold")[A2: Sat Coord.],
        text(weight: "bold")[A3: Gnd Coord.],
        text(weight: "bold")[B: Sat Server],
      ),
      [*Keepalive issue*], [Yes], [Yes], [No], [Reduced],
      [*Ground round-trips*], [Every phase], [Job in/out], [Job in/out], [None],
      [*ISL data flow*], [No], [Yes], [Yes], [Yes],
      [*ColonyOS changes*], [None], [None], [None], [Reimplementation],
      [*Practical today*], [Poor fit], [Poor fit], [Yes], [No],
    )
  ]

  #v(8pt)
  #text(size: 16pt)[
    *Design A3 (Ground Coordinator)* is the recommended approach. It works within
    ColonyOS's existing model, avoids the keepalive problem, and lets satellites
    exchange data directly over ISL.
  ]
]

// ============================================================
// Summary
// ============================================================

#slide[
  #align(center + horizon)[
    #text(size: 32pt, weight: "bold")[Key Takeaway]
    #v(20pt)
    #text(size: 20pt)[
      ColonyOS assumes *always-reachable* executors. \
      Satellites are *predictably unreachable*. \
      #v(8pt)
      The solution: keep ColonyOS on the ground \
      and let a *ground coordinator* bridge the gap. \
      #v(8pt)
      Satellites execute autonomously over ISL. \
      ColonyOS sees a single, always-available executor.
    ]
    #v(24pt)
    #text(size: 16pt, fill: dim)[
      LeoDOS provides the on-board stack: SRSPP transport, ISL routing, cFS integration.
    ]
  ]
]

// ============================================================
// Backup: InSAR
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Backup: InSAR]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Interferometric Synthetic Aperture Radar — detecting mm-scale ground displacement]
  #v(6pt)

  #text(size: 12pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 16pt,
      [
        *SAR (Synthetic Aperture Radar):*
        - Sends microwave pulses, records the reflected signal
        - Satellite motion synthesizes a large antenna — meter-scale resolution per pixel
        - Works through clouds and at night

        #v(4pt)
        *InSAR adds interferometry:*
        - Compare two SAR images of the same area from different passes
        - Measure the *phase shift* of the returned wave between passes
      ],
      [
        *How mm precision works:*
        - Radar wavelength is fixed (~5.6 cm for C-band)
        - 5 mm ground shift → 10 mm round-trip change → ~18% of a wave cycle
        - Each pixel covers meters, but phase is sensitive to sub-wavelength changes

        #v(4pt)
        *Correction for satellite motion:*
        - Orbit differs slightly between passes, but position is known to cm precision
        - Compute expected phase from orbit difference, subtract — remainder is ground displacement
      ],
    )
  ]
]

// ============================================================
// Backup: Multispectral Imaging
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Backup: Multispectral Imaging]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Identifying ground materials by how they reflect different wavelengths]
  #v(6pt)

  #text(size: 12pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 16pt,
      [
        *How it works:*
        - Different materials reflect different wavelengths differently
        - Sensor captures multiple narrow bands simultaneously — like photos through different color filters
        - Sentinel-2: 13 bands, 10–60 m per pixel

        #v(4pt)
        *Band ratios:*
        - Divide one band by another to detect materials
        - NDVI#fn(1) = (NIR#fn(2) − red) / (NIR + red)
        - High NDVI = healthy vegetation, low = bare soil/rock
      ],
      [
        *Material signatures:*

        #table(
          columns: (1fr, 1.5fr),
          stroke: none,
          inset: 3pt,
          fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
          table.header(text(weight: "bold")[Material], text(weight: "bold")[Signature]),
          [Healthy vegetation], [Bright in NIR, dark in red],
          [Iron oxide minerals], [Bright in red, dark in blue],
          [Water], [Dark in infrared],
          [Stressed crops], [Less NIR than healthy],
        )

        #v(4pt)
        Output is a stack of measurements per pixel, classified algorithmically. Only flagged pixels need to be downlinked.
      ],
    )
  ]

  #v(2pt)
  #text(size: 8pt, fill: dim)[
    #fn(1) Normalized Difference Vegetation Index
    · #fn(2) Near-Infrared
  ]
]
