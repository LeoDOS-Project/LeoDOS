//! Implements the CCSDS Telecommand (TC) and Telemetry (TM) Data Link Protocols.
//!
//! This module provides zero-copy views and builders for creating and parsing
//! TC/TM Transfer Frames, which are the "envelopes" used to transport
//! `SpacePacket`s over a radio link. It also includes the necessary utilities
//! for randomization and CLTU (uplink) encoding.
//!
//! # Workflow
//!
//! **Uplink (Sending a command to a satellite):**
//! 1.  Build a `SpacePacket` with the command payload.
//! 2.  Build a `TCTransferFrame`, which acts as an envelope.
//! 3.  Copy the serialized `SpacePacket` into the `TCTransferFrame`'s data field.
//! 4.  (Optional but recommended) Apply randomization to the `TCTransferFrame` bytes using the `randomizer` module.
//! 5.  Encode the `TCTransferFrame` bytes into a CLTU using the `cltu` module.
//! 6.  Send the resulting CLTU bytes to the radio transmitter.
//!
//! **Downlink (Receiving telemetry from a satellite):**
//! 1.  Receive the raw TM Transfer Frame bytes from the radio.
//! 2.  (Optional but recommended) Apply de-randomization to the bytes.
//! 3.  Parse the bytes into a `TMTransferFrame` view.
//! 4.  The `TMTransferFrame`'s data field now contains one or more `SpacePacket`s,
//!     which can be parsed in turn.
//!
pub mod cltu;
pub mod randomizer;
pub mod tctf;
pub mod tmtf;

pub use tctf::TelecommandTransferFrame;
pub use tmtf::TelemetryTransferFrame;
