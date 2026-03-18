# Reliability

The reliability mechanisms at different layers are complementary,
not redundant. Each layer handles a class of failure that the
layers below cannot see.

## Why COP-1 alone is not sufficient

COP-1 runs independently on each hop. Consider a three-hop path:

```
Sat A → Sat B → Sat C → Sat D
```

COP-1 on the A--B link confirms that Sat B received the frame. But
Sat B's router may drop the packet before forwarding it to Sat C
— due to queue overflow, a software fault, or a route change.
COP-1 on A--B has already reported success. Neither the sender (A)
nor the final receiver (D) knows the packet was lost.

Only an end-to-end protocol (SRSPP) between A and D can detect and
recover from this. SRSPP's sequence numbers span the entire path,
so D knows exactly which packets it has received and can request
retransmission from A.

## Why SRSPP alone is not sufficient

Without per-hop reliability, SRSPP must retransmit every packet
lost to bit errors. On a lossy RF link this means:

- Each retransmission traverses the full multi-hop path, consuming
  bandwidth on every intermediate link.
- Each retransmission is itself subject to the same per-hop loss
  rate.
- Throughput degrades geometrically with hop count: a 1% frame
  loss rate per hop becomes $(1 - 0.99^n)$ end-to-end loss for an
  $n$-hop path.

With COP-1 on each hop, frame losses are recovered locally in one
link round-trip time. SRSPP only needs to retransmit when an
intermediate router drops a packet — a much rarer event than a
bit error.

## Recovery summary

1. A bit error on the RF link is corrected by the coding layer's
   FEC. No retransmission occurs.
2. If FEC cannot correct the damage, the corrupted frame is
   discarded. COP-1 detects the gap in the frame sequence and
   retransmits the frame on the same hop.
3. If a packet survives all hops but is dropped at an intermediate
   router, SRSPP detects the missing sequence number and
   retransmits end-to-end.

Each layer handles only the residual failures of the layer below.
The result is that SRSPP retransmissions are rare, but they remain
necessary for correctness in a multi-hop network.
