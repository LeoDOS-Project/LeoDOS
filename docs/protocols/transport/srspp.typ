#set page(paper: "a4", margin: 2.5cm)
#set text(font: "Helvetica Neue", size: 11pt)
#set heading(numbering: "1.1")
#set par(justify: true)

= SRSPP: Simple Reliable Space Packet Protocol

SRSPP is a lightweight reliable transport protocol built on top of
CCSDS Space Packets. It provides reliable, ordered delivery of
messages over unreliable links with minimal overhead.

== Design Goals

- *Minimal overhead:* reuses existing SPP header fields (sequence
  count, sequence flags)
- *No congestion control:* designed for point-to-point links with
  predictable latency
- *Simple implementation:* suitable for resource-constrained flight
  software
- *Efficient ACKs:* combined cumulative and selective acknowledgment
- *No handshake:* data is sent immediately without connection
  establishment

=== Why No Handshake?

SRSPP has no connection setup or liveness check. The sender can
transmit data immediately without waiting for a round-trip
confirmation that the receiver exists.

This design choice is motivated by space link characteristics:

- *High latency:* handshakes add significant delay on links with
  multi-second round-trips. In the worst case, a satellite may pass
  out of range before a handshake completes.
- *Known endpoints:* mission configurations define who communicates
  with whom.
- *Scheduled contacts:* ground stations know when satellites are
  reachable.

The tradeoff is that sending to an unreachable receiver wastes time
until packets hit max retransmits. The application receives packet
loss notifications and can decide when to abort. This optimizes for
the common case (receiver is reachable) rather than the failure
case.

== SRSPP Header

All SRSPP packets begin with a 3-byte header following the SPP
primary header:

#table(
  columns: (1.2fr, 0.6fr, 2fr),
  stroke: none,
  inset: 4pt,
  fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
  table.header(
    text(weight: "bold")[Field],
    text(weight: "bold")[Size],
    text(weight: "bold")[Description],
  ),
  [Source Address], [2 bytes], [Address of the sender (RawAddress format)],
  [Type], [2 bits], [Packet type: 0=DATA, 1=ACK, 2=EOS, 3=Reserved],
  [Version], [2 bits], [Protocol version (current: 0)],
  [Spare], [4 bits], [Reserved for future use, must be 0],
)

The source address identifies the sending spacecraft. Combined with
the destination APID from the SPP header, it forms the stream key:
`(source_address, destination_apid)`.

Only one sender per spacecraft is supported for each destination
APID. In CFS, this is typically achieved by having a single gateway
app handle the SRSPP stream while other apps communicate with it
via the Software Bus.

== Packet Types

SRSPP defines three packet types.

=== DATA Packet (Type=0)

```
+------------------------------------+---------+
| Space Packet Primary Header        | 6 bytes |
|   - APID: Destination application            |
|   - Sequence Count: Reliability seq number   |
|   - Sequence Flags: Segmentation info        |
+------------------------------------+---------+
| SRSPP Header                       | 3 bytes |
|   - Source Address (2 bytes)                 |
|   - Type/Version/Spare (1 byte)              |
+------------------------------------+---------+
| Payload                            | N bytes |
+------------------------------------+---------+
```

=== ACK Packet (Type=1)

```
+------------------------------------+---------+
| Space Packet Primary Header        | 6 bytes |
+------------------------------------+---------+
| SRSPP Header                       | 3 bytes |
|   - Source Address (2 bytes)                 |
|   - Type/Version/Spare (1 byte)              |
+------------------------------------+---------+
| ACK Payload                        | 4 bytes |
|   - Cumulative ACK (u16)                     |
|   - Selective ACK Bitmap (u16)               |
+------------------------------------+---------+
```

=== EOS Packet (Type=2)

```
+------------------------------------+---------+
| Space Packet Primary Header        | 6 bytes |
+------------------------------------+---------+
| SRSPP Header                       | 3 bytes |
|   - Source Address (2 bytes)                 |
|   - Type/Version/Spare (1 byte)              |
+------------------------------------+---------+
```

The EOS packet signals that the sender has no more data to
transmit. It has its own sequence number and is acknowledged like a
DATA packet. The transfer is complete when the sender receives an
ACK covering the EOS sequence number.

== Reliability Mechanism

=== Sequence Numbers

SRSPP uses the 14-bit sequence count field from the SPP primary
header for reliability. Sequence numbers wrap around at 16383
(0x3FFF).

=== Acknowledgments

The receiver acknowledges received packets using a combined
cumulative and selective ACK scheme:

*Cumulative ACK:* the highest sequence number received in-order.
All packets up to and including this number are acknowledged.

*Selective ACK Bitmap:* a 16-bit bitmap indicating which
out-of-order packets have been received. Bit N indicates receipt
of packet (cumulative\_ack + 1 + N).

Example:
```
Received: 0, 1, 2, 5, 7
Cumulative ACK: 2
Selective Bitmap: 0b00010100
                      │  │
                      │  └─ bit 2 = seq 5 received
                      └──── bit 4 = seq 7 received
```

=== Retransmission

The sender maintains a retransmission timer for each
unacknowledged packet. When a timer expires, the packet is
retransmitted. After a configurable number of failed
retransmissions, the packet is considered lost.

=== ACK Timing

The receiver can be configured for:

- *Immediate ACK:* send ACK immediately upon receiving any data
  packet.
- *Delayed ACK:* wait for a timeout before sending ACK (allows
  batching).

== Segmentation

Large messages that exceed the MTU are segmented using the SPP
sequence flags:

#table(
  columns: (1.2fr, 0.6fr, 2fr),
  stroke: none,
  inset: 4pt,
  fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
  table.header(
    text(weight: "bold")[Flag],
    text(weight: "bold")[Value],
    text(weight: "bold")[Meaning],
  ),
  [Unsegmented], [`0b11`], [Complete message in one packet],
  [First], [`0b01`], [First segment of a message],
  [Continuation], [`0b00`], [Middle segment],
  [Last], [`0b10`], [Final segment of a message],
)

The receiver reassembles segments in order. All segments of a
message share consecutive sequence numbers.

== Flow Control

SRSPP uses a sliding window for flow control:

- *Window size (WIN):* maximum number of unacknowledged packets
  in flight.
- *Send buffer (BUF):* total bytes that can be queued for
  transmission.

The sender blocks when either the window is full or the buffer is
exhausted.

== Progress Timeout

The receiver optionally supports a *progress timeout* for
semi-reliable operation. When `progress_timeout_ticks` is set:

+ A gap is detected (out-of-order packet arrives).
+ The progress timer starts.
+ If the gap fills before the timer expires, the timer is
  cancelled.
+ If the timer expires with the gap still open, the receiver
  *skips* the missing packet — advancing `expected_seq` past the
  gap and delivering any buffered packets that are now
  consecutive.

This allows the receiver to make forward progress even when a
packet is permanently lost, without waiting for the sender's
retransmission. It trades guaranteed delivery for bounded
latency.

When `progress_timeout_ticks` is `None`, the receiver never skips
gaps. It relies entirely on the sender's retransmission mechanism
(fully reliable mode).

The progress timer restarts each time a new gap forms after a
skip. If multiple gaps exist, each is skipped one at a time as
successive timeouts expire.

== Configuration Parameters

#table(
  columns: (1.2fr, 0.6fr, 0.5fr, 2fr),
  stroke: none,
  inset: 4pt,
  fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
  table.header(
    text(weight: "bold")[Parameter],
    text(weight: "bold")[Used by],
    text(weight: "bold")[Default],
    text(weight: "bold")[Description],
  ),
  [APID], [Both], [—], [Application Process ID for routing],
  [WIN], [Both], [8], [Window size (max in-flight packets)],
  [BUF], [Sender], [4096], [Send buffer size (bytes)],
  [MTU], [Sender], [512], [Maximum transmission unit (bytes per packet)],
  [REASM], [Receiver], [8192], [Maximum reassembled message size (bytes)],
  [RTO Policy], [Sender], [Fixed], [Retransmission timeout strategy],
  [Max Retransmits], [Sender], [3], [Attempts before declaring packet lost],
  [ACK Delay], [Receiver], [100], [Delayed ACK timeout (ticks)],
  [Progress Timeout], [Receiver], [None], [Gap-skip timeout (ticks); None = fully reliable],
)

=== RTO Policy

The retransmission timeout is governed by a pluggable `RtoPolicy`
trait. The driver queries the policy each time it starts a
retransmission timer, passing the current time so the policy can
adapt dynamically.

Two built-in policies are provided:

*FixedRto* — returns a constant timeout. Suitable for ISL links
with stable, predictable latency.

*OrbitAwareRto* — adapts the timeout based on a contact schedule:

- If the current time falls inside a LOS window: use a short ISL
  RTO (the link is active, real loss should be detected quickly).
- If outside a window: set RTO to the time until the next LOS
  window plus a configurable margin. This prevents the sender from
  declaring packets lost during normal orbital gaps.
- If no future windows exist in the schedule: fall back to the ISL
  RTO.

The contact schedule is stored in a `ContactSchedule<N>` backed by
a `heapless::Vec`, keeping it `no_std` compatible. Each window
records a station ID and start/end time in seconds.

Custom policies can be implemented by implementing
`RtoPolicy::rto_ticks`.

*Note:* SRSPP does not negotiate parameters at runtime. Both
endpoints must be configured with compatible values.

Compatibility constraints:

- APID must match for routing to work.
- Receiver's WIN should be ≥ sender's WIN (to buffer all in-flight
  packets).
- Receiver's REASM must be ≥ sender's largest message size.

== Sender Operation

The sender maintains a window of unacknowledged packets. Each
packet in the window is in one of two states:

- *Pending Transmit:* queued but not yet sent, or marked for
  retransmission.
- *Awaiting ACK:* transmitted and waiting for acknowledgment.

=== Send Flow

+ Application submits a message to send.
+ If message exceeds MTU, segment it into multiple packets
  (First / Continuation / Last).
+ For each packet: assign the next sequence number, store in send
  buffer, mark as Pending Transmit.
+ Transmit all Pending Transmit packets.
+ Start retransmission timer for each transmitted packet.
+ Mark transmitted packets as Awaiting ACK.

=== ACK Processing

When an ACK arrives:

+ For each packet covered by the cumulative ACK (seq ≤
  cumulative\_ack): stop its retransmission timer, remove from
  send buffer.
+ For each bit set in the selective bitmap: calculate
  seq = cumulative\_ack + 1 + bit\_position, stop that packet's
  retransmission timer, remove from send buffer.
+ Slide the window forward.

=== Timeout Handling

When a retransmission timer expires:

+ If retransmit count < max\_retransmits: increment retransmit
  count, mark packet as Pending Transmit, retransmit the packet,
  restart timer.
+ Otherwise: declare packet lost, remove from send buffer, signal
  error to application.

=== Stream Termination

To signal end of transmission:

+ Application requests stream close.
+ Send EOS packet with next sequence number.
+ Wait for ACK covering the EOS sequence.
+ Transfer is complete when EOS is acknowledged.

The EOS is retransmitted like DATA if no ACK arrives.

== Receiver Operation

The receiver maintains the expected sequence number and a reorder
buffer for out-of-order packets.

=== Receive Flow

When a DATA or EOS packet arrives:

+ Compare packet sequence to expected sequence.
+ If seq == expected (in-order): deliver payload to reassembly,
  advance expected sequence, check reorder buffer for
  now-deliverable packets, repeat until no more consecutive
  packets.
+ If seq > expected but within window (out-of-order): store in
  reorder buffer, set corresponding bit in selective bitmap.
+ If seq < expected (duplicate): ignore.
+ Schedule or send ACK (see ACK Generation).

=== Reassembly

As packets are delivered in order:

+ Check sequence flag:
  - Unsegmented: complete message, deliver to application.
  - First: start new reassembly buffer.
  - Continuation: append to reassembly buffer.
  - Last: append and deliver complete message to application.

=== ACK Generation

After processing each DATA packet:

+ If immediate\_ack mode: send ACK immediately.
+ If delayed\_ack mode: if no ACK timer running, start one. When
  timer expires, send ACK.

The ACK contains:

- Cumulative ACK = expected\_seq − 1 (highest in-order seq
  received).
- Selective bitmap = bits for each buffered out-of-order packet.

== Version Handling

The SRSPP header includes a 2-bit version field. When a packet
arrives with an unrecognized version:

+ Discard the packet silently.
+ Do not send an ACK (sender will retransmit or timeout).
+ Optionally log the event for diagnostics.

This allows future protocol versions to coexist during
transitions. Endpoints should be upgraded to matching versions
for reliable communication.

== Error Handling

=== Sender Errors

#table(
  columns: (1.2fr, 2fr),
  stroke: none,
  inset: 4pt,
  fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
  table.header(
    text(weight: "bold")[Condition],
    text(weight: "bold")[Behavior],
  ),
  [Send buffer full], [Block or reject new messages until space available],
  [Window full], [Block until ACKs free window slots],
  [Packet lost (max retransmits)], [Signal error to application, remove packet],
)

=== Receiver Errors

#table(
  columns: (1.2fr, 2fr),
  stroke: none,
  inset: 4pt,
  fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
  table.header(
    text(weight: "bold")[Condition],
    text(weight: "bold")[Behavior],
  ),
  [Reorder buffer full], [Drop out-of-order packet (will be retransmitted)],
  [Message too large for REASM], [Discard segments, signal error to application],
  [Continuation without First], [Discard segment, signal reassembly error],
  [Unknown packet type], [Discard silently],
  [Unknown version], [Discard silently],
)

Receiver errors do not generate negative acknowledgments. The
sender will retransmit based on timeouts if needed.

== Protocol Sequences

=== Normal Operation (Immediate ACK)

```
  Sender                                    Receiver
    │                                          │
    │────── DATA [seq=0] ─────────────────────>│
    │────── DATA [seq=1] ─────────────────────>│
    │────── DATA [seq=2] ─────────────────────>│
    │<─────────────────── ACK [cum=0, bmp=0] ──│
    │<─────────────────── ACK [cum=1, bmp=0] ──│
    │<─────────────────── ACK [cum=2, bmp=0] ──│
    │                                          │
```

With immediate ACK mode, the receiver sends an ACK after every
DATA packet. The sender does not wait for ACKs before sending
more packets — it can have up to WIN packets in flight
simultaneously.

=== Normal Operation (Delayed ACK)

```
  Sender                                    Receiver
    │                                          │
    │────── DATA [seq=0] ─────────────────────>│
    │                                          │  (start ACK timer)
    │────── DATA [seq=1] ─────────────────────>│
    │────── DATA [seq=2] ─────────────────────>│
    │                                          │  (ACK timer expires)
    │<─────────────────── ACK [cum=2, bmp=0] ──│
    │                                          │
```

=== Packet Loss and Retransmission

```
  Sender                                    Receiver
    │                                          │
    │────── DATA [seq=0] ─────────────────────>│
    │<─────────────────── ACK [cum=0, bmp=0] ──│
    │────── DATA [seq=1] ────────X             │  (lost)
    │────── DATA [seq=2] ─────────────────────>│
    │<────────────────── ACK [cum=0, bmp=01] ──│
    │  (timeout for seq=1)                     │
    │────── DATA [seq=1] ─────────────────────>│  (retransmit)
    │<─────────────────── ACK [cum=2, bmp=0] ──│
    │                                          │
```

=== Progress Timeout (Gap Skip)

```
  Sender                                    Receiver
    │                                          │
    │────── DATA [seq=0] ─────────────────────>│
    │<─────────────────── ACK [cum=0, bmp=0] ──│
    │────── DATA [seq=1] ────────X             │  (permanently lost)
    │────── DATA [seq=2] ─────────────────────>│
    │<────────────────── ACK [cum=0, bmp=01] ──│
    │                                          │  (progress timer starts)
    │                                          │  ...
    │                                          │  (progress timer expires)
    │                                          │  expected_seq jumps to 3
    │                                          │  seq=2 delivered to app
    │                                          │
```

When the progress timer expires, the receiver skips past seq=1 and
delivers seq=2. The sender may still retransmit seq=1, but it will
be treated as a duplicate (seq < expected) and ignored.

=== Segmented Message

```
  Sender                                    Receiver
    │                                          │
    │  Message: "HELLO WORLD" (segmented)      │
    │────── DATA [seq=0, FIRST, "HELLO"] ─────>│
    │                                          │  (start ACK timer)
    │────── DATA [seq=1, LAST, " WORLD"] ─────>│
    │                                          │  (ACK timer expires)
    │<─────────────────── ACK [cum=1, bmp=0] ──│
    │                         Message ready:   │
    │                         "HELLO WORLD"    │
    │                                          │
```

=== End of Stream

```
  Sender                                    Receiver
    │                                          │
    │────── DATA [seq=0] ─────────────────────>│
    │────── DATA [seq=1] ────────X             │  (lost)
    │────── DATA [seq=2] ─────────────────────>│
    │────── EOS  [seq=3] ─────────────────────>│
    │<─────────────────── ACK [cum=0, bmp=0] ──│
    │<────────────────── ACK [cum=0, bmp=110] ─│
    │  (timeout for seq=1)                     │
    │────── DATA [seq=1] ─────────────────────>│  (retransmit)
    │<─────────────────── ACK [cum=3, bmp=0] ──│
    │         Transfer complete                │
```

The EOS packet has its own sequence number and is acknowledged
like any DATA packet. The transfer is complete when the sender
sees an ACK covering the EOS sequence number.

#pagebreak()

= Receiver Buffer Design

== Implementation Architecture

The receiver is split into three layers:

+ *Backend* (`ReceiverBackend` trait) — buffering and delivery
  only. Takes a packet, buffers or delivers it, returns a
  `DataOutcome { progressed, has_gap }`. Gap-skipping returns
  `GapOutcome { has_gap }`. No config, no ACK logic, no timers.

+ *Sequence tracker* (`ReceiverBase`) — owned by the backend.
  Tracks `expected_seq` and a `recv_bitmap` for duplicate
  detection. The backend calls `advance()`, `record_ooo()`, etc.

+ *ACK state* (`AckState`) — owned by the driver, per stream.
  Takes a `DataOutcome` or `GapOutcome` plus the backend's
  current `expected_seq` and `recv_bitmap`, produces a
  `HandleResult` with optional ACK info and timer actions.

Three backends are implemented. `PackedReceiver` (Option C) is
the default (`ReceiverMachine` type alias).

== Primitives

All backends use `Vec<T, N>` — a fixed-capacity, stack-allocated
vector — and decompose into five building blocks:

#table(
  columns: (1.5fr, 1fr, 1fr, 1fr, 1fr, 1fr),
  stroke: none,
  inset: 4pt,
  fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
  table.header(
    text(weight: "bold")[Option],
    text(weight: "bold")[Bitset],
    text(weight: "bold")[SlotMap],
    text(weight: "bold")[BumpSlab],
    text(weight: "bold")[GapTracker],
    text(weight: "bold")[Queue],
  ),
  [A: `FastReceiver`], [x], [x], [], [], [],
  [B: `LiteReceiver`], [], [], [], [x], [],
  [C: `PackedReceiver`], [x], [], [x], [], [],
)

- *Bitset* — fixed-size bit array for slot occupancy tracking.
- *SlotMap* — fixed-size circular slot array indexed by `seq % N`.
- *BumpSlab* — append-only byte arena, reset on delivery.
- *GapTracker* — sorted non-overlapping interval set for byte-range tracking.

== Option A: Indexed Slots (`FastReceiver`)

Fixed-size slots indexed by `seq % WIN`. Each slot holds one
MTU-sized payload. O(1) insert, O(1) delivery.

```rust
struct FastReceiver<const WIN: usize, const MTU: usize, ...> {
    base: ReceiverBase,
    occupied: Bitset<WIN>,
    slots: SlotMap<WIN, MTU>,
    flags: [SequenceFlag; WIN],
    reassembly: [u8; REASM],
}
```

Static memory: `WIN × MTU` (reorder) + `REASM` (reassembly).

== Option B: Gap-Tracked Contiguous Buffer (`LiteReceiver`)

A single buffer where segments are placed at their final byte
offset (`(seq − base_seq) × MTU`). A sorted interval list tracks
missing byte ranges. Lowest memory: reorder and reassembly share
one buffer.

```rust
struct LiteReceiver<const REASM: usize, const WIN: usize, const MTU: usize> {
    base: ReceiverBase,
    message_buf: [u8; REASM],
    gaps: GapTracker<WIN>,
    message_ends: Vec<usize, WIN>,
}
```

Delivery is deferred via `pending_shift` to avoid unnecessary
buffer shifts. Static memory: `REASM` only.

== Option C: Indexed Slab (`PackedReceiver`)

Slots indexed by `seq % WIN` (like A) backed by an append-only
slab instead of fixed MTU-sized arrays. No padding waste. Default
backend (`ReceiverMachine` alias).

```rust
struct PackedReceiver<const WIN: usize, const BUF: usize, const REASM: usize> {
    base: ReceiverBase,
    occupied: Bitset<WIN>,
    slot_meta: [SlotMeta; WIN],
    slab: BumpSlab<BUF>,
    reassembly: [u8; REASM],
}
```

Static memory: `BUF` (reorder slab) + `REASM` (reassembly).

== Backend Comparison

#table(
  columns: (1.5fr, 1fr, 1fr, 1fr),
  stroke: none,
  inset: 4pt,
  fill: (_, y) => if y == 0 { luma(220) } else if calc.rem(y, 2) == 1 { luma(240) } else { white },
  table.header(
    text(weight: "bold")[Property],
    text(weight: "bold")[A: Fast],
    text(weight: "bold")[B: Lite],
    text(weight: "bold")[C: Packed],
  ),
  [In-order insert], [O(1)], [O(1)], [O(1)],
  [OOO insert], [O(1)], [O(WIN)], [O(1)],
  [Padding waste], [MTU − payload], [None], [None],
  [Contiguous delivery], [Only if no wrap], [Always], [Never],
)

== Delivery Token

The receiver API uses a type-state pattern for zero-copy delivery:

+ `wait_for_message` takes `&mut self` — prevents double receive.
+ The token holds `&mut RxHandle` but does _not_ hold a borrow.
  The driver keeps receiving while the token is held.
+ `consume` takes `self` with a synchronous `FnOnce(&[u8])`
  closure — the borrow is held only for the duration of the copy.

This eliminates double receive, consuming without waiting, and
holding the buffer borrow across `.await` at compile time.
