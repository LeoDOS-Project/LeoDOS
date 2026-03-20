# Software

Satellites run real-time flight software that manages all onboard operations: attitude control, power management, communication, and payload data processing. The software must be deterministic (predictable execution time), fault-tolerant (recover from radiation-induced errors), and portable (run on different processors and operating systems).

## Flight Software Frameworks

Several frameworks exist for structuring flight software:

| Framework | Agency | Open source | Architecture | Target scale | Notable missions |
|---|---|---|---|---|---|
| **[cFS](/cfs/overview)** | NASA Goddard | Yes (Apache 2.0) | Layered (PSP → OSAL → cFE), app-based, pub-sub Software Bus | Medium to large | GPM, Lunar Gateway, Roman Space Telescope |
| **F Prime** | NASA JPL | Yes (Apache 2.0) | Component-based, port connections, code generation | Small (CubeSats, instruments) | Ingenuity Mars Helicopter, ASTERIA |
| **OSRA/SAVOIR** | ESA | Spec public, impl proprietary | Reference architecture, Eclipse tooling | Medium to large | European institutional missions |
| **KOSMOS** | CNES | No (institutional) | Modular, pre-qualified components (ECSS Level B) | Medium to large | CNES missions since 2019 |

LeoDOS uses cFS — the only open-source framework with heritage on large missions and a layered architecture that separates hardware, OS, and application concerns. See [Core Flight System](/cfs/overview) for details.

## Real-Time Operating Systems

Flight software runs on a real-time operating system (RTOS) that provides deterministic scheduling — tasks run at guaranteed times with bounded latency.

| RTOS | License | POSIX | Determinism | Use case |
|---|---|---|---|---|
| **VxWorks** | Commercial | Partial | Hard real-time | Most widely used RTOS in space |
| **RTEMS** | Open source (BSD) | Yes | Hard real-time | ESA and NASA missions, growing adoption |
| **Linux** | Open source (GPL) | Yes | Soft (hard with RT patches) | Development, COTS hardware, payload processing |

cFS abstracts the OS through [OSAL](/cfs/osal), so the same application code runs on any of these without modification. LeoDOS develops on Linux and targets RTEMS or VxWorks for flight.

## LeoDOS

LeoDOS applications are written in Rust and compiled as C-compatible shared objects that the cFS executive loads like any other app. Rust provides memory safety without a garbage collector — critical on processors with no MMU where a wild pointer can corrupt any address. The Rust code links against `leodos-libcfs`, which provides safe wrappers around the cFE, OSAL, and PSP APIs.
