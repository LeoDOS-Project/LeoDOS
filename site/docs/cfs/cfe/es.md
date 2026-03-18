# Executive Services

Executive Services (ES) manages the lifecycle of every application in the cFS system. It handles startup, run-loop orchestration, shutdown, restart after faults, and provides shared resources like memory pools and the Critical Data Store.

## App Lifecycle

Every cFS application registers with ES at startup. ES tracks each app by its AppId, name, and version. Applications run in a loop — typically waiting on a [Software Bus](/cfs/cfe/sb) pipe — until ES signals them to shut down. If an application faults (exception, watchdog timeout), ES can restart it automatically based on the app's restart policy.

## Critical Data Store

The Critical Data Store (CDS) is a region of memory that survives processor resets. Applications register named CDS blocks at startup and store state that must persist across restarts — counters, sequence numbers, calibration data. The physical storage is provided by the [PSP](/cfs/psp) (battery-backed RAM, flash, or a file), but applications interact with it through the ES API without knowing the backing medium.

## Memory Pools

ES provides pre-allocated memory pools for dynamic allocation without heap fragmentation. Applications request a pool of a given size at startup, then allocate and free fixed-size blocks from it. This avoids the unpredictable latency of general-purpose malloc in a real-time environment.

## Performance Monitoring

ES includes entry/exit performance markers that applications place around critical code sections. These markers record timestamps, allowing ground tools to measure execution time, detect deadline overruns, and profile CPU usage across the system.

## Reset Types

ES distinguishes between power-on resets and processor resets. A power-on reset clears all state including the CDS. A processor reset preserves the CDS, allowing applications to recover. Processor resets have subtypes — commanded, watchdog, exception — that applications can query at startup to adapt their recovery behavior.
