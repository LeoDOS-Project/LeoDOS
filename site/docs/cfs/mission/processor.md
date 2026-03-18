# Processor

cFS targets radiation-hardened processors that differ significantly from commodity hardware. Understanding these constraints explains many design decisions in the framework — why memory is pre-allocated, why there is no MMU assumption, and why portability across word sizes and byte orders matters.

## Word Size

Flight processors range from 32-bit (LEON3, ARM Cortex-R) to 64-bit (LEON5, ARM Cortex-A on newer missions). cFS is portable across both. The framework and OSAL APIs use sized types (`uint32`, `int16`) rather than platform-dependent types like `int`, so structure layouts and message formats are consistent regardless of the target's native word size.

## Byte Order

Most flight processors are big-endian (SPARC/LEON family), but ARM targets can be either. CCSDS protocol headers are big-endian by specification. cFS does not assume a particular byte order internally — the PSP exposes the target's endianness, and protocol code performs explicit conversions at serialization boundaries rather than relying on native layout.

## Memory Architecture

Flight processors typically have a flat memory model with no MMU (memory management unit). There is no virtual memory, no per-process address space isolation, and no demand paging. All apps share a single physical address space. This is why cFS uses [memory pools](/cfs/mission/memory) rather than malloc — fragmentation in a shared flat address space is fatal, and there is no OS-level protection to contain it.

Some processors provide a limited MPU (memory protection unit) that can mark regions as read-only or no-execute, but full page-level isolation is rare. The practical consequence: a wild pointer in one app can corrupt another app's data. This is mitigated by coding discipline, [watchdog](/cfs/mission/fault-tolerance) resets, and [CDS](/cfs/cfe/es) recovery rather than hardware memory protection.

## Radiation

LEO processors are exposed to ionizing radiation that causes single-event upsets (bit flips in RAM and registers) and single-event latchups (short circuits that require a power cycle). Radiation-hardened processors reduce but do not eliminate these effects. The remaining risk is addressed by ECC memory (corrects single-bit, detects double-bit errors), software checksums on persistent and configuration data, and the [watchdog/reset escalation chain](/cfs/mission/fault-tolerance).

## Clock and Timing

The processor's local oscillator provides the time base for MET and the scheduler. Oscillator accuracy varies — crystal oscillators drift a few parts per million, meaning milliseconds per day. This drift is corrected by the [time synchronization](/cfs/mission/time) mechanism (1PPS tone from GPS or ground uplink). The timer resolution (how finely the OS can schedule wakeups) is typically microseconds, which is more than sufficient for the 100–250 ms minor frame periods used in [scheduling](/cfs/mission/scheduling).

## Persistent Storage

Flight processors typically have access to non-volatile storage: EEPROM, flash, or battery-backed RAM. The [PSP](/cfs/psp) abstracts the specific technology. cFS uses this for the [Critical Data Store](/cfs/cfe/es) (data that survives processor resets), boot images (the core executive binary), and the file system (app binaries, table images, startup scripts). Write endurance varies — flash has limited write cycles, which is why CDS writes are infrequent and sized carefully.

## LeoDOS Context

LeoDOS targets the LEON3 (32-bit SPARC, big-endian) for flight and x86-64 Linux for development and NOS3 simulation. The PSP and OSAL layers absorb the differences — application code, including the Rust communication stack, is identical across both targets. The NOS3 simulation PSP routes hardware access through the NOS Engine transport instead of real registers, so the same binary that runs in Docker also runs on the flight processor.
