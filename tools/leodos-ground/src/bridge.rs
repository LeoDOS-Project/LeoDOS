//! Bridge daemon: long-running ground-station endpoint that turns
//! [`PingRequestFrame`]s arriving from the leo-viz bridge server into
//! actual SRSPP pings via [`crate::ping`], and ships each result back
//! as an [`EventFrame`] tagged with `app_name="GROUND"`.
//!
//! The daemon also receives [`GroundStateFrame`]s — currently only
//! logged. Once outbound UDP is LOS-gated (TODO in CLAUDE.md), the
//! daemon will use `visible[0]` as its first-hop gateway.

use crate::ping_via_gateway;
use crate::wait_for_contact;
use crate::GROUND_STATION_ID;
use leodos_bridge::DecodeError;
use leodos_bridge::EventFrame;
use leodos_bridge::GroundStateFrame;
use leodos_bridge::Hello;
use leodos_bridge::KIND_GROUND_STATE;
use leodos_bridge::KIND_PING_REQUEST;
use leodos_bridge::PingRequestFrame;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::transport::srspp::dtn::Reachable;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;

/// LOS oracle backed by the latest `GroundStateFrame` received over
/// the bridge.
///
/// Reachability semantics: a target sat is reachable iff *any* sat
/// is currently visible from this ground station — that sat acts as
/// the first-hop gateway and the on-orbit router forwards via ISL
/// to the actual target. The target itself doesn't have to be in
/// LOS.
///
/// `gateway()` returns the visible sat to use as the gateway, in
/// elevation order (highest first, as ordered by leo-viz).
#[derive(Default, Clone)]
pub struct BridgeLos {
    state: Arc<Mutex<LosState>>,
}

#[derive(Default)]
struct LosState {
    visible: Vec<(u8, u8)>,
    /// Seconds until the next predicted AOS, computed by leo-viz
    /// from the orbital propagator. `u32::MAX` if no AOS is
    /// predicted within the search horizon.
    next_aos_secs: u32,
}

impl BridgeLos {
    /// Replace the visible list and AOS estimate with the contents
    /// of a fresh `GroundStateFrame`.
    pub fn update(&self, frame: &GroundStateFrame) {
        let mut s = self.state.lock().expect("BridgeLos lock");
        s.visible.clear();
        for v in frame.visible_slice() {
            s.visible.push((v.orb, v.sat));
        }
        s.next_aos_secs = frame.next_aos_secs.get();
    }

    /// Current first-hop gateway: the visible sat with the highest
    /// elevation, or `None` if no sat is visible.
    pub fn gateway(&self) -> Option<(u8, u8)> {
        self.state.lock().expect("BridgeLos lock").visible.first().copied()
    }
}

impl Reachable for BridgeLos {
    fn is_reachable(&self, _origin: Address, target: Address) -> bool {
        let Address::Satellite(_) = target else {
            return true;
        };
        !self.state.lock().expect("BridgeLos lock").visible.is_empty()
    }

    fn seconds_until_contact(&self, _origin: Address, target: Address) -> Option<u32> {
        let Address::Satellite(_) = target else {
            return None;
        };
        let s = self.state.lock().expect("BridgeLos lock");
        if !s.visible.is_empty() {
            return None;
        }
        match s.next_aos_secs {
            u32::MAX => None,
            n => Some(n),
        }
    }
}

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
    let (mut reader, writer) = stream.into_split();
    let writer = Arc::new(tokio::sync::Mutex::new(writer));

    {
        let mut w = writer.lock().await;
        let hello = Hello::ground(cfg.station_id as u32);
        w.write_all(hello.as_bytes()).await?;
    }

    let los = BridgeLos::default();
    // Pings are run inline by a single processor task (not via
    // tokio::spawn per ping) because each ping binds the fixed
    // router-reply UDP port (9000). Concurrent binds with
    // SO_REUSEADDR make the kernel demux unpredictable, so we
    // serialize: at most one ping in flight per daemon. The reader
    // task feeds requests into a bounded channel; ground-state
    // frames update the LOS oracle inline so a ping that's parked
    // waiting for LOS sees fresh visibility data within one tick.
    let (ping_tx, mut ping_rx) =
        tokio::sync::mpsc::channel::<PingRequestFrame>(16);

    let station_id = cfg.station_id as u32;
    let num_sats_per_plane = cfg.num_sats_per_plane;
    let los_for_processor = los.clone();
    let writer_for_processor = writer.clone();
    let processor = tokio::spawn(async move {
        let mut event_seq: u32 = 0;
        while let Some(frame) = ping_rx.recv().await {
            let n = if frame.num_sats_per_plane == 0 {
                num_sats_per_plane
            } else {
                frame.num_sats_per_plane
            };
            let timeout_s = (frame.timeout_ms.get().max(1000) / 1000) as u64;
            let request_id = frame.request_id.get();
            let target = Address::Satellite(
                leodos_protocols::network::isl::torus::Point::new(
                    frame.target_orb,
                    frame.target_sat,
                ),
            );
            let source = Address::Ground { station: GROUND_STATION_ID };

            // If we don't have LOS, tell the UI we're parked waiting.
            if !los_for_processor.is_reachable(source, target) {
                let mut msg = heapless_msg();
                match los_for_processor.seconds_until_contact(source, target) {
                    Some(secs) => {
                        let _ = std::fmt::Write::write_fmt(
                            &mut msg,
                            format_args!(
                                "req={} waiting for line-of-sight (eta ~{}s)",
                                request_id, secs
                            ),
                        );
                    }
                    None => {
                        let _ = std::fmt::Write::write_fmt(
                            &mut msg,
                            format_args!(
                                "req={} waiting for line-of-sight (no estimate)",
                                request_id
                            ),
                        );
                    }
                }
                if !emit_event(&writer_for_processor, &mut event_seq, station_id, &msg).await {
                    break;
                }
            }

            let waited = wait_for_contact(&los_for_processor, source, target).await;
            let (gw_orb, gw_sat) = los_for_processor
                .gateway()
                .unwrap_or((frame.target_orb, frame.target_sat));

            // Report contact + chosen gateway once we have LOS.
            if waited > std::time::Duration::ZERO {
                let mut msg = heapless_msg();
                let _ = std::fmt::Write::write_fmt(
                    &mut msg,
                    format_args!(
                        "req={} got line-of-sight via sat({},{}) after {:.1}s",
                        request_id, gw_orb, gw_sat, waited.as_secs_f32()
                    ),
                );
                if !emit_event(&writer_for_processor, &mut event_seq, station_id, &msg).await {
                    break;
                }
            } else {
                let mut msg = heapless_msg();
                let _ = std::fmt::Write::write_fmt(
                    &mut msg,
                    format_args!(
                        "req={} routing via gateway sat({},{})",
                        request_id, gw_orb, gw_sat
                    ),
                );
                if !emit_event(&writer_for_processor, &mut event_seq, station_id, &msg).await {
                    break;
                }
            }

            let res = ping_via_gateway(
                frame.target_orb,
                frame.target_sat,
                n,
                frame.rto_ms.get(),
                timeout_s,
                gw_orb,
                gw_sat,
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
            if !emit_event(&writer_for_processor, &mut event_seq, station_id, &msg).await {
                break;
            }
        }
    });

    let read_loop_result: io::Result<()> = async {
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
                    if ping_tx.send(frame).await.is_err() {
                        return Err(io::Error::other("ping processor stopped"));
                    }
                }
                KIND_GROUND_STATE => {
                    let mut buf = [0u8; core::mem::size_of::<GroundStateFrame>()];
                    reader.read_exact(&mut buf).await?;
                    let frame = GroundStateFrame::read_from_bytes(&buf)
                        .map_err(|_| invalid("GroundStateFrame decode"))?;
                    frame.validate().map_err(decode_to_io)?;
                    los.update(&frame);
                }
                other => return Err(invalid_kind(other)),
            }
        }
    }
    .await;

    drop(ping_tx);
    let _ = processor.await;
    read_loop_result
}

/// Send a `String` payload as a `GROUND` EventFrame. Returns `false`
/// if the write failed (caller should stop the processor loop).
async fn emit_event(
    writer: &Arc<tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    event_seq: &mut u32,
    station_id: u32,
    msg: &str,
) -> bool {
    let event = EventFrame::new(
        *event_seq,
        epoch_millis(),
        station_id,
        1,
        2,
        b"GROUND",
        msg.as_bytes(),
    );
    *event_seq = event_seq.wrapping_add(1);
    let mut w = writer.lock().await;
    w.write_all(event.as_bytes()).await.is_ok()
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
