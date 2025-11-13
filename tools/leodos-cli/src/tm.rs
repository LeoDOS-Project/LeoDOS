use anyhow::Result;
use leodos_protocols::network::cfe::tm::Telemetry;
use leodos_protocols::network::spp::SpacePacket;
use std::net::UdpSocket;
use zerocopy::FromBytes;

const RUST_HK_TLM_MID: u16 = 0x08FA;

pub async fn listen(port: u16) -> Result<()> {
    let listen_addr = format!("0.0.0.0:{}", port);
    let socket = UdpSocket::bind(&listen_addr)?;
    println!("Listening on {}", listen_addr);

    let mut buffer = [0u8; 2048];

    loop {
        let (num_bytes, src_addr) = socket.recv_from(&mut buffer)?;
        let raw_bytes = &buffer[..num_bytes];

        println!("\n--- Received {} bytes from {} ---", num_bytes, src_addr);

        match SpacePacket::parse(raw_bytes) {
            Ok(packet) => {
                // Reconstruct the full message ID from the primary header fields.
                let message_id = packet.cfe_msg_id();
                println!(
                    "Packet MID: {:#06x} (APID: {:#05x})",
                    message_id,
                    packet.apid()
                );

                match message_id {
                    RUST_HK_TLM_MID => {
                        println!("  -> Recognized as Housekeeping Telemetry");
                        if let Ok(hk_packet) = Telemetry::ref_from_bytes(raw_bytes) {
                            println!("{:#?}", hk_packet);
                        } else {
                            println!(
                                "  [ERROR] Packet matched HK MID but had an invalid size/layout."
                            );
                        }
                    }
                    _ => {
                        println!("  -> MID not recognized by this tool.");
                    }
                }
            }
            Err(e) => {
                eprintln!("[ERROR] Failed to parse CCSDS Space Packet: {}", e);
            }
        }
    }
}
