# Hardware

Satellites run on radiation-hardened processors that differ significantly from commodity hardware. Understanding these constraints explains many design decisions in the flight software.

## Processors

Flight processors range from 32-bit (LEON3, ARM Cortex-R) to 64-bit (LEON5, ARM Cortex-A on newer missions). They are orders of magnitude slower than ground servers — hundreds of MHz to low GHz, with limited RAM (megabytes to low gigabytes). Performance is constrained by radiation hardening, power budget, and thermal limits.

| Processor | Architecture | Clock | RAM | Radiation | Use case |
|---|---|---|---|---|---|
| **LEON3** | SPARC V8 (32-bit) | ~100 MHz | 128–512 MB | Rad-hard | Heritage flight processor, ESA/NASA missions |
| **LEON5** | SPARC V8 (64-bit) | ~250 MHz | 1–4 GB | Rad-hard | Next-gen European missions |
| **ARM Cortex-R** | ARMv7-R (32-bit) | ~400 MHz | 256 MB–1 GB | Rad-tolerant | Safety-critical real-time tasks |
| **ARM Cortex-A** | ARMv8-A (64-bit) | ~1.5 GHz | 2–8 GB | COTS + shielding | High-performance payloads, ML inference |
| **Nvidia Jetson** | ARM + GPU | ~1.5 GHz + 128 CUDA | 4–8 GB | COTS + shielding | ML inference on CubeSats |
| **FPGA** (Xilinx) | Configurable | — | — | Rad-hard variants | Compression, image processing, custom pipelines |

For comparison, a ground server runs at 3–5 GHz with 64–512 GB RAM and terabytes of storage. Flight processors trade performance for radiation tolerance, power efficiency, and reliability over multi-year missions.

## Memory

Flight processors typically have a flat memory model with no MMU (memory management unit). There is no virtual memory, no per-process address space isolation, and no demand paging. All applications share a single physical address space. This is why flight software uses pre-allocated [memory pools](/cfs/mission/memory) rather than malloc — fragmentation in a shared flat address space is fatal.

## Persistent Storage

Non-volatile storage (EEPROM, flash, or battery-backed RAM) stores data that must survive processor resets — the [Critical Data Store](/cfs/cfe/es), boot images, application binaries, and configuration tables. Flash has limited write cycles, which is why persistent writes are infrequent and sized carefully.

## Power

Satellites generate power from solar panels and store it in batteries. During eclipse (the satellite is in Earth's shadow), there is no solar input and the satellite runs on battery alone. Power-intensive operations — imaging, ISL transmission, onboard processing — must be scheduled around the power budget. The [EPS](/simulation/sensors) monitors battery state, solar array output, and power modes.
