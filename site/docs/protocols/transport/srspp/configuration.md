# Configuration Parameters

SRSPP does not negotiate parameters at runtime. Both endpoints must be configured with compatible values.

- **APID** — application process ID used for routing. Must match on sender and receiver.
- **WIN** (default 8) — window size: maximum number of unacknowledged packets in flight. Receiver's WIN should be >= sender's WIN.
- **BUF** (default 4096) — send buffer size in bytes. Limits total queued data.
- **MTU** (default 512) — maximum transmission unit: largest payload per packet. Messages larger than MTU are segmented.
- **REASM** (default 8192) — maximum reassembled message size on the receiver. Must be >= the sender's largest message.
- **RTO Policy** (default Fixed) — retransmission timeout strategy (see below).
- **Max Retransmits** (default 3) — attempts before declaring a packet lost and signaling an error to the application.
- **ACK Delay** (default 100 ticks) — time the receiver waits before sending a delayed ACK, allowing multiple packets to be acknowledged in one ACK.

## RTO Policy

The retransmission timeout is governed by a pluggable RTO policy.
The sender queries the policy each time it starts a retransmission timer,
passing the current time so the policy can adapt dynamically.

Two built-in policies are provided:

**FixedRto** — returns a constant timeout. Suitable for ISL links
with stable, predictable latency.

**OrbitAwareRto** — adapts the timeout based on a contact schedule:

- If the current time falls inside a LOS window: use a short ISL RTO
  (the link is active, real loss should be detected quickly).
- If outside a window: set RTO to the time until the next LOS window
  plus a configurable margin. This prevents the sender from declaring
  packets lost during normal orbital gaps.
- If no future windows exist in the schedule: fall back to the ISL RTO.

The contact schedule is stored in a fixed-size buffer, suitable for embedded systems. Each window records a station ID and start/end time in seconds.

Custom policies can be implemented by providing a different RTO computation.

## Compatibility Constraints

SRSPP does not negotiate parameters at runtime. Both endpoints
must be configured with compatible values. The simplest approach is to
share the same constants between sender and receiver applications. With
matching defaults, the sender will never exceed what the receiver can
handle.

- APID must match for routing to work
- Receiver's WIN should be >= sender's WIN (to buffer all in-flight packets)
- Receiver's REASM must be >= sender's largest message size
