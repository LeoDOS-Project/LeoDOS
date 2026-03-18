# Security

Satellite RF links are inherently broadcast — anyone with a suitable
antenna and knowledge of the downlink frequency can receive the signal.
Uplinks are similarly exposed: without authentication, a ground-based
attacker could inject commands that the spacecraft would execute.

SDLS (Space Data Link Security) addresses both threats at the frame
level by encrypting and authenticating each transfer frame. This page
describes how security interacts with the other layers in the stack.

## Position in the Pipeline

SDLS is applied after the transfer frame is constructed but before
the frame reaches the coding layer:

```
Application data
  → Transport (SRSPP / CFDP)
  → Network (routing)
  → Data Link (frame construction)
  → **SDLS (encrypt + MAC)**
  → Coding (randomize → FEC → framing)
  → Physical (modulation → radio)
```

This ordering has two important consequences:

1. **FEC protects the ciphertext.** The coding layer's error
   correction operates on the encrypted frame. If a bit error
   corrupts the ciphertext, FEC corrects it before SDLS ever
   sees the data. SDLS only processes intact ciphertext, which
   simplifies the security processing and avoids the ambiguity
   of decrypting partially corrupted data.

2. **MAC prevents forged frames.** Even if an attacker corrupts
   the RF signal in a way that FEC "corrects" into valid-looking
   ciphertext, the MAC verification will fail because the attacker
   does not know the key. The receiver discards the frame before
   any higher layer processes it.

## Interaction with Reliability

COP-1 retransmits frames that are lost or corrupted beyond FEC
recovery. Because SDLS is applied before COP-1 sequencing, each
retransmitted frame carries the same encrypted content and MAC.
This means:

- **Retransmissions do not expose plaintext.** An observer who
  captures both the original and the retransmitted frame sees
  identical ciphertext.
- **COP-1 sequence numbers are outside the encrypted region.**
  The frame header (including the COP-1 sequence counter) is
  authenticated by the MAC but not encrypted, so intermediate
  nodes can read the sequence number without decrypting the payload.

## Interaction with Routing

The network layer routes Space Packets, not transfer frames.
Routing decisions are made before the packet is placed into a
frame and before SDLS is applied. This means:

- **Intermediate routers never see encrypted data.** They forward
  Space Packets at the network layer. SDLS only applies to the
  point-to-point link between adjacent nodes.
- **Each hop has its own Security Association.** A packet traversing
  three hops is encrypted and decrypted three times, once per link.
  This is necessary because each link may use different keys and
  because the transfer frame headers change at each hop.

## Key Management

Each Security Association (SA) binds a key, algorithm, and IV
management policy to a specific link. In a constellation:

- **ISL keys** are shared between adjacent satellites. A Walker
  Delta constellation with four neighbors per satellite needs
  four SAs per node (north, south, east, west).
- **Ground keys** are shared between a satellite and its ground
  station. These may be rotated more frequently since ground
  contacts are intermittent.
- **IV uniqueness** is critical. AES-GCM requires that the same
  (key, IV) pair is never reused. The IV is typically a counter
  that increments with each frame. After a reboot, the counter
  must resume from a value guaranteed to be higher than any
  previously used — the cFE Critical Data Store can persist the
  last-used IV for this purpose.

Key distribution and rotation are mission-specific and outside the
scope of SDLS itself. Pre-loaded keys with periodic ground-commanded
updates are the simplest approach for a LEO constellation.
