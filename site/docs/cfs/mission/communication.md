# Communication

Apps in a cFS mission do not call each other directly. All inter-app communication flows through the [Software Bus](/cfs/cfe/sb). This design addresses three concerns: decoupling, data flow conventions, and backpressure.

## Decoupling

An app that produces data publishes a message to a topic on the bus. Any app that cares about that data subscribes to the same topic and receives the message. The sender does not know who is subscribed, and subscribers do not know who sent the message. This decoupling is what allows apps to be added, removed, or restarted independently — no app holds a direct reference to any other app.

## Commands and Telemetry

By convention, messages fall into two categories:

- **Commands** — instructions sent to an app (start a scan, reset a counter, load a table). Command messages typically originate from the ground via the Command Ingest app, or from the Stored Command app for autonomous sequences.
- **Telemetry** — state reports published by an app (housekeeping counters, sensor readings, event messages). Telemetry flows to the Telemetry Output app for downlink, and may also be consumed by other apps onboard.

Both categories use the same bus infrastructure. The distinction is a naming convention on topic identifiers, not a separate mechanism.

## Backpressure

Each app receives messages through a bounded queue with a fixed depth. If a consumer falls behind and its queue fills up, new messages to that queue are dropped and an error event is emitted. This prevents a slow consumer from blocking producers or causing unbounded memory growth. The system degrades gracefully — a stalled app loses messages, but every other app continues unaffected.

Where possible, the bus avoids copying message data. Producers write into a buffer obtained from the bus, and all subscribers receive pointers to that same buffer. The buffer is released when every subscriber has finished processing. This matters for high-rate telemetry on resource-constrained processors.
