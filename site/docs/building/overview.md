# Overview

LeoDOS has three build targets: native tests on the host, Docker-based cFS tests, and full NOS3 constellation simulation. Each target produces the same Rust flight software — only the hardware abstraction layer and test infrastructure differ.

## Native Tests

Run Rust unit and integration tests directly on the host machine without cFS or Docker:

```
cargo test
```

This tests protocol implementations, algorithms, and data structures in isolation.

## Docker Tests

Build and test the cFS-integrated flight software inside a Linux container:

```
make docker-build    # build the Docker image (once)
make docker-test     # run cargo test --features=cfs inside the container
```

This tests the Rust code linked against the real cFE, OSAL, and PSP libraries. The container auto-prepares the cFS build directory if needed.

## NOS3 Simulation

Build and run a full constellation simulation with orbital mechanics, hardware simulators, and inter-satellite communication:

```
make nos3-build      # build Docker image (adds Rust toolchain to NOS3 base)
make nos3-config     # generate NOS3 build configuration
make nos3-build-sim  # build hardware simulators (C++)
make nos3-build-fsw  # build flight software (cFS + Rust apps)
make nos3-launch     # start all containers
make nos3-stop       # stop all containers
make nos3-shell      # interactive shell in FSW container
```

For Earth observation workflow testing, generate synthetic sensor data before launching:

```
make eosim-gen       # generate thermal raster files in tools/eosim/output/
```

## Documentation Site

Build and preview the documentation site locally:

```
cd site
npm install          # install dependencies (once)
npm run start        # development server with hot reload
npm run build        # production build
```

The site deploys automatically to GitHub Pages on pushes to `main` that modify `site/`.
