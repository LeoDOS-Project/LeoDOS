# Overview

ColonyOS is a meta-OS for distributed computing developed at RISE. It coordinates heterogeneous workers — ground servers, edge devices, HPC clusters, and satellites — through a central server using a pull-based execution model. Executors can reside anywhere on the Internet, even behind firewalls, connecting via HTTP Long Polling or WebSocket.

## Core Concepts

- **Colony** — a group of executors managed by a single server. Each colony has a cryptographic identity and a process queue.
- **Function Specification** — a description of work to be done, submitted to a colony. Specifies a function name, arguments, conditions (which executor type is eligible), and timing constraints (`maxwaittime`, `maxexectime`, `maxretries`). Submitting a function spec creates a process.
- **Process** — an instance created from a function specification. Tracks the lifecycle of that execution: waiting in the queue, assigned to an executor, running, completed, or failed. A process that is not completed within `maxexectime` is automatically moved back to the queue for another executor. After `maxretries` failures, the process is closed as failed.
- **Executor** — a worker that pulls processes from the colony. An executor calls `assign()` when ready, receives a process, executes the specified function, sets output attributes on the process, and closes it. Executors can be anything — a Kubernetes pod, a CLI tool, a browser, or a cFS app on a satellite.
- **Workflow** — a directed acyclic graph (DAG) of processes with dependencies between them. ColonyOS tracks which processes must complete before others can start.

## Pull-Based Model

ColonyOS uses a pull model rather than push. The server does not decide which executor runs a process — executors volunteer by calling `assign()` when they are ready for work. The server matches the process's conditions (executor type, constraints) against the requesting executor and assigns a suitable process. This decouples the server from executors: the server never opens connections to executors, and executors can be behind firewalls or NAT.

If an executor fails to complete a process in time (`maxexectime`), the process is automatically moved back to the queue and made available for another executor. This makes the system fault-tolerant — crashed or stalled executors do not block progress.

## Identity and Security

ColonyOS uses a crypto-identity protocol inspired by Bitcoin and Ethereum. Each colony and executor has a private key (ECDSA on secp256k1). Messages are signed with the private key, and the server reconstructs the sender's identity from the signature — the identity is a SHA3-256 hash of the reconstructed public key. The server never stores private keys.

This means:
- Only registered executors can pull work from their colony
- The server verifies identity on every request without exchanging keys
- Results are cryptographically attributed to the executor that produced them

## Why ColonyOS for Space

A satellite constellation needs an orchestrator to submit jobs, track progress, and collect results. ColonyOS provides this without requiring the constellation to run any orchestration logic onboard. The ground submits a function specification, an executor plans and dispatches it to the constellation, and results flow back — all through ColonyOS's existing API.

The challenge is that ColonyOS's fault model assumes unreachable executors have failed, while satellites are predictably unreachable on a regular schedule. The [integration](integration) page describes how this is solved.
