# Software Bus

The Software Bus (SB) is the publish-subscribe message passing system at the heart of cFS. Every inter-application message — commands, telemetry, internal notifications — flows through the SB, decoupling senders from receivers.

## Key Concepts

- **MsgId** — a numeric topic identifier. Each message type (housekeeping telemetry, a specific command, a sensor reading) has a unique MsgId. Senders publish to a MsgId; receivers subscribe to it.
- **Pipe** — a subscription endpoint owned by an application. An app creates a pipe with a configurable depth, subscribes it to one or more MsgIds, and reads messages from it in a loop.
- **QoS** — quality of service hints (reliability, priority) attached to subscriptions. In practice, all messages on the SB are delivered in-order within a pipe.

## How It Works

An application creates a pipe, subscribes to the MsgIds it cares about, and enters a loop calling receive on the pipe. When any other application (or cFE itself) sends a message with a matching MsgId, the SB routes a pointer to that message into every subscribed pipe. The message stays in the SB's internal buffer — no copy is made for each subscriber — until all subscribers have processed it.

## Command and Telemetry Messages

By convention, command messages and telemetry messages use separate MsgId ranges. Command messages carry instructions (e.g., "start a scan", "update a table"), while telemetry messages carry state reports (e.g., housekeeping counters, sensor readings). Both flow through the same SB infrastructure.

## Zero-Copy Design

Where possible, the SB avoids copying message data. The sender writes into a buffer obtained from the SB, and subscribers receive a pointer to that same buffer. This is important for high-rate telemetry on resource-constrained processors where memory bandwidth is limited.
