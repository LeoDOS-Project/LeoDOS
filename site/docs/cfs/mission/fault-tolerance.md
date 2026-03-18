# Fault Tolerance

Any app may be stopped at any time — by a ground command, after a detected fault, or by a processor reset. cFS is designed around this assumption. The question is not whether faults happen, but how each class of fault is detected and how the system recovers.

## Data Integrity

Radiation in LEO causes single-event upsets — individual bits flipping in RAM or registers. This is addressed at multiple levels. Hardware ECC memory corrects single-bit errors and detects double-bit errors transparently. Persistent storage and configuration data are checksummed, so corruption is detected on read and the app falls back to defaults rather than using corrupt state. Messages between apps stay in shared memory within a single processor and are protected by the same ECC. For data that crosses a physical link, integrity is handled by the communication stack — CRC in transfer frames, forward error correction in the [coding layer](/protocols/coding/overview).

## App Crashes

When an app faults — null pointer dereference, stack overflow, illegal instruction — the processor raises an exception. The exception handler logs the context (registers, stack pointer, program counter) and notifies the executive. If the app is registered as restartable, it is reloaded and restarted. It recovers its persistent state and resumes. If it is not restartable, it is marked as failed and the system continues without it.

Because apps communicate only through the [Software Bus](/cfs/cfe/sb), restarting one app does not require notifying or restarting others. The bus simply stops delivering messages from that app until it re-registers. Other apps see a gap in telemetry or a missed response, but they do not crash or deadlock — they were never coupled to the faulting app.

## System Hangs

If the entire system hangs — a deadlocked mutex, an infinite loop in a high-priority task, a kernel panic — no software recovery is possible. The hardware watchdog timer is the last line of defense. It is serviced every scheduling cycle; if the service stops, the watchdog forces a processor reset. The reset type is recorded so ground tools can distinguish watchdog resets from commanded ones.

## Escalation

Repeated faults trigger escalation. If the same app faults multiple times within a configurable window, the system escalates from an app restart to a processor reset. A processor reset preserves persistent storage, so all apps can recover their state. If processor resets also recur, the system performs a power-on reset that clears persistent storage and forces a clean start. This ladder prevents a single faulty app from consuming all recovery attempts indefinitely.

## Observability

Every detected error — hardware exceptions, checksum mismatches, invalid commands, out-of-range readings — is emitted as a structured event via [Event Services](/cfs/cfe/evs). These events flow through the [Software Bus](/cfs/cfe/sb), get packed into telemetry, and are downlinked to the ground. The ground has visibility into every error the system detected, including those handled autonomously. Rate filtering prevents high-frequency errors from overwhelming the downlink.

## LeoDOS Context

The LeoDOS communication stack adds its own fault handling. Bit errors on the RF link are corrected by the coding layer (randomization, Reed-Solomon, LDPC). Frame-level losses are recovered by [COP-1](/protocols/datalink/reliability/cop1). End-to-end packet loss across multiple hops is handled by [SRSPP](/protocols/transport/srspp) retransmission. If the communication app itself crashes, the executive restarts it and SRSPP resumes retransmission from its persisted sequence numbers.
