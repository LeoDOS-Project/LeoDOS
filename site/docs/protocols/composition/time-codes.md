# Time Codes

CCSDS 301.0-B-4 defines standard time formats for space missions.
Time stamps appear in cFE telemetry secondary headers (6 bytes),
in protocol metadata, and in science data annotations. Two formats
are used in LeoDOS: CUC and CDS.

Both formats use a two-part encoding. The _P-field_ (preamble) is a
1-byte descriptor that identifies the time format, epoch, and field
sizes. The _T-field_ contains the actual time value. When sender and
receiver agree on the format in advance, the P-field can be omitted
and the T-field is interpreted using the implicit configuration.

Both formats reference the CCSDS epoch: **1958-01-01T00:00:00 TAI**.
TAI (International Atomic Time) is a monotonic time scale that does
not include leap seconds, which is important for spacecraft where
a discontinuity in the time reference could cause control loops to
misbehave.

## CUC — Unsegmented Code

CUC encodes time as a binary count of whole seconds (the _coarse_
field) and fractional seconds (the _fine_ field) since the epoch.
The number of bytes for each is configurable: 1--4 bytes for coarse
and 0--3 bytes for fine. This flexibility allows trading off range
against resolution and encoded size.

The P-field encodes:

- **Time code ID** (3 bits): `001` for agency-defined epoch, `010`
  for the CCSDS epoch.
- **Coarse octets** (2 bits): the number of coarse bytes minus one
  (0--3, meaning 1--4 bytes).
- **Fine octets** (2 bits): the number of fine bytes (0--3).

Common configurations:

- **4+2** (standard): 4 coarse bytes give a range of ~136 years from
  epoch. 2 fine bytes give ~15 us resolution
  ($1 / 2^{16} \approx 15.3 \mu s$). Total T-field: 6 bytes.
- **4+0**: 4 coarse bytes, no fractional part. 1-second resolution.
  Total T-field: 4 bytes.

CUC is used in the cFE telemetry secondary header (6-byte
timestamp). It is the natural choice when the on-board clock
provides a seconds-and-ticks counter.

## CDS — Day Segmented

CDS encodes time as a day count since the epoch plus milliseconds
within the day. It optionally includes a sub-millisecond field for
higher resolution.

The P-field encodes:

- **Time code ID** (3 bits): `100` for CDS.
- **Epoch ID** (1 bit): 0 = CCSDS epoch, 1 = agency-defined.
- **Day segment length** (1 bit): 0 = 16-bit day count (range
  ~179 years), 1 = 24-bit day count (range ~45,000 years).
- **Sub-millisecond resolution** (2 bits): `00` = none, `01` = 16-bit
  microseconds (0--999 us), `10` = 32-bit picoseconds
  (0--999,999,999 ps).

Common configurations:

- **16-bit day, no sub-ms**: 2 day + 4 ms = 6-byte T-field.
  Millisecond resolution, ~179 year range.
- **16-bit day, us**: 2 day + 4 ms + 2 us = 8-byte T-field.
  Microsecond resolution.
- **24-bit day, ps**: 3 day + 4 ms + 4 ps = 11-byte T-field.
  Picosecond resolution, extended range.

CDS is suited for ground systems and science data where wall-clock
time (day + time-of-day) is more natural than a raw second count.

## Choosing Between CUC and CDS

CUC is compact and maps directly to hardware tick counters. CDS is
human-readable (day + milliseconds) and convenient for correlating
events with calendar dates. In practice, flight software uses CUC
for telemetry timestamps and CDS for ground-originated time
references and science data annotations.
