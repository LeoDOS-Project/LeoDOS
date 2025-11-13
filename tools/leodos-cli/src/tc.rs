use anyhow::anyhow;
use anyhow::Result;
use leodos_protocols::network::cfe::tc::Telecommand;
use leodos_protocols::network::cfe::tm::Telemetry;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::network::spp::SequenceCount;
use tokio::net::UdpSocket;
use zerocopy::IntoBytes;

pub async fn send(
    host: &str,
    port: u16,
    mid: u16,
    fcn_code: u8,
    payload: &[u8],
    expect_response: bool,
) -> Result<()> {
    let apid = Apid::new(mid).expect("Should be a valid APID");
    let mut buffer = [0u8; 256];

    let packet = Telecommand::builder()
        .buffer(&mut buffer)
        .apid(apid)
        .sequence_count(SequenceCount::new())
        .function_code(fcn_code)
        .payload_len(payload.len())
        .build()
        .expect("Should build Telecommand packet");

    packet.payload_mut().copy_from_slice(payload);
    packet.set_cfe_checksum();

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let local_addr = socket.local_addr()?;

    socket.send_to(packet.as_bytes(), (host, port)).await?;

    println!("Successfully sent command (MID {mid:#06x}) from {local_addr}",);

    if expect_response {
        println!("Waiting for telemetry response...");

        match socket.recv_from(&mut buffer).await {
            Ok((num_bytes, src_addr)) => {
                println!("\nResponse Received ({num_bytes} bytes from {src_addr})",);
                match Telemetry::parse(&buffer[..num_bytes]) {
                    Ok(packet) => {
                        if let Ok(msg) = core::str::from_utf8(packet.payload()) {
                            println!("Response Payload: '{msg}'");
                        } else {
                            println!("Response Payload: [Binary Data]");
                        }
                        Ok(())
                    }
                    Err(e) => Err(anyhow!("Failed to parse reseponse as Telemetry: {e:?}",)),
                }
            }
            Err(e) => Err(anyhow!("Socket error while receiving response: {}", e)),
        }
    } else {
        Ok(())
    }
}
