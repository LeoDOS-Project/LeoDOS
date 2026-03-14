use anyhow::Result;
use leodos_protocols::network::spp::SpacePacket;
use tokio::net::UdpSocket;
use tokio::time::{Duration, timeout};

pub async fn run(
    host: &str,
    cmd_port: u16,
    tlm_port: u16,
) -> Result<()> {
    // Listen for telemetry response
    let tlm_sock =
        UdpSocket::bind(format!("0.0.0.0:{tlm_port}")).await?;

    // Send CFE_ES_SEND_HK command
    // CFE_ES_SEND_HK_MID = 0x1800 (topic-dependent)
    let es_send_hk_mid: u16 = 0x1800;
    println!("Requesting ES housekeeping from {host}:{cmd_port}...");
    crate::tc::send(host, cmd_port, es_send_hk_mid, 0, &[], false)
        .await?;

    // Wait for HK telemetry response
    let mut buf = [0u8; 2048];
    match timeout(Duration::from_secs(3), tlm_sock.recv_from(&mut buf))
        .await
    {
        Ok(Ok((len, src))) => {
            println!("Received {len} bytes from {src}");
            match SpacePacket::parse(&buf[..len]) {
                Ok(pkt) => {
                    println!(
                        "  MID: {:#06x}  APID: {:#05x}  Len: {}",
                        pkt.cfe_msg_id(),
                        pkt.apid(),
                        pkt.primary_header.packet_len(),
                    );
                    let payload = pkt.data_field();
                    if !payload.is_empty() {
                        print!("  Payload:");
                        for b in payload.iter().take(64) {
                            print!(" {:02x}", b);
                        }
                        if payload.len() > 64 {
                            print!(" ...");
                        }
                        println!();
                    }
                }
                Err(e) => {
                    eprintln!("  Parse error: {e:?}");
                }
            }
        }
        Ok(Err(e)) => {
            eprintln!("Socket error: {e}");
        }
        Err(_) => {
            println!(
                "No response within 3s. \
                 Is cFS running and TO_LAB enabled?"
            );
        }
    }

    Ok(())
}
