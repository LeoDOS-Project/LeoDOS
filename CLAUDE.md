- Commit changes incrementally — each commit should be a single logical change

## Design plans

`.claude/plans/` contains design documents for future work.
Check there before starting larger changes — a plan may
already exist.

## TODO

### Bugs

- [x] `Router::next_hop` routes `Address::Ground` immediately
  to `Direction::Ground`, but the ground station may not be
  directly reachable — fixed with LOS-based gateway routing.

- [ ] `Router::poll_links` uses `select_biased!` — all
  `DatalinkReader::read()` impls must be cancel safe (atomic
  read or no partial consumption on drop).
- [x] RouterService (client/driver) copies data through
  LocalChannel instead of zero-copy like SRSPP direct use.
  — Replaced with generic RouterDriver<D: DatalinkRead +
  DatalinkWrite>; RouterClient/RouterService removed.

### Stack cleanup (in progress)

- [x] Step 1: Simplify FrameReader — replace `next()` with
  `data_field()`, move SpacePacket extraction to LinkReader
- [x] Move SpacePacket from `network::spp` to `datalink::spp`
  — `network::spp` re-exports for backward compat.
- [x] Step 2: Router as NetworkWriter/NetworkReader
- [x] Step 3a: Unified method names — all layer traits now
  use write/read. TransportSender/Receiver renamed to
  TransportWriter/Reader.
- [x] Step 3b: Traits use *Write/*Read, structs use
  *Writer/*Reader. LinkWriter→DatalinkWriter, etc.
- [x] Fix spacecomp app — updated error types for
  Router-as-NetworkWriter pattern.
- [x] Remove CfsLinkError — use CfsError directly.
- [x] Rename routing Error → RouterError, srspp Error →
  TransportError.

### Future improvements

- [x] Reactor: write-readiness support — `register_write`
  registers an fd into a parallel `write_fds`; `block`
  passes both read and write sets to `OS_SelectMultiple`;
  `UdpSocket::send` uses it on `QueueFull`.

- [ ] Reactor: persistent `FdSet` with per-leaf wakers.
  Today every poll cycle rebuilds the set from scratch
  (each leaf re-registers on every `Pending`). A more
  efficient design: leaves register once with their own
  waker, reactor holds an `FdSet` across cycles and fires
  only the ready leaf's waker so unrelated leaves are
  not re-polled. Needs deregister-on-drop bookkeeping.
  Worth doing together, since per-leaf wakers is what
  makes selective wakeup possible.

- [ ] sch_lab-fed wakeup for SB-only Rust apps. The
  Runtime's reactor already parks socket apps in
  `OS_SelectMultiple`, but SB-only apps (sb_echo,
  spacecomp_wildfire, telemetry/HK apps) have no fd to
  register and fall back to `OS_TaskDelay(50ms)` — 20
  polls/sec per app. Instead, let the Runtime subscribe
  a pipe to a sch_lab wake-up MsgID and
  `CFE_SB_ReceiveBuffer(pipe, PEND_FOREVER)` when no fds
  are registered. The OS mqueue blocks the task until
  sch_lab fires → 0% idle CPU, canonical cFS pacing.
  API sketch: `Runtime::new().with_wakeup(mid)`. Mixed
  apps (router) still use select_multiple on UDP +
  short timeout to also drain SB pipes.

- [ ] Rebuild sb_echo + spacecomp_wildfire against the
  new libcfs. Current build only refreshed ping/router;
  the other Rust apps still link the old spinning
  Runtime. Either force their cargo_build to rebuild
  (touch/clean) or workspace-ify so libcfs changes
  invalidate them automatically.

- [ ] Move cFS Rust apps into a Cargo workspace. Each
  app currently has its own `Cargo.toml` and per-app
  `CARGO_TARGET_DIR`, so common deps (leodos-protocols,
  leodos-libcfs, aes-gcm, futures, ...) are recompiled N
  times per build. A shared workspace target would cut
  full builds from ~30 min to ~5.

- [ ] Declare `nos3_cfe` cfg in leodos-libcfs so cargo
  stops emitting `unexpected cfg condition name` warnings
  on every rustc invocation. Add
  `println!("cargo::rustc-check-cfg=cfg(nos3_cfe)");` to
  the crate's build.rs, or a `[lints.rust] unexpected_cfgs`
  entry in Cargo.toml.

- [ ] Heap-allocate SrsppNode buffers in ping/router —
  the receiver/sender buffer arrays overflow the 65 KB
  default cFE task stack, which forced us to bump
  PING_APP's stack to 524 KB in `cpu1_cfe_es_startup.scr`.
  libcfs has a heap allocator now; move big buffers onto
  it so stack sizes stay reasonable.

- [ ] RingBuffer front-drop policy — add a `front_drop: bool`
  field so the router can evict the oldest packet instead
  of dropping incoming. Useful for telemetry freshness on
  congested links. SRSPP retransmits either way.
- [ ] SB send ceremony — `SendBuffer::new` → `.view()` →
  `.init()` → fill → `.send()` is 5 steps. Add a typed
  publish helper, e.g. `pipe.publish(msg_id, &payload)?`.
- [ ] Closure-based write API — `write` traits take `&[u8]`
  which can't represent RingBuffer wrap-around (two
  disjoint slices). A closure API like
  `fn write(&mut self, len, f: impl FnOnce(&mut [u8]))`
  would let the caller write directly into the target
  buffer, avoiding the copy. Affects FrameWrite::push,
  DatalinkWrite::write, NetworkWrite::write.


- [ ] Tile geo-registration — collector captures GPS nadir
  at image time and includes lat/lon + GSD in each
  TileHeader. Enables mapper/reducer to geo-locate
  hotspots precisely, and supports shuffling tiles to
  multiple mappers (each tile self-contained).
- [ ] Job AOI propagation — pass Job's GeoAoi to the
  SpaceComp trait methods so the collector can check GPS
  against the AOI before capturing (like the standalone
  wildfire app does).
- [ ] Tile source coding — Rice-encode quantized delta
  temperatures in DualBandTile::write_to for ~2-3×
  compression on the ISL.

### NOS3 network simulation extensions

The NOS3 demo currently runs as software-in-the-loop with all
ISL links permanently up and zero latency. To make it useful
for testing distributed/network aspects of LeoDOS, the
following extensions are needed. Ordered by value per effort.

- [ ] LOS-gated ISL links — biggest gap. All ISL UDP links
  are currently permanently up. Build a `leodos-topology`
  sidecar that subscribes to 42's orbital state (via
  truth42sim's NOS Engine bus), computes pairwise LOS for
  the torus neighbors, and blocks packets on closed links
  (iptables rules or a per-link UDP proxy). Unlocks testing
  of routing convergence, SRSPP retransmission under churn,
  and DTN-style stored-and-forward paths.

- [ ] LOS-gated ground link — ground UDP link is always up.
  Same sidecar should compute LOS between each satellite and
  each ground station in the `GatewayTable`, dropping ground
  link packets when out of view. Validates gateway handover
  and the `DistanceMinimizing` algorithm's station selection.

- [ ] Per-link netem — `tc qdisc` with port classifiers to
  add delay, jitter, loss, and bandwidth limits per ISL link.
  Can be parameterized by distance from 42's state for
  realistic LEO variation (few ms propagation, 100 Mbps-1 Gbps).
  No code needed, just shell config. Exercises SRSPP window
  sizing and recovery under realistic conditions.

- [ ] Per-satellite clock drift — currently all satellites
  share one time driver. Add per-SCID offset in the time
  driver (or run one time driver per satellite with slightly
  different tick rates). Tests cFE time sync code that's
  otherwise not exercised.

- [ ] Partition injection — once LOS gating is in place,
  deliberately cause network partitions by forcing LOS
  thresholds or blocking link groups. Tests DTN storage
  paths and application-layer resilience.

- [ ] Multi-host scaling — NOS Engine speaks TCP, so it can
  span hosts. Run one FSW container per satellite on a
  Docker Swarm or Kubernetes cluster. Unblocks scale testing
  beyond ~20 satellites on a single laptop and gives real
  process isolation (one satellite's crash can't affect
  another's memory). Real work is orchestration and
  inter-host network routing, not the sim itself.

- [ ] Orbital topology snapshots — precompute the full
  contact plan offline (using 42 or a standalone tool like
  pyorbital) and feed it to the topology sidecar as a
  time-indexed table. Faster than recomputing LOS every
  tick, and matches how real mission planning works.

Prior art to learn from:
- Hypatia (Kassing et al., 2020) — ns-3-based LEO network
  simulator with time-varying topology. Not flight-software-
  in-the-loop, but the topology engine is what we'd replicate.
- Celestial / LeoEM — similar research-grade simulators.

### Missing CCSDS protocols

Protocols from CCSDS 130.0-G-4 not yet implemented:

- [ ] 734.1-B — Licklider Transmission Protocol (LTP): convergence
  layer for Bundle Protocol over lossy links. Needed for
  full BP/DTN support.
- [x] 355.1-B — SDLS Extended Procedures: key management and
  security association negotiation for SDLS.
- [x] 211.1-B — Proximity-1 Physical Layer: GMSK params, UHF
  bands, data rates. `physical::proximity1`.
- [x] 211.2-B — Proximity-1 Coding and Sync Sublayer: pipeline
  composition (randomizer + convolutional + 24-bit ASM).
  `coding::proximity1`.
- [x] 122.1-B — Spectral Preprocessing Transform: upshift,
  downshift, IWT (5-level CDF 5/3). `coding::compression::spectral`.
  POT and AAT transforms not yet implemented.
- [ ] IP over SRSPP — carry IP datagrams over SRSPP for
  constellation-wide IP connectivity. Similar to IPoC
  (702.1-B) but using SRSPP for reliable delivery instead
  of raw Encapsulation Packets over data link frames.

Note: CCSDS 352.0-B (Cryptographic Algorithms) specifies
AES-GCM 128/256 which we already cover via the `aes-gcm`
crate. No custom crypto implementation needed.

## Communication stack composition

Six layers stitch the stack together:

```
Application                              (application/)
  Source coding: Rice, DWT, hyperspectral
  Compressor / Decompressor traits
        ↕
TransportWrite / TransportRead           (transport/)
  SRSPP, CFDP, Bundle Protocol
        ↕
NetworkWrite / NetworkRead               (network/)
  Router, PointToPoint
        ↕
DatalinkWrite / DatalinkRead             (datalink/link/)
  DatalinkWriter<F, W, S>:
    FrameWrite → SecurityProcessor → CodingWrite
  DatalinkReader<F, R, S>:
    CodingRead → SecurityProcessor → FrameRead
        ↕
CodingWrite / CodingRead                 (coding/)
  Randomizer → FEC → Framer (pipeline.rs)
        ↕
AsyncPhysicalWriter / AsyncPhysicalReader (physical/)
```

### DatalinkWriter composition

`DatalinkWriter<F, W, S>` composes:
- `F: FrameWrite` — TM/TC/AOS/USLP frame construction
- `S: SecurityProcessor` — SDLS (AES-GCM) or `NoSecurity`
- `W: CodingWrite` — coding pipeline to physical layer

Builder via `bon`:
```rust
DatalinkWriter::builder()
    .frame_writer(TmFrameWriter::new(config))
    .security(SdlsProcessor::new(sa, crypto, 6))
    .coding_writer(CodingWriter::new(...))
    .build()
```

### What composes

- [x] FrameWriter/FrameReader ↔ CodingWriter/CodingReader:
  Composed via DatalinkWriter/DatalinkReader in
  datalink/link/framed.rs.
- [x] SDLS security in DatalinkWriter — `SecurityProcessor`
  applied between frame construction and coding.
- [x] COP-1 state machines (FARM/FOP) — fully implemented
  in datalink/reliability/cop1/.
- [x] Coding wrappers — AsmWriter, CltuWriter, FrameSyncReader
  exist; randomizer/RS are inline in CodingWritePipeline.
- [x] Modulation — standalone for testing only, not part
  of the composition chain (real HW radio handles it).

## Coding/FEC primitives (all implemented)

- [x] SDLS crypto — AES-GCM 128/256 (CCSDS 355.0-B-2)
- [x] Modulation — BPSK/QPSK modulate, demodulate, LLR output
- [x] Physical/NOS3 bridge — Docker Compose + Makefile targets
- [x] LDPC — encoder, decoder, syndrome check (all 6 codes)
- [x] Reed-Solomon (255,223)
- [x] CADU / ASM frame sync
- [x] CLTU encoding
- [x] Pseudo-randomizer
- [x] Rice coding — CCSDS 121.0-B-3 lossless data compression
- [x] CCSDS 123.0-B-2 — lossless multispectral/hyperspectral image compression
- [x] CCSDS 122.0-B-2 — wavelet-based image data compression (integer 5/3 DWT)

## NOS3 component bindings TODO

Generate bindgen bindings + safe Rust wrappers in `leodos-libcfs`
(behind the `nos3` feature flag) for these NOS3 simulator components.
Everything except CryptoLib (we already have SDLS in Rust).

- [x] generic_radio — ground/ISL RF link simulation (bindings)
- [x] generic_eps — electrical power system (bindings)
- [x] generic_adcs — attitude determination & control (bindings)
- [x] generic_css — coarse sun sensor (bindings)
- [x] generic_fss — fine sun sensor (bindings)
- [x] generic_imu — gyroscope/accelerometer (bindings)
- [x] generic_mag — magnetometer (bindings)
- [x] generic_star_tracker — attitude determination (bindings)
- [x] generic_reaction_wheel — momentum/torque control (bindings)
- [x] generic_torquer — magnetic desaturation (bindings)
- [x] generic_thruster — orbit/attitude maneuvers (bindings)
- [x] novatel_oem615 — GPS receiver (bindings)
- [x] arducam — camera/imaging (bindings)
- [ ] nos_time_driver — synchronized simulation clock (C++, skip)
- [ ] truth_42_sim — orbital mechanics (C++, skip)

Each component has `*_msg.h` (command/telemetry structs) and
`*_device.h` (device driver API) headers to bind.
Safe Rust wrappers still needed for all 13 components.

## Docker testing

Run `make docker-build` once to build the image, then
`make docker-test` to run `cargo test --features=cfs` for
`leodos-protocols` inside a Linux container. It auto-preps
the cFS build directory if needed.

## NOS3 simulation

Uses `ivvitc/nos3-64` Docker image (NOS Engine SDK pre-installed).

```
make nos3-build      # build Docker image (adds Rust toolchain)
make nos3-config     # generate NOS3 build configuration
make nos3-build-sim  # build simulators (C++)
make nos3-build-fsw  # build flight software (cFS + Rust)
make nos3-launch     # start all containers
make nos3-stop       # stop all containers
make nos3-shell      # interactive shell in FSW container
```

The hwlib sim sources (`libs/nos3/fsw/apps/hwlib/sim/src/`)
implement the same bus API (uart, i2c, spi, can) but route
through NOS Engine TCP instead of real hardware. Our Rust
wrappers work transparently — no code changes needed.
