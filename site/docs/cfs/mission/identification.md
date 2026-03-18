# Identification

Every entity in a cFS mission — spacecraft, apps, messages, data streams — has a numeric identifier. These identifiers are how the ground distinguishes one spacecraft from another, how the bus routes messages to the right app, and how telemetry is attributed to its source.

## Spacecraft ID

Each spacecraft in a constellation has a unique Spacecraft ID assigned at mission configuration time. The Spacecraft ID appears in every [transfer frame](/protocols/datalink/transfer-frame/overview) and [Space Packet](/protocols/datalink/packet/spp) leaving the vehicle, so the ground (and other spacecraft) can identify the source without ambiguity. In a constellation, the Spacecraft ID is also used by the [routing layer](/protocols/network/routing) to address packets to a specific node.

## Application IDs

Each app on the bus is associated with one or more Application Process Identifiers (APIDs). An APID identifies a data stream — not the app itself, but the type of data it produces or consumes. A single app may own multiple APIDs (one for housekeeping telemetry, another for science data, another for its command input). APIDs appear in [Space Packet](/protocols/datalink/packet/spp) headers and are the basis for message routing both on the bus and across the communication link.

The APID space is partitioned by convention: ranges are reserved for cFE internal services, standard apps, and mission-specific apps. This prevents collisions when integrating apps from different sources.

## Message IDs

The [Software Bus](/cfs/cfe/sb) routes messages by Message ID (MsgId), which combines the APID with a command/telemetry flag and optional secondary header information. MsgIds are the internal addressing scheme — an app subscribes to a MsgId, not to another app. This indirection is what allows the bus to be reconfigured (rerouting data to a different consumer) without changing application code.

## How They Relate

The identifiers form a hierarchy:

- **Spacecraft ID** identifies the vehicle — used in frame headers for inter-spacecraft and ground communication.
- **APID** identifies a data stream within a spacecraft — used in packet headers.
- **MsgId** identifies a message type on the bus — used internally for publish-subscribe routing.

When telemetry leaves the spacecraft, the packet carries its APID and the frame carries the Spacecraft ID. The ground system uses both to route the data to the correct processing pipeline. When a command arrives, the reverse happens: the ground addresses it by Spacecraft ID and APID, and the bus maps the APID to the correct MsgId and delivers it to the subscribing app.
