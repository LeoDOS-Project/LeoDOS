#![cfg_attr(not(feature = "std"), no_std)]

//! # leodos-libcsp: Rust Bindings for the Cubesat Space Protocol
//!
//! This crate provides safe Rust bindings for [libcsp](https://github.com/libcsp/libcsp),
//! a small network-layer delivery protocol designed for CubeSats and other embedded systems.
//!
//! ## Features
//!
//! - **`rdp`**: Enable the Reliable Datagram Protocol for connection-oriented communication
//! - **`hmac`**: Enable HMAC authentication
//! - **`promisc`**: Enable promiscuous mode for packet sniffing
//!
//! ## Modules
//!
//! - [`csp`] - Core CSP functions (init, connect, ping, transactions)
//! - [`types`] - Core types (Packet, Connection, Socket)
//! - [`rtable`] - Routing table management
//! - [`iface`] - Network interface management
//! - [`time`] - Time and clock functions
//! - [`crc`] - CRC32 utilities
//! - [`promisc`] - Promiscuous mode for packet sniffing
//! - [`rdp`] - RDP protocol options
//! - [`config`] - CSP configuration access
//!
//! ## Quick Start
//!
//! ```ignore
//! use leodos_libcsp::{csp, Socket, Connection, Priority, ConnectOpts};
//!
//! // Initialize CSP
//! csp::init();
//!
//! // Server side
//! let mut socket = Socket::default();
//! socket.bind(10)?;
//! socket.listen(5)?;
//!
//! // Accept connection
//! if let Some(conn) = socket.accept(1000) {
//!     if let Some(packet) = conn.read(1000) {
//!         println!("Received {} bytes", packet.len());
//!     }
//! }
//!
//! // Client side - simple transaction
//! let mut response = [0u8; 64];
//! let n = csp::transaction(
//!     Priority::Normal,
//!     2,           // destination node
//!     10,          // destination port
//!     1000,        // timeout
//!     b"request",  // outgoing data
//!     &mut response,
//!     ConnectOpts::NONE,
//! )?;
//! ```

pub(crate) mod ffi;

pub mod bridge;
pub mod cmp;
pub mod config;
pub mod crc;
pub mod crypto;
pub mod csp;
pub mod error;
pub mod id;
pub mod if_udp;
pub mod iface;
pub mod promisc;
pub mod rdp;
pub mod rtable;
pub mod sfp;
pub mod time;
pub mod types;

pub use csp::{
    bind_callback, buffer_get, buffer_remaining, bytesize, connect, get_buf_free, get_memfree,
    get_uptime, hex_dump, init, ping, ping_noreply, print_buf_free, print_connections,
    print_memfree, print_ps, print_uptime, reboot, route_work, sendto, sendto_reply,
    service_handler, shutdown, transaction, transaction_no_reply, transaction_persistent,
};
pub use error::{Error, Result};
pub use types::{ConnectOpts, Connection, Packet, Priority, Socket, SocketOpts, ANY_PORT};
