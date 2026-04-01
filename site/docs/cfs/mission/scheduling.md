# Scheduling

cFS apps do not run on a free-wheeling loop. Execution is driven by a time-based schedule that determines which apps run, in what order, and at what rate. This addresses two concerns that matter for flight software: determinism and data ordering.

## Determinism

Each app runs in a known time slot within the schedule. A scheduler publishes wakeup messages at fixed intervals (typically 100–250 ms). Apps waiting on the [Software Bus](/cfs/cfe/sb) receive their wakeup, do one cycle of work, publish their outputs, and go back to sleep. Because every app runs in a predictable slot, worst-case execution time is bounded and analyzable — a property required for certification of safety-critical systems.

## Data Ordering

The schedule controls which apps run first within a time slot. This ensures causal ordering: sensor drivers run before the attitude controller that consumes their readings, and the attitude controller runs before the telemetry app that reports its output. Without a schedule, apps would race and the system would produce inconsistent snapshots of state.

## Rate Control

The schedule is organized into major frames (typically 1 second) divided into minor frames. The scheduler sends different wakeup messages in each minor frame, so apps can run at different rates. A sensor driver might wake every minor frame (4–10 Hz) while housekeeping wakes once per major frame (1 Hz). This avoids both unnecessary CPU usage for slow-changing data and delayed response for fast-changing data.

## Boot Sequence

Before the schedule starts, the system goes through a fixed startup sequence:

1. **PSP init** — the Platform Support Package initializes hardware and clears memory regions.
2. **Core services** — ES, EVS, SB, TIME, and TBL are initialized sequentially.
3. **Module loading** — ES parses the startup script, loading libraries and creating app tasks in order.
4. **Startup sync** — ES waits for all apps to report ready, or for a timeout to expire.
5. **Operational** — ES transitions to operational state and the scheduler begins sending wakeups.

Apps must call `CFE_ES_WaitForStartupSync` before doing real work. This call blocks until all apps are loaded and the system enters operational state. Without it, an app may try to use services — creating pipes, subscribing to messages, sending data — before the system is ready, leading to errors or crashes. The startup sync ensures every app sees a fully initialized system.

## Ground Adjustability

The schedule is configuration data, not compiled code. The ground can modify which apps wake in which slots, change the minor frame period, or reorder execution without uploading new software. This is how missions tune CPU utilization after launch, when the real workload becomes clear.
