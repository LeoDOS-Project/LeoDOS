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

- [ ] RingBuffer front-drop policy — add a `front_drop: bool`
  field so the router can evict the oldest packet instead
  of dropping incoming. Useful for telemetry freshness on
  congested links. SRSPP retransmits either way.
- [ ] Closure-based write API — `write` traits take `&[u8]`
  which can't represent RingBuffer wrap-around (two
  disjoint slices). A closure API like
  `fn write(&mut self, len, f: impl FnOnce(&mut [u8]))`
  would let the caller write directly into the target
  buffer, avoiding the copy. Affects FrameWrite::push,
  DatalinkWrite::write, NetworkWrite::write.


## Communication stack composition

Five trait boundaries stitch the stack together:

```
TransportSender / TransportReceiver    (transport/)
        ↕
NetworkLayer                           (network/)
        ↕
DataLink = FrameSender + FrameReceiver (datalink/link/)
        ↕
  ??? gap — trait mismatch ???
        ↕
AsyncPhysicalWriter / AsyncPhysicalReader (physical/)
```

### Actual type composition (what exists today)

Transport holds `L: NetworkLayer`, calls `link.send()`:
  SrsppSender<Router<N,S,E,W,G,L,R>>

Network holds `D: DataLink` per direction:
  Router<UdpDataLink, UdpDataLink, ..., PassThrough<UdpDataLink>>

DataLink drivers hold `W: FrameSender`:
  TmSenderDriver<UdpFrameSender>
  TcSenderDriver<UdpFrameSender>

Physical has `UartChannel` (behind `cfs` feature):
  UartChannel wraps hwlib Uart

Coding has standalone encode/decode functions + composable
wrappers that impl AsyncPhysicalWriter/AsyncPhysicalReader:
  RandomizerWriter<RsWriter<AsmWriter<UartChannel>>>

### What does not compose yet

- [x] FrameWriter/FrameReader ↔ CodingWriter/CodingReader:
  Composed via LinkWriter/LinkReader in datalink/link/channel.rs.
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
