# Overview

LeoDOS uses NASA's NOS3 framework to simulate a satellite constellation on a development machine. The same cFS flight software runs in simulation and in flight — application code is identical in both environments, with only the hardware abstraction layer swapped.

## Architecture

The simulation has three layers connected by NOS Engine, a middleware that replaces physical hardware buses (UART, SPI, I2C, CAN) with TCP connections:

```
42 (orbit propagation, attitude, environment)
  → NOS Engine shared memory
    → Hardware simulators (C++, one per device)
      → NOS Engine TCP (virtual UART/SPI/I2C/CAN)
        → hwlib (C, bus abstraction)
          → Rust cFS apps (via leodos-libcfs)
            → Software Bus → communication stack → ISL mesh
```

The hwlib layer is the boundary between simulated and real hardware. Flight apps call the same hwlib API in both environments — `spi_read`, `uart_write`, `i2c_transfer` — and do not know whether the underlying transport is a NOS Engine TCP socket or a real register.

Simulation time is synchronized at 10 μs per tick. NOS Engine distributes the clock to all components. The simulation can run faster or slower than wall clock time.

## Sections

- [Orbital Mechanics](orbital-mechanics) — orbit propagation, attitude dynamics, and the space environment
- [Sensors and Actuators](sensors) — attitude, navigation, power, propulsion, and imaging hardware
- [Communication](communication) — ground links, inter-satellite links, and RF modeling gaps
- [Earth Observation](earth-observation) — synthetic sensor data for workflow testing
