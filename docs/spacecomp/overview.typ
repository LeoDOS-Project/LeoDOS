#set page(paper: "a4", margin: 2.5cm)
#set text(font: "Helvetica Neue", size: 11pt)
#set heading(numbering: "1.1")
#set par(justify: true)

= SpaceCoMP Overview

SpaceCoMP (Space Computing Platform) enables distributed
computation across a LEO satellite constellation. The motivation
is that LEO satellites collect large volumes of data but have
limited downlink bandwidth. By processing data on-orbit and only
downlinking results, SpaceCoMP reduces the communication
bottleneck.

== Protocol Stack

SpaceCoMP uses the following subset of the LeoDOS communication
stack:

#let layer(name, ..children) = {
  rect(
    width: 100%,
    stroke: 0.75pt + black,
    inset: 4pt,
    [
      #text(weight: "bold", size: 8.5pt)[#name]
      #v(1pt)
      #children.pos().join()
    ]
  )
}

#let sublayer(body, width: 100%) = {
  rect(
    width: width,
    stroke: 0.5pt + luma(120),
    inset: 3pt,
    fill: rgb("#d4e8d4"),
    text(weight: "bold", size: 7.5pt)[#body]
  )
}

#let alt(..items) = {
  grid(
    columns: (1fr,) * items.pos().len(),
    column-gutter: 2pt,
    ..items.pos().map(c => sublayer(c, width: 100%))
  )
}

#let seq(body) = sublayer(body)

#layer("Application")[
  #seq([SpaceCoMP])
]
#v(-5pt)
#layer("Transport")[
  #alt([SRSPP], [CFDP])
]
#v(-5pt)
#layer("Network")[
  #alt([ISL Router], [PassThrough])
]
#v(-5pt)
#layer("Data Link")[
  #alt([SPP], [TC / TM Frames], [COP-1])
]
#v(-5pt)
#layer("Coding / Physical")[
  #alt([RS / LDPC], [ASM / CLTU], [BPSK / QPSK])
]

#v(8pt)

SRSPP handles reliable message delivery between satellites.
CFDP is used for larger file transfers to ground. The ISL
Router provides multi-hop mesh routing across the constellation;
PassThrough is used for direct point-to-point links.

== Constellation Model

Walker Delta constellation:

- N orbital planes, M satellites per plane.
- All planes at same inclination (e.g. 87°).
- Planes separated by 360/N degrees around Earth.
- Forms a 2D torus network topology.

=== Labeling Convention

- *Numbers (1, 2, 3, 4)* = different orbital planes.
- *Letters (A, B, C, D, E, F)* = positions along each orbit.

Example: A4, B4, C4, D4, E4, F4 are 6 satellites evenly spaced
around orbit 4.

== Ascending vs Descending

Each satellite is either ascending (moving toward north pole) or
descending (moving toward south pole). This creates a geographic
split:

- One hemisphere contains all ascending satellites.
- Other hemisphere contains all descending satellites.
- The boundary passes through both poles.

=== Link Constraints

*Intra-plane links (same orbit):* Always work. Satellites in the
same orbit maintain formation and move together.

*Inter-plane links (between orbits):*

- Within same phase (both ascending or both descending): work
  normally.
- Across the boundary (ascending -- descending): dynamic,
  constantly changing.

=== Dynamic Cross-Boundary Links

Ascending and descending satellites pass by each other at the
boundary. The specific pairing changes constantly --- a "sliding
seam" where a connection is always available but which satellites
are paired keeps shifting.

== Computation Model

SpaceCoMP implements a map-reduce model:

+ A ground station submits a *job* defining the computation and
  the geographic area of interest.
+ The *coordinator* plans the job: identifies which satellites
  are over the area, estimates cost (link quality, battery,
  orbital position), and solves an assignment problem using
  the Hungarian or LAPJV algorithm.
+ *Collectors* gather raw input data from on-board instruments.
+ *Mappers* process their assigned data partitions.
+ *Reducers* aggregate partial results into a final output.
+ The final result is downlinked to the ground station.

Communication between roles uses the transport layer (SRSPP or
CFDP depending on data size).

== Open Issues

=== Cross-Boundary Routing

The original paper restricts computations to one hemisphere:
only ascending or only descending satellites, never a mix. This
works for localized computations but doesn't solve global
routing.

=== Scheduling Time Constraints

If a satellite crosses the ascending/descending boundary
mid-computation, it loses inter-plane links. Computations must
complete before any involved satellite crosses the boundary, or
implement checkpoint/migration.

== Possible Improvements

- *Counter-rotating orbits:* some planes at +87°, others at
  −87° (retrograde), placing both ascending and descending
  satellites on each side.
- *Multiple shells:* different groups at different inclinations.
- *Equatorial orbits:* 0° inclination satellites to bridge
  hemispheres.
- *Ground relay:* downlink, route on ground, uplink to other
  hemisphere.
