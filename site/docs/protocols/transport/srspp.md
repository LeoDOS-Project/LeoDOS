# SRSPP

SRSPP is a lightweight reliable transport protocol built on top of CCSDS Space Packets.
It provides reliable, ordered delivery of messages over unreliable links with minimal
overhead.

## Design Goals

- **Minimal overhead**: Reuses existing SPP header fields (sequence count, sequence flags)
- **No congestion control**: Designed for point-to-point links with predictable latency
- **Simple implementation**: Suitable for resource-constrained flight software
- **Efficient ACKs**: Combined cumulative and selective acknowledgment
- **No handshake**: Data is sent immediately without connection establishment

### Why No Handshake?

SRSPP has no connection setup or liveness check. The sender can transmit data
immediately without waiting for a round-trip confirmation that the receiver exists.

This design choice is motivated by space link characteristics:
- **High latency**: Handshakes add significant delay on links with multi-second round-trips. In the worst case, a satellite may pass out of range before a handshake completes.
- **Known endpoints**: Mission configurations define who communicates with whom
- **Scheduled contacts**: Ground stations know when satellites are reachable

The tradeoff is that sending to an unreachable receiver wastes time until packets
hit max retransmits. The application receives packet loss notifications and can
decide when to abort. This optimizes for the common case (receiver is reachable)
rather than the failure case.

## SRSPP Header

All SRSPP packets begin with a 3-byte header following the SPP primary header:

| Field          | Size   | Description                                   |
|----------------|--------|-----------------------------------------------|
| Source Address | 2 bytes| Address of the sender (RawAddress format)     |
| Type           | 2 bits | Packet type: 0=DATA, 1=ACK, 2=EOS, 3=Reserved |
| Version        | 2 bits | Protocol version (current: 0)                 |
| Spare          | 4 bits | Reserved for future use, must be 0            |

The source address identifies the sending spacecraft. Combined with the
destination APID from the SPP header, it forms the stream key:
`(source_address, destination_apid)`.

Only one sender per spacecraft is supported for each destination APID. In CFS,
this is typically achieved by having a single gateway app handle the SRSPP
stream while other apps communicate with it via the Software Bus.

## Packet Types

SRSPP defines three packet types.

### DATA Packet (Type=0)

| Section | Size |
|---------|------|
| Space Packet Primary Header | 6 bytes |
| SRSPP Header | 3 bytes |
| Payload | N bytes |

### ACK Packet (Type=1)

| Section | Size |
|---------|------|
| Space Packet Primary Header | 6 bytes |
| SRSPP Header | 3 bytes |
| ACK Payload | 4 bytes |

The ACK Payload contains:
- Cumulative ACK (2 bytes)
- Selective ACK Bitmap (2 bytes)

### EOS Packet (Type=2)

| Section | Size |
|---------|------|
| Space Packet Primary Header | 6 bytes |
| SRSPP Header | 3 bytes |

The EOS packet signals that the sender has no more data to transmit. It has its
own sequence number and is acknowledged like a DATA packet. The transfer is
complete when the sender receives an ACK covering the EOS sequence number.

## Reliability Mechanism

### Sequence Numbers

SRSPP uses the 14-bit sequence count field from the SPP primary header for reliability.
Sequence numbers wrap around at 16383 (0x3FFF).

### Acknowledgments

The receiver acknowledges received packets using a combined cumulative and selective
ACK scheme:

**Cumulative ACK**: The highest sequence number received in-order. All packets up to
and including this number are acknowledged.

**Selective ACK Bitmap**: A 16-bit bitmap indicating which out-of-order packets have
been received. Bit N indicates receipt of packet (cumulative_ack + 1 + N).

Example:
```
Received: 0, 1, 2, 5, 7
Cumulative ACK: 2
Selective Bitmap: 0b00010100
                      │  │
                      │  └─ bit 2 = seq 5 received
                      └──── bit 4 = seq 7 received
```

### Retransmission

The sender maintains a retransmission timer for each unacknowledged packet. When a
timer expires, the packet is retransmitted. After a configurable number of failed
retransmissions, the packet is considered lost.

### ACK Timing

The receiver can be configured for:
- **Immediate ACK**: Send ACK immediately upon receiving any data packet
- **Delayed ACK**: Wait for a timeout before sending ACK (allows batching)

## Segmentation

Large messages that exceed the MTU are segmented using the SPP sequence flags:

| Flag           | Value | Meaning                          |
|----------------|-------|----------------------------------|
| Unsegmented    | 0b11  | Complete message in one packet   |
| First          | 0b01  | First segment of a message       |
| Continuation   | 0b00  | Middle segment                   |
| Last           | 0b10  | Final segment of a message       |

The receiver reassembles segments in order. All segments of a message share
consecutive sequence numbers.

## Flow Control

SRSPP uses a sliding window for flow control:

- **Window size (WIN)**: Maximum number of unacknowledged packets in flight
- **Send buffer (BUF)**: Total bytes that can be queued for transmission

The sender blocks when either the window is full or the buffer is exhausted.

## Configuration Parameters

| Parameter       | Used by  | Default | Description                                    |
|-----------------|----------|---------|------------------------------------------------|
| APID            | Both     | —       | Application Process ID for routing             |
| WIN             | Both     | 8       | Window size (max in-flight packets)            |
| BUF             | Sender   | 4096    | Send buffer size (bytes)                       |
| MTU             | Sender   | 512     | Maximum transmission unit (bytes per packet)   |
| REASM           | Receiver | 8192    | Maximum reassembled message size (bytes)       |
| RTO Policy      | Sender   | Fixed   | Retransmission timeout strategy (see below)    |
| Max Retransmits | Sender   | 3       | Attempts before declaring packet lost          |
| ACK Delay       | Receiver | 100     | Time to wait before sending delayed ACK (ticks)|

### RTO Policy

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

**Note:** SRSPP does not negotiate parameters at runtime. Both endpoints
must be configured with compatible values. The simplest approach is to
share the same constants between sender and receiver applications. With
matching defaults, the sender will never exceed what the receiver can
handle.

Compatibility constraints:
- APID must match for routing to work
- Receiver's WIN should be ≥ sender's WIN (to buffer all in-flight packets)
- Receiver's REASM must be ≥ sender's largest message size

## Sender Operation

The sender maintains a window of unacknowledged packets. Each packet in the window
is in one of two states:

- **Pending Transmit**: Queued but not yet sent, or marked for retransmission
- **Awaiting ACK**: Transmitted and waiting for acknowledgment

### Send Flow

1. Application submits a message to send
2. If message exceeds MTU, segment it into multiple packets (FIRST/CONTINUATION/LAST)
3. For each packet:
   - Assign the next sequence number
   - Store packet in send buffer
   - Mark as Pending Transmit
4. Transmit all Pending Transmit packets
5. Start retransmission timer for each transmitted packet
6. Mark transmitted packets as Awaiting ACK

### ACK Processing

When an ACK arrives:

1. For each packet covered by the cumulative ACK (seq ≤ cumulative_ack):
   - Stop its retransmission timer
   - Remove from send buffer
2. For each bit set in the selective bitmap:
   - Calculate seq = cumulative_ack + 1 + bit_position
   - Stop that packet's retransmission timer
   - Remove from send buffer
3. Slide the window forward

### Timeout Handling

When a retransmission timer expires:

1. If retransmit count < max_retransmits:
   - Increment retransmit count
   - Mark packet as Pending Transmit
   - Retransmit the packet
   - Restart timer
2. Otherwise:
   - Declare packet lost
   - Remove from send buffer
   - Signal error to application

### Stream Termination

To signal end of transmission:

1. Application requests stream close
2. Send EOS packet with next sequence number
3. Wait for ACK covering the EOS sequence
4. Transfer is complete when EOS is acknowledged

The EOS is retransmitted like DATA if no ACK arrives.

## Receiver Operation

The receiver maintains the expected sequence number and a reorder buffer for
out-of-order packets.

### Receive Flow

When a DATA or EOS packet arrives:

1. Compare packet sequence to expected sequence
2. If seq == expected (in-order):
   - If DATA: deliver payload to reassembly
   - If EOS: signal stream complete to application
   - Advance expected sequence
   - Check reorder buffer for now-deliverable packets
   - Repeat until no more consecutive packets
3. If seq > expected but within window (out-of-order):
   - Store in reorder buffer
   - Set corresponding bit in selective bitmap
4. If seq < expected (duplicate):
   - Ignore
5. Schedule or send ACK (see ACK Generation)

### Reassembly

As packets are delivered in order:

1. Check sequence flag:
   - UNSEGMENTED: Complete message, deliver to application
   - FIRST: Start new reassembly buffer
   - CONTINUATION: Append to reassembly buffer
   - LAST: Append and deliver complete message to application

### ACK Generation

After processing each DATA packet:

1. If immediate_ack mode:
   - Send ACK immediately
2. If delayed_ack mode:
   - If no ACK timer running, start one
   - When timer expires, send ACK

The ACK contains:
- Cumulative ACK = expected_seq - 1 (highest in-order seq received)
- Selective bitmap = bits for each buffered out-of-order packet

## Version Handling

The SRSPP header includes a 2-bit version field. When a packet arrives with an
unrecognized version:

1. Discard the packet silently
2. Do not send an ACK (sender will retransmit or timeout)
3. Optionally log the event for diagnostics

This allows future protocol versions to coexist during transitions. Endpoints
should be upgraded to matching versions for reliable communication.

## Error Handling

### Sender Errors

| Condition | Behavior |
|-----------|----------|
| Send buffer full | Block or reject new messages until space available |
| Window full | Block until ACKs free window slots |
| Packet lost (max retransmits) | Signal error to application, remove packet |

### Receiver Errors

| Condition | Behavior |
|-----------|----------|
| Reorder buffer full | Drop out-of-order packet (will be retransmitted) |
| Message too large for REASM | Discard segments, signal error to application |
| CONTINUATION without FIRST | Discard segment, signal reassembly error |
| Unknown packet type | Discard silently |
| Unknown version | Discard silently |

Receiver errors do not generate negative acknowledgments. The sender will
retransmit based on timeouts if needed.

## Protocol Sequences

### Normal Operation (Immediate ACK)

```
    Sender                                      Receiver
      │                                            │
      │────── DATA [seq=0] ───────────────────────>│
      │────── DATA [seq=1] ───────────────────────>│
      │────── DATA [seq=2] ───────────────────────>│
      │<───────────────────── ACK [cum=0, bmp=0] ──│
      │<───────────────────── ACK [cum=1, bmp=0] ──│
      │<───────────────────── ACK [cum=2, bmp=0] ──│
      │                                            │
```

With immediate ACK mode, the receiver sends an ACK after every DATA packet. The
sender does not wait for ACKs before sending more packets - it can have up to WIN
packets in flight simultaneously. ACKs arrive asynchronously and release packets
from the send window.

### Normal Operation (Delayed ACK)

```
    Sender                                      Receiver
      │                                            │
      │────── DATA [seq=0] ───────────────────────>│
      │                                            │  (start ACK timer)
      │────── DATA [seq=1] ───────────────────────>│
      │                                            │
      │────── DATA [seq=2] ───────────────────────>│
      │                                            │  (ACK timer expires)
      │<───────────────────── ACK [cum=2, bmp=0] ──│
      │                                            │
```

With delayed ACK mode, the receiver waits for the ACK timer to expire before sending.
This allows multiple packets to be acknowledged with a single ACK, reducing overhead.
The tradeoff is slightly higher latency for retransmission detection.

### Packet Loss and Retransmission (Immediate ACK)

```
    Sender                                      Receiver
      │                                            │
      │────── DATA [seq=0] ───────────────────────>│
      │                                            │
      │<───────────────────── ACK [cum=0, bmp=0] ──│
      │                                            │
      │────── DATA [seq=1] ────────X               │  (lost)
      │                                            │
      │────── DATA [seq=2] ───────────────────────>│
      │                                            │
      │<───────────────────── ACK [cum=0, bmp=01] ─│  seq=2 in bitmap
      │                                            │
      │  (timeout for seq=1)                       │
      │                                            │
      │────── DATA [seq=1] ───────────────────────>│  (retransmit)
      │                                            │
      │<───────────────────── ACK [cum=2, bmp=0] ──│
      │                                            │
```

With immediate ACK, every DATA packet triggers an ACK. When seq=0 arrives, the
receiver ACKs it immediately. When seq=1 is lost and seq=2 arrives out of order,
the receiver cannot advance the cumulative ACK past 0, but sets bit 0 in the
selective bitmap to indicate seq=2 was received. When the sender's retransmission
timer expires for seq=1, it retransmits. Once the receiver gets seq=1, it can
deliver both packets in order and advance the cumulative ACK to 2.

### Segmented Message (Delayed ACK)

```
    Sender                                      Receiver
      │                                            │
      │  Message: "HELLO WORLD" (segmented)        │
      │                                            │
      │────── DATA [seq=0, FIRST, "HELLO"] ───────>│
      │                                            │  (start ACK timer)
      │────── DATA [seq=1, LAST, " WORLD"] ───────>│
      │                                            │  (ACK timer expires)
      │<───────────────────── ACK [cum=1, bmp=0] ──│
      │                                            │
      │                             Message ready: │
      │                             "HELLO WORLD"  │
      │                                            │
```

Messages larger than the MTU are split into segments. The first segment is marked
with the FIRST flag, middle segments with CONTINUATION, and the final segment with
LAST. The receiver buffers segments until the complete message is reassembled. Each
segment has its own sequence number for reliability, but the application only sees
the complete reassembled message.

### Out-of-Order Delivery with Selective ACK (Immediate ACK)

```
    Sender                                      Receiver
      │                                            │
      │────── DATA [seq=0] ───────────────────────>│
      │                                            │
      │<───────────────────── ACK [cum=0, bmp=0] ──│
      │                                            │
      │────── DATA [seq=1] ────────X               │  (lost)
      │                                            │
      │────── DATA [seq=2] ───────────────────────>│  (buffered)
      │                                            │
      │<───────────────────── ACK [cum=0, bmp=10] ─│  bit 1 = seq 2
      │                                            │
      │────── DATA [seq=3] ───────────────────────>│  (buffered)
      │                                            │
      │<───────────────────── ACK [cum=0, bmp=110]─│  bits 1,2 = seq 2,3
      │                                            │
      │  (timeout for seq=1)                       │
      │                                            │
      │────── DATA [seq=1] ───────────────────────>│  (retransmit)
      │                                            │
      │<───────────────────── ACK [cum=3, bmp=0] ──│  all delivered
      │                                            │
```

This example shows the selective ACK mechanism in detail. After seq=1 is lost, the
receiver buffers seq=2 and seq=3 as they arrive out of order. The selective bitmap
accumulates: bit 1 indicates seq=2 (cumulative + 2), bit 2 indicates seq=3
(cumulative + 3). The sender knows these packets don't need retransmission. When
seq=1 finally arrives, the receiver delivers all buffered packets in order (1, 2, 3)
and the cumulative ACK jumps to 3 with an empty bitmap.

### End of Stream with Retransmission (Immediate ACK)

```
    Sender                                      Receiver
      │                                            │
      │────── DATA [seq=0] ───────────────────────>│
      │────── DATA [seq=1] ────────X               │  (lost)
      │────── DATA [seq=2] ───────────────────────>│
      │────── EOS  [seq=3] ───────────────────────>│
      │<───────────────────── ACK [cum=0, bmp=0] ──│
      │<───────────────────── ACK [cum=0, bmp=110]─│  got 2 and 3, missing 1
      │                                            │
      │  (timeout for seq=1)                       │
      │                                            │
      │────── DATA [seq=1] ───────────────────────>│  (retransmit)
      │                                            │
      │<───────────────────── ACK [cum=3, bmp=0] ──│  all done, including EOS
      │                                            │
      │         Transfer complete                  │
```

The EOS packet has its own sequence number (3) and is acknowledged like any DATA
packet. The cumulative ACK can only reach the EOS sequence number after all prior
data is received. When the sender sees `cum=3`, it knows all data packets (0, 1, 2)
and the EOS were received - the transfer is complete. This requires no special
logic; the existing ACK mechanism handles it naturally.

<!-- TODO: Document SRSPP behavior during network partitions (e.g., solar flare
     knocks out a segment of the torus). Does OrbitAwareRto back off until the
     next contact window, or does the sender eventually drop the stream after
     max retransmits? Document the interaction between partition duration and
     max_retransmits × RTO. -->
