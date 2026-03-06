- Commit changes incrementally — each commit should be a single logical change

## Missing layers (between DataLink and NOS3)

- [x] SDLS crypto — AES-GCM 128/256 (CCSDS 355.0-B-2)
- [ ] Modulation — symbol-level software model
- [ ] Physical/NOS3 bridge — connecting to NOS Engine bus
- [x] LDPC — encoder, decoder, syndrome check (all 6 codes)
- [x] Reed-Solomon (255,223)
- [x] CADU / ASM frame sync
- [x] CLTU encoding
- [x] Pseudo-randomizer

## Docker testing

Run `make docker-build` once to build the image, then
`make docker-test` to run `cargo test --features=cfs` for
`leodos-protocols` inside a Linux container. It auto-preps
the cFS build directory if needed.
