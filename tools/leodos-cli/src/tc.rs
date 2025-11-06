use anyhow::{anyhow, Result};
use leodos_spacepacket::cfe::tc::Telecommand;
use leodos_spacepacket::{Apid, PacketSequenceCount};
use std::mem::size_of;
use std::net::UdpSocket;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

// Define the empty payload struct needed for no-argument commands
#[repr(C)]
#[derive(IntoBytes, FromBytes, Unaligned, KnownLayout, Immutable, Default, Copy, Clone)]
struct NoArgsPayload {}

pub async fn send(
    host: &str,
    port: u16,
    mid: u16,
    fcn_code: u8,
    params: &[String],
) -> Result<()> {
    // For now, we assert that no parameters are given.
    // A more advanced version would parse the `params` Vec here.
    if !params.is_empty() {
        return Err(anyhow!(
            "This tool does not yet support command parameters."
        ));
    }

    // 1. Derive the APID from the Message ID (MID).
    // This is a standard cFE convention. The APID is part of the MID.
    let apid = Apid::new(mid & 0x07FF).expect("Should be a valid APID");

    // 2. Define the payload for this specific command.
    let payload = NoArgsPayload::default();

    // 3. Allocate a buffer and build the packet using your packet library.
    type NoArgsCommand = Telecommand<NoArgsPayload>;
    let mut buffer = vec![0u8; size_of::<NoArgsCommand>()];

    let _packet = Telecommand::new(
        &mut buffer,
        apid,
        PacketSequenceCount::from(1), // A real tool would manage this count
        fcn_code,
        &payload,
    )
    .expect("Failed to build Telecommand packet");

    // 4. Send the packet over UDP.
    let socket = UdpSocket::bind("0.0.0.0:0")?; // Bind to any available local port
    socket.send_to(&buffer, (host, port))?;

    println!("Successfully sent command with MID {:#06x}", mid);

    Ok(())
}
