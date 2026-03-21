# LTP

The Licklider Transmission Protocol (CCSDS 734.1-B-1, RFC 5326)
is a convergence layer protocol for the Bundle Protocol. It
provides reliable delivery of bundles over a single hop, sitting
between BP and the packet layer (SPP / Encapsulation). LTP is
designed for deep-space links where round-trip times are measured
in minutes or hours and link availability is scheduled.

LTP is not a general-purpose transport protocol — it exists
solely to serve BP as a convergence layer adapter.

## Red and Green Data

Each transfer is a **block** split into two contiguous parts:

- **Red part** (prefix) — reliable. Acknowledged and
  retransmitted if lost. Used for bundle headers and payload
  data that must arrive intact.
- **Green part** (suffix) — unreliable. Fire-and-forget with
  no retransmission. Used for data that is acceptable to lose
  (e.g. trailing padding, expendable telemetry).

A block can be all-red (fully reliable), all-green (fully
unreliable), or a mix. The split point is chosen by the
application.

## Sessions

Each block transfer is a unidirectional **session** between two
LTP engines. Data flows from sender to receiver; acknowledgments
flow back. Sessions are identified by the originating engine ID
and a session number.

## Checkpoints and Reports

Reliability works through checkpoints and reception reports:

1. The sender segments the block and transmits segments. Certain
   red-part segments are marked as **checkpoints** that solicit
   a response. The last red segment (End of Red Part, EORP) is
   always a checkpoint. Additional **discretionary checkpoints**
   can be inserted earlier for faster feedback.

2. The receiver responds to each checkpoint with a **report
   segment** containing **reception claims** — (offset, length)
   ranges of data successfully received within a scope. The
   sender diffs these against what it sent to identify gaps.

3. The sender **retransmits** only the missing segments. The
   last retransmitted segment becomes a new checkpoint,
   triggering another report. This repeats until the receiver
   has the complete red part.

4. The sender acknowledges each report segment with a **report
   acknowledgment**, allowing the receiver to discard its
   tracking state for that scope.

## Timers

Checkpoint timers account for round-trip light time plus
processing delay. They suspend during known link outages
(scheduled contacts) and resume when the link is available
again, avoiding unnecessary retransmission during predictable
gaps.

## Cancellation

Either side can cancel a session with a reason code (user
request, unreachable peer, retransmission limit exceeded,
protocol error). The peer acknowledges with a
cancel-acknowledgment and both sides release session state.

## Comparison with SRSPP

| | SRSPP | LTP |
|---|---|---|
| Role | Transport protocol | BP convergence layer |
| Reliability | Cumulative ACK + selective bitmap | Reception claims (byte ranges) |
| Granularity | Per-packet sequence numbers | Byte offsets within a block |
| Red/green split | No (all reliable) | Yes |
| Designed for | LEO ISL (ms-s RTT) | Deep space (min-hours RTT) |

In a LEO constellation, SRSPP serves the same reliable delivery
role that LTP serves for deep-space DTN. LTP would be relevant
for interoperability with existing DTN networks or relay
scenarios involving deep-space links.
