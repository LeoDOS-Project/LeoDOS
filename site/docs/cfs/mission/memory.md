# Memory

Flight software runs for months or years without restart. A general-purpose heap — malloc and free — leads to fragmentation over time, and eventually an allocation fails with no operator to intervene. Every memory concern in a cFS mission is addressed by making all allocations bounded and known at build time.

## Fragmentation

The central problem is that repeated allocate/free cycles of varying sizes create gaps in memory that are individually too small to satisfy new requests, even though total free memory is sufficient. cFS avoids this entirely: all dynamic-looking allocations (message buffers, task-local storage, configuration data) come from fixed-size pools where every block in a pool is the same size. Freed blocks return to their pool for reuse. There is no external fragmentation because there is no variation in block size within a pool.

## Isolation

A bug in one app must not starve another of memory. Each app's allocations are drawn from its own pool, so one app's usage pattern cannot exhaust another app's resources. This provides memory isolation between apps without requiring a hardware memory management unit, which many flight processors lack.

## Bounded Usage

Every byte of memory used by a mission is accounted for at build time — app pools, message buffer regions, configuration data regions, task stacks, persistent storage blocks. There are no runtime allocations whose size depends on input data or operational state. This means the system's memory footprint on day one is the same as on day one thousand.

Task stacks are a specific case: each task declares a fixed stack size at creation. There is no dynamic growth. Stack sizes are determined by analysis and profiling during development. An undersized stack causes a deterministic exception rather than silent corruption.

Static data (`static` variables in Rust, BSS/data segments in C) lives in the app's `.so` and is mapped by `dlopen` — separate from the task stack. There is no cFS-imposed limit on static data size. This distinction matters when working with large buffers: a function that declares a megabyte-sized local array will overflow a typical cFS task stack (64–256 KB). Moving the buffer to a `static` keeps it in BSS, and the function only places a reference on the stack.

## Persistence

Some data must survive processor resets — sequence counters, calibration values, operational modes. cFS provides a fixed-size persistent storage region backed by the [PSP](/cfs/psp) (battery-backed RAM, flash, or a file depending on the board). Apps register fixed-size blocks at startup and read them back after a reset. Each block is checksummed so that corruption from a power glitch or radiation upset is detected on read, and the app falls back to defaults rather than using corrupt state.
