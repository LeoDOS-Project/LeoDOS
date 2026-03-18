# Overview

A cFS mission is a collection of applications running on a shared bus, scheduled by the real-time OS, and managed by Executive Services. Every mission follows the same structural pattern regardless of what the spacecraft actually does — the mission-specific behavior lives entirely in the apps and their configuration.

- [Structure](structure) — what a mission is made of: apps, libraries, tables, and the build process
- [Deployment](deployment) — updating apps and configuration on a running spacecraft
- [Scheduling](scheduling) — time-driven execution model and deterministic CPU allocation
- [Communication](communication) — how apps exchange data through the bus
- [Identification](identification) — spacecraft IDs, APIDs, and message routing
- [Time](time) — how the spacecraft knows what time it is and stays synchronized
- [Processor](processor) — hardware assumptions: word size, endianness, radiation, no MMU
- [Fault Tolerance](fault-tolerance) — error detection and recovery, from bit flips to app crashes
- [Memory](memory) — bounded, pre-allocated memory without a general-purpose heap
