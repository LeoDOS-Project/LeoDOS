#import "@preview/polylux:0.4.0": *

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
    #text(size: 44pt, weight: "bold")[Space Protocol Comparison]
    #v(12pt)
    #text(size: 22pt, fill: dim)[SPP · CFDP · CSP · HDTN · SRSPP]
    #v(24pt)
    #text(size: 16pt, fill: dim)[LeoDOS Protocol Analysis]
  ]
]

// ============================================================
// SPP
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[SPP — Space Packet Protocol]
  #v(2pt)
  #text(size: 13pt, fill: dim)[CCSDS#fn(1) 133.0-B-2 · Used on ISS, Mars rovers, Hubble]
  #v(6pt)

  #text(size: 16pt)[
    SPP is the *universal envelope* for space data. Every telemetry reading
    or command is wrapped in a Space Packet before transmission.
  ]

  #v(4pt)
  #text(size: 14pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 16pt,
      [
        *How it works:*
        - *Header:* 6-byte header with an APID#fn(2) identifying the destination app
        - *Multiplexing:* apps share one radio link; the APID routes each packet to the right handler
        - *No source address:* receiver infers sender from the link it arrived on
      ],
      [
        *Why it matters:*
        - *Universal:* every CCSDS mission uses SPP
        - *Stateless:* no connection setup, no retransmission, no ordering
        - *Foundation:* higher protocols (CFDP, SRSPP) build on top
      ],
    )
  ]

  #v(2pt)
  #text(size: 10pt, fill: dim)[
    #fn(1) Consultative Committee for Space Data Systems
    · #fn(2) Application Process Identifier
  ]
]

// ============================================================
// SPP Packet Structure
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[SPP Packet Structure]
  #v(6pt)

  #text(size: 12pt)[
    #table(
      columns: (1.4fr, 0.5fr, 2fr),
      stroke: none,
      inset: 3pt,
      fill: (_, y) => if y == 0 { luma(200) } else if y == 1 or y == 9 { luma(220) } else {
        if calc.rem(y, 2) == 0 { luma(240) } else { white }
      },
      table.header(
        text(weight: "bold")[SPP Packet],
        text(weight: "bold")[Size],
        text(weight: "bold")[Purpose],
      ),
      text(weight: "bold")[Primary Header], [6 B], [],
      [#h(6pt) Packet Version], [3 b], [Always 0 (CCSDS Version 1)],
      [#h(6pt) Packet Type], [1 b], [0 = telemetry (down), 1 = telecommand (up)],
      [#h(6pt) Secondary Hdr Flag], [1 b], [Whether a secondary header follows],
      [#h(6pt) APID], [11 b], [Destination process identifier (0–2047)],
      [#h(6pt) Sequence Flags], [2 b], [First / continuation / last / unsegmented],
      [#h(6pt) Sequence Count], [14 b], [Rolling counter per APID (0–16383)],
      [#h(6pt) Data Length], [16 b], [Byte count of the data field minus 1],
      text(weight: "bold")[Data Field], [1–65536 B], [Application payload],
    )
  ]

  #v(4pt)
  #text(size: 12pt)[
    SPP is a generic envelope. TC and TM packets extend it with mission-specific secondary headers.
  ]
]

// ============================================================
// Telecommand / Telemetry
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Telecommand (TC) / Telemetry (TM)]
  #v(2pt)
  #text(size: 13pt, fill: dim)[cFE#fn(1) packets — SPP with mission-specific secondary headers]
  #v(6pt)

  #text(size: 11pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 14pt,
      [
        #table(
          columns: (1.1fr, 0.5fr, 1.6fr),
          stroke: none,
          inset: 3pt,
          fill: (_, y) => if y == 0 { luma(200) } else if y == 1 or y == 2 or y == 5 { luma(220) } else {
            if calc.rem(y, 2) == 1 { luma(240) } else { white }
          },
          table.header(
            text(weight: "bold")[TC Packet],
            text(weight: "bold")[Size],
            text(weight: "bold")[Purpose],
          ),
          text(weight: "bold")[SPP Header], [6 B], [Primary header],
          text(weight: "bold")[Secondary Hdr], [2 B], [],
          [#h(6pt) Function Code], [1 B], [Command ID within APID],
          [#h(6pt) Checksum], [1 B], [8-bit XOR of entire packet],
          text(weight: "bold")[Payload], [≤64 KB], [Command parameters],
        )
      ],
      [
        #table(
          columns: (1.1fr, 0.5fr, 1.6fr),
          stroke: none,
          inset: 3pt,
          fill: (_, y) => if y == 0 { luma(200) } else if y == 1 or y == 2 or y == 5 { luma(220) } else {
            if calc.rem(y, 2) == 1 { luma(240) } else { white }
          },
          table.header(
            text(weight: "bold")[TM Packet],
            text(weight: "bold")[Size],
            text(weight: "bold")[Purpose],
          ),
          text(weight: "bold")[SPP Header], [6 B], [Primary header],
          text(weight: "bold")[Secondary Hdr], [10 B], [],
          [#h(6pt) Timestamp], [8 B], [CCSDS Day Segmented time],
          [#h(6pt) Spare], [2 B], [Alignment padding],
          text(weight: "bold")[Payload], [≤64 KB], [Telemetry data],
        )
      ],
    )
  ]

  #v(6pt)
  #text(size: 14pt)[
    *TC* (ground → spacecraft): the function code lets one APID handle multiple commands.
    *TM* (spacecraft → ground): the timestamp records when telemetry was generated.
  ]

  #v(4pt)
  #text(size: 10pt, fill: dim)[
    #fn(1) core Flight Executive
  ]
]

// ============================================================
// TC Transfer Frame
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[TC Transfer Frame]
  #v(2pt)
  #text(size: 13pt, fill: dim)[CCSDS 232.0-B · Uplink (ground → spacecraft)]
  #v(6pt)

  #text(size: 12pt)[
    #table(
      columns: (1.4fr, 0.5fr, 2fr),
      stroke: none,
      inset: 3pt,
      fill: (_, y) => if y == 0 { luma(200) } else if y <= 7 {
        if calc.rem(y, 2) == 1 { luma(240) } else { white }
      } else { luma(220) },
      table.header(
        text(weight: "bold")[TC Frame],
        text(weight: "bold")[Size],
        text(weight: "bold")[Purpose],
      ),
      [#h(6pt) Version], [2 b], [Always 0],
      [#h(6pt) Bypass Flag], [1 b], [Type-A (checked) / Type-B (bypass)],
      [#h(6pt) Control Flag], [1 b], [Type-D (data) / Type-C (control)],
      [#h(6pt) SCID#fn(1)], [10 b], [Spacecraft ID (0–1023); 2 reserved bits precede],
      [#h(6pt) VCID#fn(2)], [6 b], [Virtual Channel ID (0–63)],
      [#h(6pt) Frame Length], [10 b], [Total frame length minus 1],
      [#h(6pt) Sequence Num], [8 b], [Per-VC rolling counter (0–255)],
      text(weight: "bold")[Data Field], [1–1019 B], [One or more SPP packets],
      text(weight: "bold")[FECF#fn(3)], [2 B], [CRC error detection],
    )
  ]

  #v(4pt)
  #text(size: 11pt)[
    Wrapped in CLTUs#fn(4) with BCH#fn(5) error correction. Header is 5 bytes (40 bits).
  ]
  #text(size: 8pt, fill: dim)[
    #fn(1) Spacecraft ID · #fn(2) Virtual Channel ID · #fn(3) Frame Error Control Field · #fn(4) Cmd Link Transfer Unit · #fn(5) Bose–Chaudhuri–Hocquenghem
  ]
]

// ============================================================
// TM Transfer Frame
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[TM Transfer Frame]
  #v(2pt)
  #text(size: 13pt, fill: dim)[CCSDS 132.0-B · Downlink (spacecraft → ground)]
  #v(6pt)

  #text(size: 12pt)[
    #table(
      columns: (1.4fr, 0.5fr, 2fr),
      stroke: none,
      inset: 3pt,
      fill: (_, y) => if y == 0 { luma(200) } else if y <= 7 {
        if calc.rem(y, 2) == 1 { luma(240) } else { white }
      } else { luma(220) },
      table.header(
        text(weight: "bold")[TM Frame],
        text(weight: "bold")[Size],
        text(weight: "bold")[Purpose],
      ),
      [#h(6pt) Version], [2 b], [Always 0],
      [#h(6pt) SCID#fn(1)], [10 b], [Spacecraft ID (0–1023)],
      [#h(6pt) VCID#fn(2)], [3 b], [Virtual Channel ID (0–7)],
      [#h(6pt) OCF#fn(3) Flag], [1 b], [Whether OCF trailer is appended],
      [#h(6pt) MC Frame Count], [8 b], [Master Channel rolling counter],
      [#h(6pt) VC Frame Count], [8 b], [Virtual Channel rolling counter],
      [#h(6pt) Data Field Status], [16 b], [Flags + First Header Pointer (11 bits)],
      text(weight: "bold")[Data Field], [fixed], [Continuous stream of SPP packets],
      text(weight: "bold")[OCF#fn(3)], [4 B], [Operational control (optional)],
      text(weight: "bold")[FECF#fn(4)], [2 B], [CRC error detection (optional)],
    )
  ]

  #v(4pt)
  #text(size: 11pt)[
    First Header Pointer locates the first SPP packet in each frame. Optionally randomized for clock recovery.
  ]
  #text(size: 8pt, fill: dim)[
    #fn(1) Spacecraft ID · #fn(2) Virtual Channel ID · #fn(3) Operational Control Field · #fn(4) Frame Error Control Field
  ]
]

// ============================================================
// CFDP
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[CFDP — CCSDS File Delivery Protocol]
  #v(2pt)
  #text(size: 13pt, fill: dim)[CCSDS 727.0-B-5 · Standard for all CCSDS file transfers]
  #v(8pt)

  #text(size: 16pt)[
    CFDP transfers *files* and manages *remote filestores* between *entities*
    (nodes identified by an Entity ID). Each operation is a *transaction*.
  ]

  #v(4pt)
  #text(size: 14pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 16pt,
      [
        *How it works:*
        - *Entities:* each endpoint has an Entity ID (1–8 bytes); a sending entity initiates a transaction toward a destination entity
        - *Segmentation:* the sender splits a file into PDUs#fn(1) and sends them inside SPP packets
        - *Recovery:* in reliable mode (Class 2), the receiver sends NAKs listing missing segments for retransmission
      ],
      [
        *Why it matters:*
        - *Loss tolerant:* designed for lossy RF links where packets are frequently lost
        - *Filestore ops:* besides transfer, supports create/delete/rename files and create/remove directories
        - *Two classes:* Class 1 (unreliable, fire-and-forget) and Class 2 (reliable, NAK-based recovery)
      ],
    )
  ]

  #v(2pt)
  #text(size: 10pt, fill: dim)[
    #fn(1) Protocol Data Unit
  ]
]

// ============================================================
// CFDP PDU Types
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[CFDP PDU Types]
  #v(6pt)

  #text(size: 12pt)[
    #table(
      columns: (1.4fr, 0.5fr, 2fr),
      stroke: none,
      inset: 3pt,
      fill: (_, y) => if y == 0 { luma(200) } else if y == 1 or y == 2 or y == 3 { luma(220) } else {
        if calc.rem(y, 2) == 0 { luma(240) } else { white }
      },
      table.header(
        text(weight: "bold")[CFDP PDU#fn(1)],
        text(weight: "bold")[Size],
        text(weight: "bold")[Purpose],
      ),
      text(weight: "bold")[Fixed Header], [4 B], [Version, type, direction, lengths],
      text(weight: "bold")[Variable Header], [3–25 B], [Entity IDs (1–8 B each) + Txn Seq],
      text(weight: "bold")[Data Field], [≤64 KB], [File Data or Directive (see below)],
    )
  ]

  #v(4pt)
  #text(size: 13pt, weight: "bold")[Directive types:]
  #v(2pt)
  #text(size: 12pt)[
    #table(
      columns: (1fr, 2.5fr),
      stroke: none,
      inset: 3pt,
      fill: (_, y) => if y == 0 { luma(200) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
      table.header(
        text(weight: "bold")[PDU Type],
        text(weight: "bold")[Purpose],
      ),
      [Metadata], [Announces a file transfer: source/dest filenames and file size],
      [File Data], [Carries a segment of the file contents],
      [EOF], [Signals end of file, includes file checksum],
      [NAK], [Receiver lists missing segments for retransmission],
      [ACK], [Confirms receipt of EOF or Finished],
      [Finished], [Receiver confirms complete file delivery],
      [Prompt], [Requests the peer to send a NAK or Keep-Alive],
      [Keep-Alive], [Reports current receive progress to the sender],
    )
  ]

  #v(2pt)
  #text(size: 11pt)[
    *Class 1* (unreliable) uses only Metadata, File Data, and EOF.
    *Class 2* (reliable) adds the remaining directives.
    #h(1fr) #text(fill: dim)[#fn(1) Protocol Data Unit]
  ]
]

// ============================================================
// CSP
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[CSP — CubeSat Space Protocol]
  #v(2pt)
  #text(size: 13pt, fill: dim)[Aalborg University, 2008 · Used on AAUSAT-3, GomSpace satellites]
  #v(8pt)

  #text(size: 17pt)[
    CSP is a *lightweight network stack* for small satellites. It connects
    subsystems (OBC#fn(1), radio, payload) over internal buses.
  ]

  #v(6pt)
  #text(size: 16pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 20pt,
      [
        *How it works:*
        - *Addressing:* each node gets a 5-bit address (up to 32 nodes)
        - *Routing:* a router forwards packets between nodes over CAN#fn(2), I2C#fn(3), or UART#fn(4)
        - *Services:* built-in ports provide ping, uptime, and buffer status without application code
      ],
      [
        *Why it matters:*
        - *Tiny footprint:* runs on 8-bit AVR / 32-bit ARM MCUs with minimal RAM
        - *Dual transport:* unreliable (UDP-like) and reliable (RDP#fn(5)) delivery
        - *Scope:* designed for internal satellite wiring, not inter-satellite links
      ],
    )
  ]

  #v(4pt)
  #text(size: 10pt, fill: dim)[
    #fn(1) On-Board Computer
    · #fn(2) Controller Area Network
    · #fn(3) Inter-Integrated Circuit
    · #fn(4) Universal Async Receiver/Transmitter
    · #fn(5) Reliable Datagram Protocol
  ]
]

// ============================================================
// CSP Packet Structure
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[CSP Packet Structure]
  #v(2pt)
  #text(size: 13pt, fill: dim)[CSP v1 header (32 bits) — v2 extends addresses to 14 bits (48-bit header)]
  #v(6pt)

  #text(size: 12pt)[
    #table(
      columns: (1.4fr, 0.5fr, 2fr),
      stroke: none,
      inset: 3pt,
      fill: (_, y) => if y == 0 { luma(200) } else if y == 1 or y == 8 { luma(220) } else {
        if calc.rem(y, 2) == 0 { luma(240) } else { white }
      },
      table.header(
        text(weight: "bold")[CSP Packet],
        text(weight: "bold")[Size],
        text(weight: "bold")[Purpose],
      ),
      text(weight: "bold")[Header], [4 B], [],
      [#h(6pt) Priority], [2 b], [0 = critical, 1 = high, 2 = normal, 3 = low],
      [#h(6pt) Source], [5 b], [Source node ID (0–31)],
      [#h(6pt) Destination], [5 b], [Destination node ID (0–31)],
      [#h(6pt) Dest Port], [6 b], [Service port (0–63)],
      [#h(6pt) Source Port], [6 b], [Source port (0–63)],
      [#h(6pt) Flags], [8 b], [HMAC, RDP#fn(1), CRC32, fragmentation],
      text(weight: "bold")[Data], [≤256 B], [Application payload],
    )
  ]

  #v(4pt)
  #text(size: 12pt)[
    Ports 0–6 are reserved for built-in services (ping, reboot, uptime, memory stats).
    v2 expands addresses to 14 bits (16 384 nodes) and header to 6 bytes.
  ]
  #text(size: 8pt, fill: dim)[#fn(1) Reliable Datagram Protocol]
]

// ============================================================
// HDTN
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[HDTN — High-Rate Delay Tolerant Networking]
  #v(2pt)
  #text(size: 13pt, fill: dim)[NASA Glenn Research Center · Bundle Protocol v6/v7]
  #v(8pt)

  #text(size: 17pt)[
    HDTN implements *store-and-forward* networking for deep space, where
    a complete end-to-end path may never exist at any single moment.
  ]

  #v(6pt)
  #text(size: 16pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 20pt,
      [
        *How it works:*
        - *Bundles:* data is wrapped in bundles addressed by endpoint IDs
        - *Store-and-forward:* each node stores bundles on disk until a link to the next hop opens
        - *Scheduling:* CGR#fn(1) uses orbital predictions to precompute when links will exist
      ],
      [
        *Why it matters:*
        - *Delay tolerant:* handles minutes to hours of propagation delay and intermittent connectivity
        - *Persistent:* bundles survive link outages — no data lost if a contact window closes
        - *High throughput:* targets Gbps (C++ on x86), not embedded MCUs
      ],
    )
  ]

  #v(4pt)
  #text(size: 11pt, fill: dim)[
    #fn(1) Contact Graph Routing
  ]
]

// ============================================================
// Bundle Structure
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Bundle Protocol v7 Structure]
  #v(2pt)
  #text(size: 13pt, fill: dim)[RFC 9171 · CBOR#fn(1)-encoded, all sizes variable]
  #v(6pt)

  #text(size: 12pt)[
    #table(
      columns: (1.4fr, 0.5fr, 2fr),
      stroke: none,
      inset: 3pt,
      fill: (_, y) => if y == 0 { luma(200) } else if y == 1 or y == 9 or y == 10 { luma(220) } else {
        if calc.rem(y, 2) == 0 { luma(240) } else { white }
      },
      table.header(
        text(weight: "bold")[Bundle],
        text(weight: "bold")[Size],
        text(weight: "bold")[Purpose],
      ),
      text(weight: "bold")[Primary Block], [var.], [],
      [#h(6pt) Version], [1 B], [Always 7],
      [#h(6pt) Processing Flags], [var.], [Fragment, admin, must-not-fragment, ...],
      [#h(6pt) CRC Type], [var.], [0 = none, 1 = CRC-16, 2 = CRC-32],
      [#h(6pt) Destination EID#fn(2)], [var.], [Target endpoint identifier],
      [#h(6pt) Source EID#fn(2)], [var.], [Originating endpoint identifier],
      [#h(6pt) Report-to EID#fn(2)], [var.], [Status report destination],
      [#h(6pt) Timestamp + Seq], [var.], [Creation time + sequence number],
      text(weight: "bold")[Extension Blocks], [var.], [Hop count, age, previous node, ...],
      text(weight: "bold")[Payload Block], [var.], [Application data],
    )
  ]

  #v(4pt)
  #text(size: 12pt)[
    Bundles are self-describing CBOR arrays. Each node stores bundles on disk
    until a link to the next hop opens (store-and-forward).
  ]
  #text(size: 8pt, fill: dim)[
    #fn(1) Concise Binary Object Representation · #fn(2) Endpoint Identifier
  ]
]

// ============================================================
// SRSPP
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[SRSPP — Simple Reliable SPP]
  #v(2pt)
  #text(size: 13pt, fill: dim)[LeoDOS · Designed for LEO#fn(1) constellation ISL#fn(2)]
  #v(8pt)

  #text(size: 17pt)[
    SRSPP adds *reliable, ordered delivery* on top of SPP for low-latency
    ISLs where TCP is too heavy and plain SPP has no retransmission.
  ]

  #v(6pt)
  #text(size: 16pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 20pt,
      [
        *How it works:*
        - *Numbering:* reuses SPP's sequence count field — no extra header bloat
        - *Selective ACK:* receiver replies with a bitmap showing exactly which packets arrived
        - *No handshake:* data flows immediately, only missing packets are retransmitted
      ],
      [
        *Why it matters:*
        - *Bare-metal:* `no_std` Rust with zero heap allocation — runs on flight hardware
        - *Low latency:* designed for 5–40 ms LEO hops, not deep-space delays
        - *Instant start:* no connection setup, critical when contact windows are short
      ],
    )
  ]

  #v(4pt)
  #text(size: 11pt, fill: dim)[
    #fn(1) Low Earth Orbit
    · #fn(2) Inter-Satellite Link
  ]
]

// ============================================================
// SRSPP Packet Types
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[SRSPP Packet Types]
  #v(6pt)

  #text(size: 12pt)[
    #grid(
      columns: (1fr, 1fr),
      gutter: 14pt,
      [
        #table(
          columns: (1.1fr, 0.5fr, 1.6fr),
          stroke: none,
          inset: 3pt,
          fill: (_, y) => if y == 0 { luma(200) } else if y == 1 or y == 2 or y == 5 { luma(220) } else {
            if calc.rem(y, 2) == 1 { luma(240) } else { white }
          },
          table.header(
            text(weight: "bold")[DATA Packet],
            text(weight: "bold")[Size],
            text(weight: "bold")[Purpose],
          ),
          text(weight: "bold")[SPP Header], [6 B], [Primary header],
          text(weight: "bold")[SRSPP Header], [3 B], [],
          [#h(6pt) Source Addr], [2 B], [Sender node ID],
          [#h(6pt) Type], [1 B], [0x00 = DATA],
          text(weight: "bold")[Payload], [≤64 KB], [Application data],
        )
      ],
      [
        #table(
          columns: (1.1fr, 0.5fr, 1.6fr),
          stroke: none,
          inset: 3pt,
          fill: (_, y) => if y == 0 { luma(200) } else if y == 1 or y == 2 or y == 5 or y == 6 { luma(220) } else {
            if calc.rem(y, 2) == 1 { luma(240) } else { white }
          },
          table.header(
            text(weight: "bold")[ACK Packet],
            text(weight: "bold")[Size],
            text(weight: "bold")[Purpose],
          ),
          text(weight: "bold")[SPP Header], [6 B], [Primary header],
          text(weight: "bold")[SRSPP Header], [3 B], [],
          [#h(6pt) Source Addr], [2 B], [Sender node ID],
          [#h(6pt) Type], [1 B], [0x01 = ACK],
          text(weight: "bold")[Cumul. ACK], [2 B], [Highest in-order seq],
          text(weight: "bold")[Select. Bitmap], [2 B], [Out-of-order bitmap],
        )
      ],
    )
  ]

  #v(4pt)
  #text(size: 13pt)[
    The selective ACK bitmap reports exactly which packets arrived, so only missing ones
    are retransmitted. SRSPP reuses SPP's sequence count — no extra sequence field needed.
  ]
]

// ============================================================
// LeoDOS Stack Options
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[LeoDOS Stack Options]
  #v(4pt)
  #text(size: 13pt, fill: dim)[SRSPP connects to interchangeable network and datalink layers]
  #v(6pt)

  #text(size: 14pt, weight: "bold")[Network layer:]
  #v(2pt)
  #text(size: 14pt)[
    #table(
      columns: (1fr, 2.5fr),
      stroke: none,
      inset: 4pt,
      fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
      table.header(text(weight: "bold")[Option], text(weight: "bold")[Use case]),
      [PassThrough], [Point-to-point link, no routing],
      [ISL Routing], [Constellation mesh with gossip-based topology discovery],
    )
  ]

  #v(6pt)
  #text(size: 14pt, weight: "bold")[Datalink layer:]
  #v(2pt)
  #text(size: 14pt)[
    #table(
      columns: (1fr, 2.5fr),
      stroke: none,
      inset: 4pt,
      fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
      table.header(text(weight: "bold")[Option], text(weight: "bold")[Use case]),
      [TC/TM Frames], [Flight radio links (CCSDS standard)],
      [UDP Datagrams], [Ground testing and simulation],
      [CFS Software Bus], [Inter-app messaging within cFE#fn(1)],
    )
  ]

  #v(4pt)
  #text(size: 13pt)[
    The `NetworkLayer` and `DataLink` traits let SRSPP run over any combination.
    E.g. ground test: PassThrough + UDP; flight: ISL Routing + TC/TM.
    #h(1fr) #text(fill: dim)[#fn(1) core Flight Executive]
  ]
]

// ============================================================
// Protocol Stacks Side-by-Side
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Protocol Stacks]
  #v(6pt)

  #text(size: 14pt)[
    #table(
      columns: (1.2fr, 1fr, 1fr, 1fr, 1fr, 1fr),
      stroke: none,
      inset: 5pt,
      align: center,
      fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { white },
      table.header(
        text(weight: "bold", size: 12pt, fill: dim)[Layer],
        text(weight: "bold")[CCSDS/SPP],
        text(weight: "bold")[CFDP],
        text(weight: "bold")[CSP],
        text(weight: "bold")[HDTN],
        text(weight: "bold")[LeoDOS],
      ),
      [*Application*], [Application], [File App], [Services], [Application], [Application],
      [*Transport*], [SPP], [CFDP / SPP], [RDP#fn(1) / UDP], [Bundle Proto], [SRSPP],
      [*Network*], [—], [—], [CSP Router], [LTP#fn(2) / TCPCL#fn(3)], [ISL#fn(4) Routing],
      [*Datalink*], [TC#fn(5)/TM#fn(6) Frames], [TC#fn(5)/TM#fn(6) Frames], [CAN#fn(7)/I2C#fn(8)/KISS#fn(9)], [Any (via CL#fn(10))], [TC#fn(5)/TM#fn(6) Frames],
      [*Physical*], [CLTU#fn(11) / CADU#fn(12)], [CLTU#fn(11) / CADU#fn(12)], [CAN#fn(7) / I2C#fn(8) / UART#fn(13)], [Any], [CLTU#fn(11) / CADU#fn(12)],
    )
  ]

  #v(2pt)
  #text(size: 9pt, fill: dim)[
    #fn(1) Reliable Datagram Proto · #fn(2) Licklider Transmission Proto · #fn(3) TCP Convergence Layer · #fn(4) Inter-Satellite Link \
    #fn(5) Telecommand · #fn(6) Telemetry · #fn(7) Controller Area Network · #fn(8) Inter-Integrated Circuit · #fn(9) Keep It Simple, Stupid (framing) \
    #fn(10) Convergence Layer · #fn(11) Cmd Link Transfer Unit · #fn(12) Channel Access Data Unit · #fn(13) Universal Async Receiver/Transmitter
  ]
]

// ============================================================
// Comparison Table
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[Feature Comparison]
  #v(12pt)

  #text(size: 17pt)[
    #table(
      columns: (1.1fr, 1fr, 0.8fr, 1fr, 0.9fr, 0.9fr),
      stroke: none,
      inset: 6pt,
      fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { bg },
      table.header(
        text(weight: "bold")[],
        text(weight: "bold")[SPP],
        text(weight: "bold")[CFDP],
        text(weight: "bold")[CSP],
        text(weight: "bold")[HDTN],
        text(weight: "bold")[SRSPP],
      ),
      [*Reliability*], [None], [NAK], [RDP/None], [Checkpoint], [SACK#fn(1)],
      [*Routing*], [None], [None], [Static], [CGR#fn(2)], [ISL],
      [*Delay*], [Real-time], [Seconds], [Real-time], [Hours], [Real-time],
      [*Ordering*], [No], [Yes], [RDP only], [No], [Yes],
      [*Scope*], [Point-to-point], [Point-to-point], [32 nodes], [Internet], [65K nodes],
    )
  ]
  #v(4pt)
  #text(size: 11pt, fill: dim)[
    #fn(1) Selective Acknowledgment
    · #fn(2) Contact Graph Routing
  ]
]

// ============================================================
// When to Use What
// ============================================================

#slide[
  #text(size: 28pt, weight: "bold")[When to Use What]
  #v(12pt)

  #text(size: 18pt)[
    #table(
      columns: (2fr, 2.5fr),
      stroke: none,
      inset: 7pt,
      fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(245) } else { bg },
      table.header(
        text(weight: "bold")[Scenario],
        text(weight: "bold")[Best Fit],
      ),
      [Single CubeSat, simple telemetry], [*SPP* over TC/TM],
      [CubeSat with multiple subsystems], [*CSP* over CAN/I2C],
      [File transfer to ground], [*CFDP* over TC/TM],
      [LEO constellation mesh], [*SRSPP* + ISL Routing],
      [Mars relay, lunar gateway], [*HDTN* / Bundle Protocol],
      [LEO constellation with ground gaps], [*SRSPP* + *HDTN* hybrid],
    )
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
      These protocols are *complementary*, not competing. \
      #v(8pt)
      *SPP* is the universal packet envelope. \
      *CFDP* adds reliable file delivery on top. \
      *CSP* connects CubeSat subsystems over internal buses. \
      *HDTN* bridges networks with long delays and outages. \
      *SRSPP* provides fast, reliable transport for LEO meshes.
    ]
    #v(24pt)
    #text(size: 16pt, fill: dim)[
      LeoDOS combines CCSDS standards with custom LEO-optimized transport.
    ]
  ]
]
