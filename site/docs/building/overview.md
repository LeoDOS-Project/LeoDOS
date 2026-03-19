# Overview

All commands run from the repository root unless otherwise noted.

## Native Tests

Run Rust unit and integration tests directly on the host machine without cFS or Docker:

```
cargo test
```

This tests protocol implementations, algorithms, and data structures in isolation. The Rust crates are in `crates/`.

## Docker Tests

Build and test the cFS-integrated flight software inside a Linux container:

```
make docker-build    # build the Docker image (once)
make docker-test     # run cargo test --features=cfs inside the container
```

This tests the Rust code linked against the real cFE, OSAL, and PSP libraries. The cFS framework is in `cfe/`, `osal/`, and `psp/`. Flight apps are in `apps/`.

## NOS3 Simulation

Build and run a full constellation simulation with [orbital mechanics](/simulation/orbital-mechanics), [hardware simulators](/simulation/sensors), and [inter-satellite communication](/simulation/communication):

```
make nos3-build      # build Docker image (adds Rust toolchain to NOS3 base)
make nos3-config     # generate NOS3 build configuration
make nos3-build-sim  # build hardware simulators (C++) from libs/nos3/components/
make nos3-build-fsw  # build flight software (cFS + Rust apps) from apps/
make nos3-launch     # start all containers
make nos3-stop       # stop all containers
make nos3-shell      # interactive shell in FSW container
```

NOS3 configuration is in `libs/nos3/cfg/`. Hardware simulator sources are in `libs/nos3/components/`.

## Constellation Simulation

The `leodos-cli` tool orchestrates multi-satellite [constellation simulation](/simulation/constellation):

```
leodos-cli sim start 3 22     # start 3 orbits × 22 satellites = 66 satellites
leodos-cli sim stop           # stop all containers
leodos-cli sim shell 0        # open a shell in orbit-0 container
```

The `start` command generates a Docker Compose configuration, assigns spacecraft IDs and ports, and brings up all containers. Each satellite gets a unique ID derived from its grid position: `(orbit + 1) × 1000 + satellite_number`.

## Earth Observation Data

Generate synthetic sensor data for [workflow testing](/simulation/earth-observation) before launching the simulation:

```
cd tools/eosim
uv run eosim wildfire examples/california_wildfire.yaml -o output/ --fmt bin
```

Or from the repository root:

```
make eosim-gen       # generate thermal raster files in tools/eosim/output/
```

Scenario definitions are YAML files in `tools/eosim/examples/`. Generated rasters go to `tools/eosim/output/`, which is mounted into the simulation container.

## Documentation Site

Build and preview the documentation site locally:

```
cd site
npm install          # install dependencies (once)
npm run start        # development server with hot reload
npm run build        # production build
```

The site source is in `site/docs/`. It deploys automatically to GitHub Pages on pushes to `main` that modify `site/`.

## Rust API Documentation

Generated Rust API docs are hosted alongside the documentation site:

- [`leodos-protocols`](/rustdoc/leodos_protocols/) — the communication stack and SpaceCoMP implementation (built with `cfs` and `tokio` features)
- [`leodos-libcfs`](/rustdoc/leodos_libcfs/) — safe Rust wrappers for the cFE, OSAL, and PSP APIs

To build locally (requires Docker for cFS headers):

```
make docker-build                  # build Docker image (once)
make docker-prep                   # prep cFS headers (once)
docker compose run --rm cfs-build \
  cargo doc --manifest-path crates/leodos-protocols/Cargo.toml \
  --features=cfs,tokio --no-deps --open
```
