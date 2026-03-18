# Overview

The core Flight Executive (cFE) is the top layer of cFS, providing five services that every flight application can use. It sits on top of OSAL and handles everything an application needs beyond basic OS primitives: lifecycle management, message routing, structured logging, runtime configuration, and time synchronization.

## The Five Services

- **[Executive Services (ES)](es)** — manages application lifecycle: startup, run loop, shutdown, restart, and health monitoring. Also provides the Critical Data Store for data that must survive processor resets.
- **[Software Bus (SB)](sb)** — publish-subscribe message passing between applications. Apps subscribe to message IDs and receive messages through pipes, decoupling senders from receivers.
- **[Event Services (EVS)](evs)** — structured logging with severity levels and rate filtering. Events are sent over the Software Bus and downlinked as telemetry.
- **[Table Services (TBL)](tbl)** — runtime-configurable data separated from code. Tables can be loaded, validated, and updated from the ground without restarting the application.
- **[Time Services (TIME)](time)** — mission-synchronized time with subsecond precision. Provides a common time reference across all applications and encodes timestamps for telemetry headers.

## How Apps Interact with cFE

A cFS application follows a standard pattern. At startup, it registers with Executive Services, creates a Software Bus pipe, and subscribes to the command message IDs it handles. In its main loop, it waits for messages on its pipe and processes them — executing commands, publishing telemetry, and emitting events. Configuration data comes from tables rather than compiled constants, so the ground can update behavior without uploading new code. When the application shuts down (or is restarted by ES after a fault), it stores critical state in the CDS so it can resume where it left off.
