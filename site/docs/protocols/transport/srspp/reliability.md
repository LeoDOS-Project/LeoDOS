# Reliability Mechanism

## Sequence Numbers

SRSPP uses the 14-bit sequence count field from the SPP primary header for reliability.
Sequence numbers wrap around at 16383 (0x3FFF).

## Acknowledgments

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

## Retransmission

The sender maintains a retransmission timer for each unacknowledged packet. When a
timer expires, the packet is retransmitted. After a configurable number of failed
retransmissions, the packet is considered lost.

## ACK Timing

The receiver can be configured for:
- **Immediate ACK**: Send ACK immediately upon receiving any data packet
- **Delayed ACK**: Wait for a timeout before sending ACK (allows batching)

## Segmentation

Large messages that exceed the MTU are segmented using the SPP sequence flags:

- **Unsegmented** (0b11) — complete message in one packet
- **First** (0b01) — first segment of a multi-packet message
- **Continuation** (0b00) — middle segment
- **Last** (0b10) — final segment

The receiver reassembles segments in order. All segments of a message share consecutive sequence numbers.

## Flow Control

SRSPP uses a sliding window for flow control:

- **Window size (WIN)**: Maximum number of unacknowledged packets in flight
- **Send buffer (BUF)**: Total bytes that can be queued for transmission

The sender blocks when either the window is full or the buffer is exhausted.
