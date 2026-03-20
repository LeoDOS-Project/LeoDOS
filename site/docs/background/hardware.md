# Hardware

Satellites run on radiation-hardened processors that differ significantly from commodity hardware. Understanding these constraints explains many design decisions in the flight software.

## Processors

Flight processors range from 32-bit (LEON3, ARM Cortex-R) to 64-bit (LEON5, ARM Cortex-A on newer missions). They are orders of magnitude slower than ground servers — hundreds of MHz to low GHz, with limited RAM (megabytes to low gigabytes). Performance is constrained by radiation hardening, power budget, and thermal limits.

Some missions carry COTS (commercial off-the-shelf) ARM processors with radiation shielding, or FPGA fabric for specialized processing (compression, image classification). GPU accelerators (Nvidia Jetson) have flown on CubeSats for ML inference.

## Memory

Flight processors typically have a flat memory model with no MMU (memory management unit). There is no virtual memory, no per-process address space isolation, and no demand paging. All applications share a single physical address space. This is why flight software uses pre-allocated [memory pools](/cfs/mission/memory) rather than malloc — fragmentation in a shared flat address space is fatal.

## Persistent Storage

Non-volatile storage (EEPROM, flash, or battery-backed RAM) stores data that must survive processor resets — the [Critical Data Store](/cfs/cfe/es), boot images, application binaries, and configuration tables. Flash has limited write cycles, which is why persistent writes are infrequent and sized carefully.

## Power

Satellites generate power from solar panels and store it in batteries. During eclipse (the satellite is in Earth's shadow), there is no solar input and the satellite runs on battery alone. Power-intensive operations — imaging, ISL transmission, onboard processing — must be scheduled around the power budget. The [EPS](/simulation/sensors) monitors battery state, solar array output, and power modes.
