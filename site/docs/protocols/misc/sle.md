# Space Link Extension (SLE)

The CCSDS Space Link Extension protocol (911.1-B-4, 911.2-B-3)
provides a standardized interface between a mission control center
and ground station antennas. Where the other protocol layers handle
the space link itself, SLE handles the ground segment: how mission
software connects to a remote antenna to send commands or receive
telemetry.

SLE runs over TCP using ISP1 (Internet SLE Protocol 1) framing.
Each message is a length-prefixed BER-encoded PDU. The protocol
follows a client-provider model where the mission control center
(client) binds to a service instance on the ground station
(provider).

## Services

Two services are supported:

- **RAF** (Return All Frames) — the provider delivers downlink
  telemetry frames to the client. The client can filter by frame
  quality (good, erred, or all). Frames arrive in transfer buffers
  containing one or more annotated TM frames with earth receive
  times and data link continuity indicators.

- **CLTU** (Forward CLTU) — the client sends Command Link
  Transmission Units to the provider for uplink radiation. Each
  CLTU is assigned a sequence ID and the provider reports whether
  the unit was radiated, expired, or interrupted.

## Session lifecycle

A session proceeds through three phases:

1. **Bind** — the client authenticates with optional SHA-1
   credentials (time + nonce + password hash) and identifies the
   service instance by its OID path (e.g. `sagr=1.spack=1.raf=onlt1`).
   The provider responds with a bind result (success, access
   denied, unsupported service type, or version mismatch).

2. **Start/Transfer** — the client starts the service. For RAF,
   this includes optional start/stop time bounds and a frame
   quality filter. For CLTU, this includes the first CLTU
   sequence ID. Data then flows: transfer buffers for RAF,
   transfer data invocations and returns for CLTU.

3. **Stop/Unbind** — the client stops the service and unbinds
   from the provider.

## ISP1 framing

ISP1 provides a simple transport layer over TCP. Each PDU is
prefixed with a 4-byte big-endian length field:

| Field  | Size    | Description          |
|--------|---------|----------------------|
| Length | 4 bytes | Payload size (BE u32)|
| PDU    | N bytes | BER-encoded SLE PDU  |

## Authentication

Bind invocations carry optional credentials: an 8-byte CDS
timestamp, a 4-byte random nonce, and a 20-byte SHA-1 hash
computed over the concatenation of time, nonce, and a shared
password. This prevents replay attacks and authenticates the
caller without transmitting the password.

## BER encoding

All SLE PDUs use ASN.1 Basic Encoding Rules. Each operation
is wrapped in a context-specific tagged CHOICE that identifies
the operation type (bind, start, transfer data, etc.). Fields
within each PDU are standard BER types: INTEGER, OCTET STRING,
ENUMERATED, SEQUENCE, and NULL.
