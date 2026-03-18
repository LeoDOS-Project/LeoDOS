# Time Services

Time Services (TIME) provides a synchronized time reference across all applications in the cFS system. Every telemetry timestamp, scheduled event, and time-tagged command uses TIME to ensure consistency within the spacecraft and across a constellation.

## Time References

TIME maintains several related time values:

- **MET (Mission Elapsed Time)** — a free-running counter that starts at zero when the spacecraft powers on. MET never jumps or adjusts — it only increments. All other time values are derived from MET plus offsets.
- **TAI (International Atomic Time)** — MET plus a fixed offset set during mission commissioning. TAI is a monotonic time scale with no leap seconds.
- **UTC (Coordinated Universal Time)** — TAI minus the current leap second count. UTC matches wall-clock time on the ground but can jump when leap seconds are applied.
- **Spacecraft Time (SCT)** — the time value used in telemetry headers. Missions configure whether SCT tracks TAI or UTC.

## Subsecond Resolution

TIME represents subseconds as a 32-bit fraction, giving a resolution of approximately 0.23 nanoseconds (1/2^32 seconds). This exceeds the precision of any onboard clock but ensures that timestamps never lose resolution through rounding during arithmetic operations.

## Tone Synchronization

Spacecraft clocks drift. TIME corrects this drift using an external time source — typically a 1PPS (one pulse per second) signal from a GPS receiver or a ground-uplinked time command. The "tone" marks the exact second boundary; a companion data message carries the absolute time value. TIME uses the difference between the expected and actual tone arrival to compute and apply a drift correction.

## CCSDS Time Encoding

Telemetry headers carry timestamps encoded per the CCSDS time code formats (CCSDS 301.0-B-4). The encoding specifies an epoch, the number of octets for seconds, and the number of octets for subseconds. TIME handles the conversion between its internal representation and the wire format. For details on the encoding, see [Time Codes](/protocols/composition/time-codes).
