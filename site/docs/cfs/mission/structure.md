# Structure

A cFS mission is organized into a fixed set of component types. Understanding these types explains how the flight software is partitioned, what depends on what, and how the build produces a bootable image.

## Apps

The basic unit of flight software is the app. Each app is an independent loadable module that registers with [Executive Services](/cfs/cfe/es) at startup, subscribes to messages on the [Software Bus](/cfs/cfe/sb), and runs until told to stop. Apps do not call each other directly — all communication goes through the bus. This means apps can be added, removed, or restarted without affecting the rest of the system.

A typical LEO mission includes:

- **Housekeeping** — periodically collects and publishes health/status from every other app
- **Command Ingest** — receives ground commands from the communication link and routes them to the bus
- **Telemetry Output** — collects telemetry messages from the bus and sends them to the communication link
- **Scheduler** — sends wakeup messages on a fixed timeline to drive periodic behavior
- **Stored Command** — executes time-tagged command sequences autonomously
- **Mission-specific apps** — payload control, attitude determination, orbit maneuvers, the LeoDOS communication stack

## Libraries

Libraries are shared code loaded by the executive but with no main loop or bus subscriptions. They provide common functionality — CRC routines, coordinate transforms, protocol encoders — that multiple apps link against. Loading a library once avoids duplicating code across apps and ensures they all use the same implementation.

## Tables

Tables are binary configuration files associated with an app, managed by [Table Services](/cfs/cfe/tbl). They are not compiled into the app — they are separate artifacts loaded at runtime. Each table has a defined schema (size and layout), a default image built with the mission, and can be replaced by the ground at any time. Tables are how the mission separates code from configuration: link timeouts, routing entries, scheduler slots, telemetry rates, and filter masks are all table data.

## Configuration

Runtime behavior is controlled by tables and configuration files, not compiled constants. Link timeouts, routing entries, scheduler slots, telemetry rates, filter masks — all of these are table data that the ground can inspect and modify while the app continues running. Beyond tables, several other configuration files shape the mission:

- **Startup script** — lists which apps to load, in what order, with what priority and stack size. Changing this file changes what runs on the next boot without rebuilding anything.
- **Message ID definitions** — the mapping between topic identifiers and apps. This defines the wiring of the bus — which messages exist and who can subscribe.
- **Platform configuration** — compile-time constants that set resource limits: maximum number of apps, pipe depths, pool sizes, [CDS](/cfs/cfe/es) capacity. These are fixed at build time and define the mission's resource envelope.

This separation between code and configuration means most operational adjustments — tuning rates, rerouting data, enabling or disabling features — never require a software update.

## The Build

A cFS mission is built by a CMake-based build system that combines the framework layers ([PSP](/cfs/psp), [OSAL](/cfs/osal), [cFE](/cfs/cfe/overview)), the mission's apps and libraries, and the target-specific configuration into a single deployable image. The build produces:

1. **The core executive** — PSP + OSAL + cFE, linked into a single binary that boots on the target processor.
2. **App modules** — each app is compiled as a separate loadable object. The executive loads them at startup based on a startup script that lists which apps to start, in what order, with what priority and stack size.
3. **Table images** — default table files for each app, placed on the file system where [Table Services](/cfs/cfe/tbl) can find them.

The startup script is itself a configuration file on the file system, not compiled in. Changing which apps load, or in what order, does not require rebuilding the core executive.

## LeoDOS Context

LeoDOS apps are written in Rust and compiled as C-compatible shared objects that the cFS executive loads like any other app. The Rust code links against `leodos-libcfs`, which provides safe wrappers around the cFE, OSAL, and PSP APIs. From the executive's perspective, a Rust app is indistinguishable from a C app — it exports the same entry point and uses the same bus and table interfaces.
