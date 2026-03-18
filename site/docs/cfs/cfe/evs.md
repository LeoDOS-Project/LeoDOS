# Event Services

Event Services (EVS) provides structured logging for flight software. Instead of unstructured print statements, applications emit typed events with severity levels that are captured, filtered, and downlinked as telemetry.

## Event Types

Every event has a severity level:

- **Debug** — detailed diagnostic information, normally filtered out in flight
- **Information** — routine operational milestones (app started, table loaded, scan complete)
- **Error** — recoverable problems (invalid command, failed checksum, timeout)
- **Critical** — conditions that may require ground intervention (hardware fault, resource exhaustion)

## Filtering

High-rate events can overwhelm the downlink. Applications register event filters at startup that limit how many times a specific event can be emitted within a time window. For example, a sensor driver might filter a "reading out of range" event to at most one per second, even if the condition triggers every cycle. Filters can be updated from the ground without restarting the application.

## Events as Telemetry

Events are not just logged locally — they are packaged as telemetry messages and sent over the [Software Bus](/cfs/cfe/sb). This means they flow through the same downlink path as all other telemetry: they can be recorded onboard, prioritized for transmission, and displayed in ground tools alongside sensor data and housekeeping. This unified approach eliminates the need for separate logging infrastructure.
