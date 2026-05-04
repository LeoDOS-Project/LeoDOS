# Constellation simulator: walker-delta + LeoDOS

## Goal

Run real cFS flight software for a constellation of LEO
satellites driven by walker-delta as both controller and
topology source. Walker-delta owns propagation, orbital
parameters, time, and statistics; LeoDOS runs the cFS apps
**unchanged** and consumes orbital state through existing
cFS abstractions.

Replaces NOS3 wholesale: no NE Engine, no per-component sims,
no per-spacecraft docker containers.

## Design rule

**Backends, not new interfaces.** Walker-delta integration is
purely a build-configuration choice that swaps backend
implementations for cFS abstractions that **already exist**.
We do not introduce a `platform::*` layer, an `EventPort`
trait, a `LosSource` trait, or any other new Rust interface.
We do not use `Box<dyn ...>` or runtime dispatch. App source
is byte-identical between flight and sim.

Mapping what we need onto what already exists:

| Bridge concern   | Existing cFS abstraction |
|------------------|--------------------------|
| Time             | PSP (1Hz tone source)    |
| Position / nadir | hwlib GPS / star-tracker drivers |
| LOS / link state | hwlib ISL link-status driver |
| Sensor data      | hwlib (thermal cam, mag, IMU, …) |
| Telemetry events | `CFE_EVS_SendEvent` → TO Lab → UDP downlink |

For each row, walker-delta provides a backend implementation;
cFS apps continue to use the existing API. The bridge is what
sits *underneath* hwlib and the PSP — never above them.

## Architecture

```
[Mac host]                                     [Docker container]

  walker-delta                                   ┌─ cFS_0 (sat(0,0))
   ─ SGP4 propagator (already exists)            ├─ cFS_1 (sat(0,1))
   ─ GUI: orbital params, sat count,             ├─ ...
     propagation rate, sim time control          └─ cFS_8 (sat(2,2))
   ─ stats dashboard
   ─ event renderer (3D view overlays)
                                  UDP unicast
                                 ───port 7000──►   topology backend
                                                   (inside hwlib /
                                                    custom PSP)

                                 ◄──port 1235──    TO Lab TM downlink
                                                   (already today)
```

Walker-delta sends one direction (state → cFS) over a new
port; receives the other direction (events ← cFS) over the
existing TM downlink. No new uplink protocol, no new event
protocol — both already exist in cFS.

## Wire format (state push only)

Defined in walker-delta as a `bridge.rs` module. LeoDOS side
carries a matching module. When stable, extract to a shared
crate (path-dependency from walker-delta into LeoDOS).

Stable byte layout via `#[repr(C)]` + zerocopy. No serde, no
allocation, no `dyn`.

```rust
const STATE_MAGIC: [u8; 4] = *b"LEOS";
const BRIDGE_VERSION: u16 = 1;
const TOPOLOGY_PORT: u16 = 7000;
const MAX_SATS: usize = 1024;

#[repr(C)]
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
struct StateHeader {
    magic: [u8; 4],
    version: U16,
    seq: U32,
    sim_time_ms: U64,
    real_time_ms: U64,
    num_sats: U16,
    _pad: [u8; 2],
}

#[repr(C)]
#[derive(FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
struct SatState {
    scid: U32,
    pos_eci_m: [F64; 3],
    vel_eci_m_s: [F64; 3],
    nadir_quat: [F64; 4],
    los_neighbors: u8,        // bitmask: N=0,S=1,E=2,W=3
    los_ground: U16,          // bitmask of gateway IDs in view
    _pad: [u8; 5],
}
```

The reverse direction (cFS → walker-delta) reuses the
existing TM downlink — walker-delta listens on the same UDP
port that COSMOS uses today, decodes Software Bus telemetry,
extracts events.

## What changes per side

### walker-delta

Pure binary; nothing depends on it. New `bridge.rs` module
adds:
- `StatePacket` builder fed by the existing propagator.
- UDP send loop pumping `StatePacket` to LeoDOS at the
  configured tick rate.
- TO Lab UDP listener that decodes incoming TM packets and
  feeds the event-rendering pipeline.
- 3D view overlays: arrows for ISL traffic, gateway lights
  for ground contacts, link-status colors driven by LOS
  matrix.
- Stats panes: RTT histogram, throughput, contact %.

### LeoDOS — backend swaps only

Each row is a build-time switch with no app-source change:

1. **PSP**: `nos-linux` → upstream `pc-linux`.
   No NE Engine. POSIX clock. CMake target swap.

2. **hwlib for GPS**: new backend that reads from the bridge
   topology stream and converts ECI → lat/lon/alt. Same
   `hwlib_gps_*` symbols, different `.so`.

3. **hwlib for ISL link status**: new backend that returns
   the `los_neighbors` bitmask from the bridge stream.
   Router consults the existing hwlib API.

4. **hwlib for thermal cam, magnetometer, star tracker**:
   topology-backed implementations. The few components
   wildfire and friends actually link against.

5. **null hwlib for everything else**: components that any
   loaded app might link against but doesn't actually use.
   Returns zeros / constants. Drop-in `.so`.

6. **Build configuration**: new `bridge` Cargo profile /
   feature in the affected crates that selects the bridge
   hwlib `.so` set and `pc-linux` PSP. Default profile keeps
   the current NOS3 setup intact during transition.

### Bridge subscriber (lives inside the hwlib `.so`s)

A small subscriber inside the bridge-flavored hwlib reads
UDP `:7000`, deserializes `StatePacket`, picks the entry
matching its own SCID, and stashes it in a static cell.
Each `hwlib_gps_*` / `hwlib_isl_link_status` / etc. call
reads from the cell. Cheap, stateless, no allocation.

A single subscriber thread per cFS process. No fanout, no
broker. The hwlib `.so` owns it; apps never see it.

## Time

Real-time first. Walker-delta runs at wall-clock; cFS uses
`pc-linux` PSP unmodified.

Sim-time (paused, scrub, faster-than-real) is a phase 2: a
custom PSP that uses `sim_time_ms` from the bridge stream
as the time source. `OS_TaskDelay` and `CFE_TIME` follow
the same clock. Apps still don't change.

## Build order

1. Wire format module in walker-delta. Round-trip
   serialize/deserialize test.
2. Walker-delta state publisher. UDP send loop driven by
   the existing propagator. Verify with `nc -ul 7000 | xxd`.
3. Switch one cFS app build from `nos-linux` PSP to
   upstream `pc-linux`. Confirm ping demo still works
   without any walker-delta connection. Validates the PSP
   swap as a standalone change.
4. Bridge-flavored hwlib for GPS. Subscriber + ECI→lla
   conversion. Wildfire's AOI check now uses real
   topology-derived position.
5. Bridge-flavored hwlib for ISL link status. Router
   gates on it via existing API.
6. Walker-delta TO Lab TM listener. Decodes telemetry,
   extracts CFE_EVS events, feeds 3D viz.
7. View overlays: link arrows, gateway lights, contact
   schedule.
8. Stats dashboard.
9. (Phase 2) Sim-time PSP. Walker-delta becomes the time
   source.
10. (Phase 2) Determinism / replay. Log the bridge stream
    + TM stream. Replay scenarios offline.

## What we explicitly skip

- New Rust traits, `platform::*` modules, `Box<dyn>` boundaries.
- Component sims (IMU, EPS, FSS, …) for hardware our apps
  don't use.
- 42 attitude dynamics. Walker-delta publishes nadir-pointing
  quaternion directly.
- NOS Engine.
- Per-spacecraft docker containers.
- Sensor noise / fault injection. Apps see clean
  topology-derived telemetry.

## Where existing apps slot in

All apps remain byte-identical at the source level.

- **Router** (`apps/router`): unchanged. Calls `hwlib`
  ISL-link-status; bridge backend feeds it walker-delta data.
- **Ping** (`apps/ping`): unchanged.
- **SB Echo** (`apps/sb_echo`): unchanged.
- **Wildfire** (`apps/spacecomp_wildfire`): unchanged. Calls
  `GeoCamera::capture(...)`; bridge-flavored hwlib services
  GPS + thermal cam from walker-delta.
- **leodos-protocols**: `Router` consults `hwlib` link
  status (already its abstraction); unchanged from app
  perspective. May gain an internal helper but no new
  external trait.
- **leodos-libcfs**: gains the bridge subscriber inside
  the bridge hwlib `.so`. Existing `cfe::*` modules
  unchanged.
- **leodos-ground**: optionally subscribes to walker-delta's
  state stream so the user can drive ping demos with
  knowledge of which sat is currently in view.

## Open questions

- **Process count vs. in-process cFS.** Past ~500 cFS
  processes per container, OS-thread limits bite. The
  "all sats in one process" alternative needs significant
  cFE rework — defer unless single-process scaling fails.
- **Cross-host scale-out.** Single-host today. Same wire
  format works cross-host with no protocol changes. Phase 3.
- **Sim-time clock skew across cFS processes.** When walker-
  delta drives sim time, all cFS instances must read the
  same clock. With one subscriber per process pulling from
  the same UDP stream, sequential consistency is bounded by
  network jitter. For sub-millisecond sync, would need
  shared memory — not justified yet.
