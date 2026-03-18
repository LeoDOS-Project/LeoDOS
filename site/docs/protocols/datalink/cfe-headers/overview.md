# Overview

The core Flight Executive (cFE) defines mission-specific secondary
headers that extend the generic SPP primary header. These headers
occupy the SPP secondary header field and add metadata needed by
the cFE Software Bus for message routing and validation.

A **Telemetry** (TM) packet carries a secondary header containing a
timestamp (6 bytes) used by the ground system to correlate
telemetry with on-board time.

A **Telecommand** (TC) packet carries a secondary header containing
a function code (1 byte) that identifies the specific command
within the target application, and a checksum (1 byte) computed
over the entire packet to detect corruption.

The cFE Software Bus routes packets using a composite Message ID
derived from the SPP APID and packet type fields. This allows TM
and TC packets to share the APID space without ambiguity.

- [TM](tm) --- telemetry secondary header with timestamp
- [TC](tc) --- telecommand secondary header with function code and checksum
