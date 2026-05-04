# Constellation simulator: walker-delta + LeoDOS

## Goal

Run real cFS flight software for a constellation of LEO
satellites driven by walker-delta (the existing visualizer) as
both controller and topology source. Walker-delta owns
propagation, orbital parameters, time, and statistics;
LeoDOS runs the cFS apps unchanged and consumes orbital state
to drive its routing, ISL gating, and (eventually) hwlib
sensor backends.

Replaces NOS3 wholesale: no NE Engine, no per-component sims,
no per-spacecraft docker containers, no `<42-css-scale-factor>`.

What we keep: real cFE/SB/ES/TIME semantics, real packets on
real UDP sockets, real LOS-gated ISLs, real ground contacts.
What we trade away vs NOS3: realistic sensor noise,
hardware-fault injection, IV&V-grade per-component fidelity.

## Constraints

- Single host. Mac for walker-delta, Docker for LeoDOS
  (cFS doesn't run native on macOS).
- Walker-delta is a binary — nothing depends on it. LeoDOS
  apps and walker-delta exchange bytes via UDP across the
  Mac↔container boundary, using existing docker port-forward
  pattern (already proven by `leodos-ground`).

## Architecture

```
[Mac host]                                       [Docker container]

  walker-delta                                     ┌─ cFS_0 (sat(0,0))
   ─ SGP4 propagator (already exists)              ├─ cFS_1 (sat(0,1))
   ─ GUI sliders: altitude, inclination,           ├─ ...
     RAAN spacing, sat count, propagation rate     └─ cFS_8 (sat(2,2))
   ─ stats dashboard
   ─ event renderer (3D arrows on link activity)
                                  UDP unicast
                                  ───port 7000──►   topology in
                                                    (each cFS subscribes)

                                  ◄──port 7001──    events out
                                                    (each cFS publishes)
```

Walker-delta is the truth source. cFS apps consume what they're
told and react. Inverting the previous plan's "topology process
is a separate Rust binary" — walker-delta already does
SGP4 + LOS + the GUI. Don't duplicate that.

## Wire format

Defined in walker-delta first (as a `bridge.rs` module). LeoDOS
side carries a matching module. When the protocol stabilizes,
extract to a shared `leodos-bridge` crate referenced via path
dependency from walker-delta.

Stable byte layout via `#[repr(C)]` + zerocopy. No serde;
fixed-size packets only.

```rust
// Magic + version are first so misrouted packets can be
// rejected cheaply at the receiver.

const STATE_MAGIC: [u8; 4] = *b"LEOS";
const EVENT_MAGIC: [u8; 4] = *b"LEOE";
const BRIDGE_VERSION: u16 = 1;

const TOPOLOGY_PORT: u16 = 7000;   // walker-delta → LeoDOS
const EVENT_PORT:    u16 = 7001;   // LeoDOS → walker-delta

const MAX_SATS: usize = 1024;      // upper bound; packet is
                                   //   variable-length up to this

#[repr(C)]
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
struct StateHeader {
    magic: [u8; 4],
    version: U16,
    seq: U32,                  // monotonic, walker-delta-side
    sim_time_ms: U64,          // since epoch (sim clock)
    real_time_ms: U64,         // wall clock at publish
    num_sats: U16,
    _pad: [u8; 2],
}

#[repr(C)]
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
struct SatState {
    scid: U32,
    pos_eci_m: [F64; 3],       // meters
    vel_eci_m_s: [F64; 3],     // m/s
    nadir_quat: [F64; 4],      // body→ECI
    los_neighbors: u8,         // bitmask: N=0,S=1,E=2,W=3
    los_ground: U16,           // bitmask of gateway IDs in view
    _pad: [u8; 5],
}

// Wire packet:  StateHeader  ‖  SatState[num_sats]

#[repr(C)]
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
struct EventHeader {
    magic: [u8; 4],
    version: U16,
    scid: U32,                 // emitter
    real_time_ms: U64,
    kind: u8,                  // EventKind tag
    _pad: [u8; 1],
}

#[repr(u8)]
enum EventKind {
    PacketSent  = 1,           // Body: PacketEvent
    PacketRecv  = 2,
    AckSent     = 3,
    Retransmit  = 4,
    GroundOpen  = 5,           // Body: GroundEvent
    GroundClose = 6,
    AppLog      = 7,           // Body: LogEvent
}

#[repr(C)]
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
struct PacketEvent {
    peer_scid: U32,            // dst on send, src on recv
    link: u8,                  // Direction: 0=N 1=S 2=E 3=W 4=Ground
    apid: U16,
    len: U16,
    _pad: [u8; 1],
}
```

Bandwidth math:
- 9 sats: header (32B) + 9 × SatState (~80B) ≈ 750 B/packet.
  At 100 Hz that's ~75 kB/s. Trivial.
- 100 sats: ~8 kB/packet × 100 Hz = ~800 kB/s. Fine.
- 1000 sats: ~80 kB/packet — over the typical UDP MTU. Either
  reduce tick rate, drop irrelevant sats per receiver, or split
  into multiple sub-packets. Cross that bridge if it gets there.

## How LeoDOS receives state

Pattern: each cFS process binds UDP `:7000` (or rather, joins a
fanout port — see below) and runs a small "topology
subscriber" loop that reads `StatePacket`, finds its own SCID's
slice, and exposes it to:

- **Router**: consult `los_neighbors` before sending on each
  ISL link. If bit is clear, drop or queue (DTN later).
- **Topology-driven hwlib backend** (new): GPS reads return
  `pos_eci → lla` conversion. Magnetometer reads return IGRF
  field at nadir. Star tracker returns `nadir_quat`. Wildfire's
  AOI check uses real positions instead of stubs.

Two wiring options for the Mac↔container hop:

a) **Each cFS process binds its own UDP port mapped through
   docker** (e.g. cFS_N binds container:7000 + N*100, walker-
   delta sends a tailored per-sat packet to each). Simpler in
   the short term — no fanout process. Walker-delta sends N
   small packets per tick instead of one fat one.

b) **One topology fanout process inside the container**
   listens on `:7000`, parses, and republishes per-sat state
   to local UDP/SB pipes. Each cFS subscribes locally. One
   fat packet per tick across the bridge; fanout is internal.

Start with (a) — it's the same pattern leodos-ground already
uses. Move to (b) only if walker-delta's per-sat-send loop
becomes a measurable bottleneck (probably never at single-
host scale).

## How walker-delta receives events

Each cFS process publishes `EventPacket` to UDP
`host.docker.internal:7001` (or the host's IP). Walker-delta
listens, decodes, and feeds into:

- 3D viewport overlay: arrow pulse on the relevant link when
  `PacketSent`/`PacketRecv` arrives. Color by kind (data
  green, ack blue, retransmit red).
- Statistics dashboard: rolling RTT histogram from
  Send/Recv timestamps; throughput by link; retransmit
  rate; gateway uptime; per-sat event counter.
- Time-series log for replay: write events + state to disk;
  later "scrub" through history without re-running the sim.

## What changes on each side

### walker-delta

- Add `bridge.rs` module: wire format + UDP send loop +
  receive loop.
- Hook the existing propagator into `StatePacket` building.
  Walker-delta already produces ECI position/velocity for
  every animated sat; reuse that.
- Add LOS computation for the torus neighbors (it visualizes
  ISLs already; the data is there).
- Add the event-stream consumer + viz overlays.
- Add stats panes (dockable egui plots, bridge already uses
  egui_plot).

### LeoDOS

- New `crates/leodos-bridge` (eventually) with the matching
  wire format. Initially just a module copied alongside
  walker-delta's.
- `topology_subscriber` task in each cFS process that owns
  the UDP recv loop and stashes per-sat state for hwlib +
  router to consume.
- Router: consult `los_neighbors` in `next_hop` / `forward`.
- New `null_hwlib` backend for components LeoDOS apps actually
  use (today: GPS for wildfire, plus stubs for any others
  that link). Backend reads from `topology_subscriber`.
- Event publisher: small `EventPort` helper that any app or
  protocol layer can call to emit `EventPacket`s. Wire it
  into router (link sends), SRSPP (acks/retransmits), and
  ground link state changes.

### Both

- Drop NOS3 docker-compose. Replace with a small launcher that
  spawns N cFS processes inside one container with the same
  env each gets today, plus a `--scid` arg.
- Switch cFS PSP from `nos-linux` to upstream `pc-linux`. No
  more NE Engine subscription. `OS_TaskDelay` and CFE_TIME
  use POSIX clocks.

## Time

Real-time first. Walker-delta's animation matches wall clock;
cFS uses `pc-linux` PSP unmodified. Both sides agree on time
because both follow real wall clock.

Sim-time (faster than realtime, paused, scrub) is a phase 2:
- Walker-delta drives a simulated clock controlled by GUI.
- cFS needs a custom PSP that uses the bridge clock instead
  of POSIX.
- `OS_TaskDelay` re-routes to the same clock.
- Determinism: log walker-delta's tick stream + cFS event
  stream → replay reproducible.

Defer until research demands it. Real-time gets us the demo,
and most routing/contact experiments don't actually need
sim-time compression to validate.

## What we explicitly skip

- Component sims (IMU, EPS, star tracker, FSS, …) for
  components LeoDOS apps don't use today.
- 42 attitude dynamics: walker-delta publishes a
  nadir-pointing quaternion derived from orbit state. Add
  full attitude only if needed.
- NOS Engine. Closed-source, single-host-shaped, source of
  every contention bug we've debugged.
- Per-spacecraft docker containers. cFS is just a Linux
  process in one container.
- Sensor noise / fault injection. Apps see clean
  topology-derived telemetry.

## Build order

1. **Wire format in walker-delta**. Define structs, encode/
   decode round-trip test, no IO yet. Paste matching module
   into LeoDOS side.
2. **Walker-delta state publisher**. Hook propagator output
   into a UDP send loop. Configurable sat count + tick rate.
   Verify with `nc -ul 7000 | xxd` from a separate terminal.
3. **LeoDOS topology subscriber**. New helper crate or module
   that any cFS process can use to get its current
   `SatState`. No app integration yet — just the plumbing.
4. **Switch from `nos-linux` PSP to `pc-linux`**. Remove
   NE Engine dependency. Verify ping-pong still works in
   docker without any walker-delta connection. This unblocks
   everything else.
5. **Router LOS gating**. Consult `los_neighbors` in `next_hop`.
   Test with a static state from walker-delta where some links
   are forced down — verify packets stop on those links and
   eventually route around.
6. **null_hwlib + GPS backend**. Wildfire's GPS reads return
   topology-derived nadir. Verify wildfire's AOI check passes
   when topology says we're over California.
7. **Event publisher + walker-delta viz**. Router emits
   `PacketSent`/`PacketRecv` per ISL hop. Walker-delta animates
   them. First "demo moment".
8. **Stats dashboard**. egui plots for RTT, throughput,
   contact %.
9. **Sim-time PSP** (deferred — only if research demands it).
10. **Determinism / replay** (deferred — only if debugging
    routing churn becomes painful).

Build order is sequential by hard dependency: 1→2→3 must
exist before any LeoDOS-side work can use bridge data; 4
unblocks the whole stack from NOS3; 5–7 is where the demo
gets compelling. 8+ are polish.

## Open questions

- **Process count vs. in-process cFS.** Eventually scaling
  past ~500 cFS processes in one container hits OS-thread
  limits. The "all sats in one process" alternative needs
  significant cFE rework — defer unless the per-process
  approach actually fails.
- **Determinism under UDP reorder.** UDP doesn't guarantee
  ordering. State packets are idempotent (latest wins via
  `seq`), but events arriving out of order would jumble the
  3D viz timeline. Either timestamp-sort with a small
  reorder buffer in walker-delta, or accept slight glitches.
- **Cross-host scale-out.** Single-host is the constraint
  today. Same wire format works cross-host with no changes;
  walker-delta would just send to a remote UDP endpoint.
  Phase 3.

## Where existing apps slot in

- **Router** (`apps/router`): unchanged except for the LOS
  gate consultation + event emission. No FSW logic changes.
- **Ping** (`apps/ping`): no changes.
- **Wildfire** (`apps/spacecomp_wildfire`): hwlib backend
  swap from NOS3 → topology-driven. AOI check works against
  real positions.
- **SRSPP / Gossip / etc.** in `crates/leodos-protocols`:
  no changes; they consume datalink interfaces that the
  router provides.
- **leodos-ground**: gains optional bridge subscription so
  the user can drive ping demos with a knowledge of which
  sat is currently in view of the gateway.
