# Infrastructure

The simulation runs in Docker containers using the `ivvitc/nos3-64` base image (NOS Engine SDK pre-installed). The LeoDOS build adds the Rust toolchain and compiles the flight software.

## Build and Run

```
make nos3-build      # build Docker image (adds Rust toolchain)
make nos3-config     # generate NOS3 build configuration
make nos3-build-sim  # build hardware simulators (C++)
make nos3-build-fsw  # build flight software (cFS + Rust apps)
make nos3-launch     # start all containers
make nos3-stop       # stop all containers
make nos3-shell      # interactive shell in FSW container
```

For Earth observation testing, generate synthetic sensor data first:

```
make eosim-gen       # generate thermal raster files in tools/eosim/output/
```

The Docker Compose setup mounts the `tools/eosim/output/` directory read-only into the simulator container at `/sim/thermal_data`.

## Data Flow

For a single simulated satellite:

```
42 (orbit propagation)
  → NOS Engine shared memory
    → Hardware simulators (C++, each on a virtual bus)
      → NOS Engine TCP
        → hwlib (C, UART/SPI/I2C/CAN abstraction)
          → Rust cFS apps (via leodos-libcfs bindings)
            → Software Bus
              → LeoDOS communication stack
                → ISL router → other satellites / ground
```

The boundary between simulated and real hardware is at the hwlib layer. Flight app code is identical on both sides of this boundary.

## Time Synchronization

The simulation uses synchronized time at 10 μs per tick by default. NOS Engine distributes the simulation clock to all components — 42, hardware simulators, and cFS. The real-time rate mapping is configurable: the simulation can run faster or slower than wall clock time.

## Rust Bindings

Each NOS3 hardware component has C header files (`*_msg.h` for command/telemetry structs, `*_device.h` for the device driver API). These are bound into Rust via bindgen in the `leodos-libcfs` crate, providing safe wrappers that cFS apps use to interact with simulated (or real) hardware through the same API.
