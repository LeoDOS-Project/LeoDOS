# Time

A spacecraft has no network time server, no GPS signal in every orbit, and a local oscillator that drifts. Despite this, every telemetry packet needs an accurate timestamp, every [scheduled](/cfs/mission/scheduling) action must fire at the right moment, and a constellation of spacecraft must agree on what time it is. cFS addresses this with a layered time model: a free-running local counter, offsets that convert it to absolute time, and a synchronization mechanism that corrects drift.

## The Local Clock

At the base is Mission Elapsed Time (MET) — a free-running counter that starts at zero when the spacecraft powers on and increments monotonically. MET never jumps, never adjusts, and never goes backward. It is the raw tick count from the [processor's](/cfs/mission/processor) hardware timer. Everything else is derived from MET.

## Absolute Time

MET alone does not tell you what time it is in the real world. Two offsets convert it:

- **STCF (Spacecraft Time Correlation Factor)** — added to MET to produce TAI (International Atomic Time). This offset is set during mission commissioning and updated when drift is corrected. TAI is monotonic and has no leap seconds, making it the preferred time scale for onboard computations.
- **Leap seconds** — subtracted from TAI to produce UTC. UTC matches wall-clock time on the ground but can jump when a leap second is applied. Missions choose whether telemetry timestamps use TAI or UTC.

## Drift and Synchronization

The local oscillator drifts — typically a few milliseconds per day. Left uncorrected, timestamps diverge from real time and scheduled events shift. cFS corrects drift using an external time source:

- **1PPS tone** — a one-pulse-per-second signal from a GPS receiver (when available) or a ground-uplinked time command. The tone marks the exact second boundary.
- **Time-at-the-tone** — a companion data message carries the absolute time value corresponding to the next (or most recent) tone.

The time service compares the expected tone arrival against the actual arrival, computes the drift, and adjusts STCF. Between corrections, MET continues to free-run. The result is that onboard time tracks real time to within the precision of the tone source — microseconds with GPS, milliseconds with ground uplink.

## Resolution

Time is represented internally as seconds plus a 32-bit fractional subsecond, giving a resolution of approximately 0.23 nanoseconds. This exceeds the precision of any onboard clock but ensures that arithmetic on timestamps (differences, interpolation, scheduling calculations) never loses resolution through rounding.

## Time in a Constellation

In a multi-spacecraft constellation, each vehicle maintains its own MET and STCF independently. Because all vehicles synchronize to the same external time source (GPS or ground), their absolute timestamps converge. The communication stack timestamps packets at the source, and the receiving spacecraft can compare those timestamps against its own time to measure one-way delay or detect clock divergence.

## Telemetry Encoding

Timestamps in telemetry headers are encoded per the CCSDS time code formats, which specify an epoch, the number of octets for seconds, and the number of octets for subseconds. The time service handles conversion between its internal representation and the wire format. For details on the encoding, see [Time Codes](/protocols/composition/time-codes).
