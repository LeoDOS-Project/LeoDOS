# Overview

ColonyOS is a meta-OS for distributed computing developed at RISE. It coordinates heterogeneous workers — ground servers, edge devices, HPC clusters, and satellites — through a central server using a pull-based execution model.

## Core Concepts

- **Colony** — a group of executors managed by a single server. Each colony has an identity (ECDSA key pair) and a process queue.
- **Executor** — a worker that pulls jobs from the colony. An executor calls `assign()` when ready, receives a process specification, executes it, and reports the result. Executors can run anywhere — in a data center, on a laptop, or on a satellite.
- **Process** — a unit of work submitted to the colony. Describes what to run, with what inputs, and where to deliver the result.
- **Keepalive** — executors send periodic heartbeats to the server. If an executor stops responding, the server considers it dead and makes its process available for another executor.

## Pull-Based Model

ColonyOS uses a pull model rather than push. The server does not decide which executor runs a process — executors volunteer by calling `assign()`. This decouples the server from the executors: the server does not need to know executor capabilities, network addresses, or availability in advance. An executor that goes offline simply stops pulling, and its work is reassigned.

## Identity and Security

Every colony, executor, and user has an ECDSA key pair. All messages are signed and verified. This means:

- Only authorized executors can pull work from a colony
- Results are cryptographically attributed to the executor that produced them
- The server can verify that a process was completed by the executor it was assigned to

## Why ColonyOS for Space

A satellite constellation needs an orchestrator to submit jobs, track progress, and collect results. ColonyOS provides this without requiring the constellation to run any orchestration logic onboard. The ground submits a job, an executor plans and dispatches it to the constellation, and results flow back — all through ColonyOS's existing API.

The challenge is that ColonyOS assumes executors are always reachable, while satellites are predictably unreachable. The [integration](integration) page describes how this is solved.
