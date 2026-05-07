//! Bridge daemon: long-running ground-station endpoint that turns
//! [`PingRequestFrame`]s arriving from the leo-viz bridge server into
//! actual SRSPP pings via [`crate::ping`], and ships each result back
//! as an [`EventFrame`] tagged with `app_name="GROUND"`.
//!
//! The daemon also receives [`GroundStateFrame`]s — currently only
//! logged. Once outbound UDP is LOS-gated (TODO in CLAUDE.md), the
//! daemon will use `visible[0]` as its first-hop gateway.

use crate::ping;
use leodos_bridge::DecodeError;
use leodos_bridge::EventFrame;
use leodos_bridge::GroundStateFrame;
use leodos_bridge::Hello;
use leodos_bridge::KIND_GROUND_STATE;
use leodos_bridge::KIND_PING_REQUEST;
use leodos_bridge::PingRequestFrame;
use std::io;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;

const RECONNECT_BACKOFF: Duration = Duration::from_secs(1);

/// Configuration for the bridge daemon.
pub struct BridgeConfig {
    pub bridge_addr: String,
    pub station_id: u8,
    pub num_sats_per_plane: u8,
}

/// Run the daemon forever. Reconnects with backoff on disconnect.
pub async fn bridge_loop(cfg: BridgeConfig) -> io::Result<()> {
    loop {
        match TcpStream::connect(&cfg.bridge_addr).await {
            Ok(stream) => {
                eprintln!(
                    "leodos-ground bridge: connected station_id={} to {}",
                    cfg.station_id, cfg.bridge_addr
                );
                if let Err(e) = run_session(stream, &cfg).await {
                    eprintln!("leodos-ground bridge: session ended: {}", e);
                }
            }
            Err(e) => {
                eprintln!(
                    "leodos-ground bridge: connect to {} failed: {}",
                    cfg.bridge_addr, e
                );
            }
        }
        tokio::time::sleep(RECONNECT_BACKOFF).await;
    }
}

async fn run_session(stream: TcpStream, cfg: &BridgeConfig) -> io::Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    {
        let hello = Hello::ground(cfg.station_id as u32);
        writer.write_all(hello.as_bytes()).await?;
    }

    let mut event_seq: u32 = 0;
    // Pings are run inline (not via tokio::spawn) because each ping
    // binds the fixed router-reply UDP port (9000). Concurrent binds
    // with SO_REUSEADDR make the kernel demux unpredictable, so we
    // serialize: at most one ping in flight per daemon. Subsequent
    // PingRequestFrames queue in the TCP receive buffer.
    loop {
        let mut kind_buf = [0u8; 1];
        reader.read_exact(&mut kind_buf).await?;
        match kind_buf[0] {
            KIND_PING_REQUEST => {
                let mut buf = [0u8; core::mem::size_of::<PingRequestFrame>()];
                reader.read_exact(&mut buf).await?;
                let frame = PingRequestFrame::read_from_bytes(&buf)
                    .map_err(|_| invalid("PingRequestFrame decode"))?;
                frame.validate().map_err(decode_to_io)?;
                let n = if frame.num_sats_per_plane == 0 {
                    cfg.num_sats_per_plane
                } else {
                    frame.num_sats_per_plane
                };
                let timeout_s = (frame.timeout_ms.get().max(1000) / 1000) as u64;
                let request_id = frame.request_id.get();
                let res = ping(
                    frame.target_orb,
                    frame.target_sat,
                    n,
                    frame.rto_ms.get(),
                    timeout_s,
                )
                .await;
                let mut msg = heapless_msg();
                match &res {
                    Ok(p) => {
                        let _ = std::fmt::Write::write_fmt(
                            &mut msg,
                            format_args!(
                                "req={} pong sat({},{}) rtt_ms={}",
                                request_id, p.orb, p.sat, p.rtt_ms
                            ),
                        );
                    }
                    Err(e) => {
                        let _ = std::fmt::Write::write_fmt(
                            &mut msg,
                            format_args!("req={} err: {}", request_id, e),
                        );
                    }
                }
                let event = EventFrame::new(
                    event_seq,
                    epoch_millis(),
                    cfg.station_id as u32,
                    /*event_id*/ 1,
                    /*event_type INFO*/ 2,
                    b"GROUND",
                    msg.as_bytes(),
                );
                event_seq = event_seq.wrapping_add(1);
                writer.write_all(event.as_bytes()).await?;
            }
            KIND_GROUND_STATE => {
                let mut buf = [0u8; core::mem::size_of::<GroundStateFrame>()];
                reader.read_exact(&mut buf).await?;
                let frame = GroundStateFrame::read_from_bytes(&buf)
                    .map_err(|_| invalid("GroundStateFrame decode"))?;
                frame.validate().map_err(decode_to_io)?;
                // TODO: stash latest visible[] for LOS-aware gateway selection.
            }
            other => {
                return Err(invalid_kind(other));
            }
        }
    }
}

fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn heapless_msg() -> String {
    String::with_capacity(96)
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

fn invalid_kind(kind: u8) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("unknown frame kind {}", kind),
    )
}

fn decode_to_io(e: DecodeError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{:?}", e))
}
