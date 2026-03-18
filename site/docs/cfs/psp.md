# PSP

The Platform Support Package (PSP) is the lowest cFS layer — the interface between the flight software and the physical processor board. It abstracts board-level hardware so that the layers above never interact with specific registers, memory maps, or boot sequences directly.

## Purpose

Different missions fly different processors (LEON, ARM, PowerPC) on different board support packages. Without the PSP, every layer would need conditional logic for each target. The PSP isolates all board-specific code into a single replaceable module: swap the PSP and the same application binary runs on a different board.

## Key Abstractions

- **Processor restart and reset** — initiating power-on resets, processor resets, and querying the reset type and subtype after boot. This lets applications adapt their startup behavior depending on whether the reset was commanded, caused by a watchdog timeout, or triggered by an exception.
- **Memory access** — reading and writing to volatile (RAM) and non-volatile (EEPROM/flash) memory regions. The PSP provides a uniform interface regardless of the underlying memory technology.
- **Watchdog timer** — configuring and servicing the hardware watchdog. If the flight software stops servicing the watchdog (due to a hang or runaway task), the watchdog triggers a processor reset.
- **Cache management** — flushing and invalidating data and instruction caches when required for DMA transfers or self-modifying code paths.
- **Critical Data Store backend** — the PSP provides the physical storage that backs the cFE's [Critical Data Store](/cfs/cfe/es). On hardware with battery-backed RAM this is a direct memory region; on other platforms it may be backed by a file or flash partition.

## LeoDOS Context

During development and NOS3 simulation, LeoDOS uses a PSP that routes hardware access through the NOS Engine transport layer instead of real registers. This means the same flight software binary that runs against simulated hardware in Docker also runs on the target board — only the PSP changes.
