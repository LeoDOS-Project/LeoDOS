# COP-1

Communications Operation Procedure-1 (CCSDS 232.1-B-2) provides
reliable frame delivery on a single hop. It is an ARQ (Automatic
Repeat reQuest) protocol — a scheme where the receiver
acknowledges each successfully received frame, and the sender
retransmits anything that is not acknowledged within a timeout. ARQ
uses a _sliding window_: the sender can have multiple frames
in flight simultaneously (up to the window size) without waiting
for each to be acknowledged individually.

The sender side is called FOP-1 (Frame Operation Procedure). It
assigns a sequence number to each TC frame, places it in a
retransmission buffer, and starts a timer. If the timer expires
before the receiver acknowledges the frame, FOP-1 retransmits it.

The receiver side is called FARM-1 (Frame Acceptance and Reporting
Mechanism). It maintains a window of expected sequence numbers. If
a frame arrives with the expected sequence number, FARM-1 accepts
it and advances the window. If a frame arrives out of order or
with a gap, FARM-1 can either reject it or buffer it depending on
configuration.

FARM-1 communicates its state back to the sender via the CLCW
(Command Link Control Word), a 32-bit field piggybacked on TM
frames. The CLCW reports the next expected sequence number and
status flags (e.g. "no RF available", "retransmit"). FOP-1 parses
the CLCW to determine which frames have been received and which
need retransmission.

COP-1 is essential for link-level recovery. Without it, every
frame lost due to uncorrectable bit errors would propagate up to
the transport layer, which would have to retransmit across the
entire multi-hop path. With COP-1, most losses are recovered in a
single link round-trip time.
