use anyhow::{Context, Result};
use leodos_protocols::network::cfe::tc::Telecommand;
use leodos_protocols::network::cfe::tm::Telemetry;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::network::spp::SequenceCount;
use tokio::net::UdpSocket;
use tokio::time::{Duration, timeout};
use zerocopy::IntoBytes;

const FS_SRV_APID: u16 = 0x71;
const CI_PORT: u16 = 1234;
const TLM_PORT: u16 = 1235;

const OP_LIST: u8 = 0;
const OP_PUT: u8 = 1;
const OP_GET: u8 = 2;
const OP_REMOVE: u8 = 3;
const OP_RENAME: u8 = 4;
const OP_INFO: u8 = 5;

const CHUNK_SIZE: usize = 400;

fn build_request(
    op: u8,
    path: &str,
    dest: &str,
    data_offset: u32,
    data: &[u8],
) -> Vec<u8> {
    let path_bytes = path.as_bytes();
    let dest_bytes = dest.as_bytes();
    let mut buf = Vec::new();

    // FsRequest header: op(1) + pad(1) + path_len(2) +
    // dest_len(2) + data_offset(4) + data_len(4) = 14 bytes
    buf.push(op);
    buf.push(0); // pad
    buf.extend_from_slice(
        &(path_bytes.len() as u16).to_le_bytes(),
    );
    buf.extend_from_slice(
        &(dest_bytes.len() as u16).to_le_bytes(),
    );
    buf.extend_from_slice(&data_offset.to_le_bytes());
    buf.extend_from_slice(
        &(data.len() as u32).to_le_bytes(),
    );

    // Followed by path, dest, data
    buf.extend_from_slice(path_bytes);
    buf.extend_from_slice(dest_bytes);
    buf.extend_from_slice(data);
    buf
}

async fn send_request(
    host: &str,
    port: u16,
    payload: &[u8],
) -> Result<()> {
    crate::tc::send(host, port, FS_SRV_APID, 0, payload, false)
        .await
}

async fn recv_response(
    sock: &UdpSocket,
    timeout_s: u64,
) -> Result<Option<Vec<u8>>> {
    let mut buf = [0u8; 2048];
    match timeout(
        Duration::from_secs(timeout_s),
        sock.recv_from(&mut buf),
    )
    .await
    {
        Ok(Ok((len, _))) => Ok(Some(buf[..len].to_vec())),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Ok(None),
    }
}

fn parse_response_payload(data: &[u8]) -> Option<(u8, u8, &[u8])> {
    // FsResponse: op(1) + status(1) + pad(2) + payload_len(4) = 8
    if data.len() < 8 {
        return None;
    }
    let op = data[0];
    let status = data[1];
    let payload_len =
        u32::from_le_bytes([data[4], data[5], data[6], data[7]])
            as usize;
    let payload = &data[8..8 + payload_len.min(data.len() - 8)];
    Some((op, status, payload))
}

pub async fn ls(host: &str, path: &str) -> Result<()> {
    let req = build_request(OP_LIST, path, "", 0, &[]);
    send_request(host, CI_PORT, &req).await?;

    let sock =
        UdpSocket::bind(format!("0.0.0.0:{TLM_PORT}")).await?;

    match recv_response(&sock, 5).await? {
        Some(data) => {
            if let Some((_op, status, payload)) =
                parse_response_payload(&data)
            {
                if status == 0 {
                    let text =
                        core::str::from_utf8(payload)
                            .unwrap_or("<binary>");
                    print!("{text}");
                } else {
                    eprintln!("Error (status={status})");
                }
            }
        }
        None => eprintln!("No response (timeout)."),
    }
    Ok(())
}

pub async fn put(
    host: &str,
    local_path: &str,
    remote_path: &str,
) -> Result<()> {
    let data = std::fs::read(local_path)
        .with_context(|| format!("Failed to read {local_path}"))?;

    let total = data.len();
    let mut offset: usize = 0;

    while offset < total {
        let end = (offset + CHUNK_SIZE).min(total);
        let chunk = &data[offset..end];
        let req = build_request(
            OP_PUT,
            remote_path,
            "",
            offset as u32,
            chunk,
        );
        send_request(host, CI_PORT, &req).await?;
        offset = end;
        print!(
            "\rUploading... {}/{}",
            offset, total
        );
    }

    // Send empty chunk to signal completion
    let req =
        build_request(OP_PUT, remote_path, "", offset as u32, &[]);
    send_request(host, CI_PORT, &req).await?;
    println!("\rUploaded {total} bytes to {remote_path}");
    Ok(())
}

pub async fn get(
    host: &str,
    remote_path: &str,
    local_path: &str,
) -> Result<()> {
    let req = build_request(OP_GET, remote_path, "", 0, &[]);
    send_request(host, CI_PORT, &req).await?;

    let sock =
        UdpSocket::bind(format!("0.0.0.0:{TLM_PORT}")).await?;

    let mut file_data = Vec::new();

    loop {
        match recv_response(&sock, 5).await? {
            Some(data) => {
                if let Some((_op, status, payload)) =
                    parse_response_payload(&data)
                {
                    if status != 0 {
                        eprintln!("Error (status={status})");
                        return Ok(());
                    }
                    if payload.is_empty() {
                        break;
                    }
                    file_data.extend_from_slice(payload);
                    print!(
                        "\rDownloading... {} bytes",
                        file_data.len()
                    );
                }
            }
            None => {
                break;
            }
        }
    }

    std::fs::write(local_path, &file_data)
        .with_context(|| {
            format!("Failed to write {local_path}")
        })?;
    println!(
        "\rDownloaded {} bytes to {local_path}",
        file_data.len()
    );
    Ok(())
}

pub async fn rm(host: &str, path: &str) -> Result<()> {
    let req = build_request(OP_REMOVE, path, "", 0, &[]);
    send_request(host, CI_PORT, &req).await?;
    println!("Removed {path}");
    Ok(())
}

pub async fn mv(
    host: &str,
    src: &str,
    dest: &str,
) -> Result<()> {
    let req = build_request(OP_RENAME, src, dest, 0, &[]);
    send_request(host, CI_PORT, &req).await?;
    println!("Renamed {src} → {dest}");
    Ok(())
}

pub async fn info(host: &str, path: &str) -> Result<()> {
    let req = build_request(OP_INFO, path, "", 0, &[]);
    send_request(host, CI_PORT, &req).await?;

    let sock =
        UdpSocket::bind(format!("0.0.0.0:{TLM_PORT}")).await?;

    match recv_response(&sock, 5).await? {
        Some(data) => {
            if let Some((_op, status, payload)) =
                parse_response_payload(&data)
            {
                if status == 0 {
                    let text =
                        core::str::from_utf8(payload)
                            .unwrap_or("<binary>");
                    println!("{text}");
                } else {
                    eprintln!("Error (status={status})");
                }
            }
        }
        None => eprintln!("No response (timeout)."),
    }
    Ok(())
}
