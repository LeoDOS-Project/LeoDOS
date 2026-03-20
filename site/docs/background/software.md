# Software

Satellites run real-time flight software that manages all onboard operations: attitude control, power management, communication, and payload data processing. The software must be deterministic (predictable execution time), fault-tolerant (recover from radiation-induced errors), and portable (run on different processors and operating systems).

## Flight Software Frameworks

Several frameworks exist for structuring flight software:

- **[cFS](/cfs/overview)** (NASA) — open-source, layered architecture (PSP → OSAL → cFE), app-based with a publish-subscribe Software Bus. Used on 50+ NASA missions. LeoDOS uses cFS.
- **F Prime** (NASA JPL) — open-source, component-based, targets smaller missions (CubeSats, instruments). Used on the Ingenuity Mars Helicopter.
- **OSRA/SAVOIR** (ESA) — reference architecture for European missions. Implementations are proprietary.
- **KOSMOS** (CNES) — modular framework with pre-qualified components. Available to CNES partners.

See [Core Flight System](/cfs/overview) for details on cFS and how LeoDOS uses it.

## Real-Time Operating Systems

Flight software runs on a real-time operating system (RTOS) that provides deterministic scheduling — tasks run at guaranteed times with bounded latency. Common choices:

- **VxWorks** — the most widely used RTOS in space. Commercial, proprietary.
- **RTEMS** — open-source RTOS used by several space missions. POSIX-compatible.
- **Linux** — used for development and some flight missions (typically with real-time patches). Not traditionally considered flight-qualified, but increasingly used on newer missions with COTS hardware.

cFS abstracts the OS through [OSAL](/cfs/osal), so the same application code runs on any of these without modification.

## LeoDOS

LeoDOS applications are written in Rust and compiled as C-compatible shared objects that the cFS executive loads like any other app. Rust provides memory safety without a garbage collector — critical on processors with no MMU where a wild pointer can corrupt any address. The Rust code links against `leodos-libcfs`, which provides safe wrappers around the cFE, OSAL, and PSP APIs.
