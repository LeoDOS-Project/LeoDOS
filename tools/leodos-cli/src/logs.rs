use anyhow::Result;
use leodos_protocols::network::spp::SpacePacket;
use std::net::UdpSocket;

// CFE_EVS_HK_TLM_MID and long event MID vary by mission config.
// We detect EVS packets by checking the APID range.
const _EVS_LONG_EVENT_APID_MASK: u16 = 0x08;

pub async fn run(port: u16) -> Result<()> {
    let addr = format!("0.0.0.0:{port}");
    let socket = UdpSocket::bind(&addr)?;
    println!("Streaming logs on {addr} (Ctrl+C to stop)\n");

    let mut buf = [0u8; 2048];

    loop {
        let (len, _src) = socket.recv_from(&mut buf)?;
        let raw = &buf[..len];

        let Ok(pkt) = SpacePacket::parse(raw) else {
            continue;
        };

        let mid = pkt.cfe_msg_id();
        let payload = pkt.data_field();

        // EVS long event packet has a known layout:
        //   TlmHeader (12 bytes already consumed by SpacePacket)
        //   PacketID: AppName[20] + EventID(u16) + EventType(u16)
        //   Message[122]
        // Total payload ~148 bytes
        if payload.len() >= 26 {
            // Try to extract as EVS event
            let app_name = extract_cstr(&payload[..20]);
            let msg_offset = 24; // after AppName + EventID + EventType
            if msg_offset < payload.len() {
                let message =
                    extract_cstr(&payload[msg_offset..]);
                if !app_name.is_empty() && !message.is_empty()
                {
                    let event_id = u16::from_le_bytes([
                        payload[20],
                        payload[21],
                    ]);
                    let event_type = u16::from_le_bytes([
                        payload[22],
                        payload[23],
                    ]);
                    let severity = match event_type {
                        1 => "DEBUG",
                        2 => "INFO",
                        3 => "ERROR",
                        4 => "CRIT",
                        _ => "???",
                    };
                    println!(
                        "[{severity}] {app_name} ({event_id}): {message}"
                    );
                    continue;
                }
            }
        }

        // Not an EVS event — print raw
        println!(
            "[MID {mid:#06x}] {len} bytes",
        );
    }
}

fn extract_cstr(data: &[u8]) -> &str {
    let end = data
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(data.len());
    core::str::from_utf8(&data[..end]).unwrap_or("")
}
