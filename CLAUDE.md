- Commit changes incrementally — each commit should be a single logical change

## Missing layers (between DataLink and NOS3)

- [x] SDLS crypto — AES-GCM 128/256 (CCSDS 355.0-B-2)
- [x] Modulation — BPSK/QPSK modulate, demodulate, LLR output
- [ ] Physical/NOS3 bridge — connecting to NOS Engine bus
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
