# Protocol Sequences

## Normal Operation (Immediate ACK)

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

## Normal Operation (Delayed ACK)

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

## Packet Loss and Retransmission (Immediate ACK)

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

## Segmented Message (Delayed ACK)

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

## Out-of-Order Delivery with Selective ACK (Immediate ACK)

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

## End of Stream with Retransmission (Immediate ACK)

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

## TODO

- Document SRSPP behavior during network partitions (e.g., a solar flare knocks out a segment of the torus). Does OrbitAwareRto back off until the next contact window, or does the sender eventually drop the stream after max retransmits? Document the interaction between partition duration and max_retransmits x RTO.
