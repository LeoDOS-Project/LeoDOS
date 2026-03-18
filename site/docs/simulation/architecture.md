# Architecture

The simulation stack has three layers: orbital mechanics (42), hardware simulators (NOS3 components), and flight software (cFS + LeoDOS apps). They communicate through the NOS Engine transport layer, which replaces real hardware buses with TCP connections.

## 42 — Orbital Mechanics

42 is NASA's spacecraft simulation environment. It propagates orbits, computes attitude, and models the space environment (sun position, magnetic field, atmospheric drag). Each satellite in the simulation has a 42 instance that provides its position, velocity, and attitude to the hardware simulators below.

## NOS3 Components

NOS3 provides C/C++ hardware simulators that behave like real devices. Each simulator registers on a virtual bus (UART, SPI, I2C, CAN) through NOS Engine and responds to the same register-level protocol as the real hardware. The LeoDOS constellation uses:

- **generic_radio** — RF transceiver for ground and inter-satellite links
- **generic_eps** — electrical power system (battery, solar arrays, power modes)
- **generic_adcs** — attitude determination and control
- **generic_css / generic_fss** — coarse and fine sun sensors
- **generic_imu** — gyroscope and accelerometer
- **generic_mag** — magnetometer
- **generic_star_tracker** — attitude determination via star catalog matching
- **generic_reaction_wheel** — momentum/torque control
- **generic_torquer** — magnetic desaturation
- **generic_thruster** — orbit and attitude maneuvers
- **novatel_oem615** — GPS receiver (provides position to workflow apps)
- **arducam** — visible-light camera
- **thermal_cam** — thermal IR camera (custom, for wildfire detection workflows)

Each component has C header files (`*_msg.h` for command/telemetry structs, `*_device.h` for the device driver API) bound into Rust via `leodos-libcfs` with bindgen.

## NOS Engine Transport

NOS Engine replaces physical buses with TCP connections. The hwlib layer in cFS provides the same API (uart_read, spi_write, i2c_transfer) regardless of whether the underlying transport is a real hardware register or a NOS Engine TCP socket. Flight apps call hwlib; hwlib routes to real hardware or NOS Engine depending on the build configuration.

## Docker Setup

Each simulated satellite runs in a Docker container based on the `ivvitc/nos3-64` image (NOS Engine SDK pre-installed). The LeoDOS build adds the Rust toolchain and compiles the flight software. Sensor data directories (e.g., synthetic thermal images from `eosim`) are mounted as read-only volumes.

```
make nos3-build      # build Docker image (adds Rust toolchain)
make nos3-config     # generate NOS3 build configuration
make nos3-build-sim  # build hardware simulators (C++)
make nos3-build-fsw  # build flight software (cFS + Rust apps)
make nos3-launch     # start all containers
make nos3-stop       # stop all containers
make nos3-shell      # interactive shell in FSW container
```

## How It Connects

The data flow for a single simulated satellite:

```
42 (orbit propagation)
  → NOS Engine TCP
    → Hardware simulators (C++, each on a virtual bus)
      → NOS Engine TCP
        → hwlib (C, SPI/UART/I2C abstraction)
          → Rust cFS apps (via leodos-libcfs bindings)
            → Software Bus
              → LeoDOS communication stack
                → ISL router → other satellites / ground
```

The key property: the Rust app code is identical in simulation and flight. The boundary between simulated and real hardware is at the hwlib/NOS Engine layer, which is invisible to application code.
