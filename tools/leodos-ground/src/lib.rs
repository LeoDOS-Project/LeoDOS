//! Ground-station ping logic, factored out so leo-viz can call it
//! in-process from its egui send-window without spawning a subprocess.
//!
//! Public API: [`ping`] — async, runs to completion, returns
//! `Result<PongInfo, String>`.

use leodos_protocols::buffer_pool::HeapBufferPool;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::tokio::SrsppReceiver;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::Reachable;
use leodos_protocols::transport::srspp::api::tokio::SrsppSender;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverMachine;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;
use std::net::SocketAddr;
use std::time::Duration;
use zerocopy::network_endian::U32;
use zerocopy::network_endian::U64;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

pub mod bridge;
pub mod udp_link;

pub use bridge::bridge_loop;
pub use bridge::BridgeConfig;
use udp_link::GroundSocket;

pub const PORT_BASE: u16 = 6000;
pub const PORTS_PER_SAT: u16 = 5;
pub const GROUND_OFFSET: u16 = 4;
pub const GROUND_STATION_ID: u8 = 0;
pub const GROUND_LOCAL_PORT: u16 = 9000;

const PING_APID: u16 = 0x62;

const SRSPP_WIN: usize = 8;
const SRSPP_MTU: usize = 512;
const SRSPP_BUF_SIZE: usize = 4096;
const SRSPP_TICKS_PER_SEC: u32 = 1000;
const SRSPP_MAX_RETRANSMITS: u8 = 60;
const POOL_OVERHEAD: usize = 1024;
const POOL_BYTES: usize = SRSPP_BUF_SIZE + 2 * SRSPP_MTU + POOL_OVERHEAD;

const RX_REASM_BUF: usize = 8192;

#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone)]
struct PingPayload {
    seq: U32,
    sent_ms: U64,
}

#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone)]
struct PongPayload {
    seq: U32,
    scid: U32,
    orb: u8,
    sat: u8,
    _pad: [u8; 2],
    met_seconds: U32,
    met_subseconds: U32,
    sent_ms: U64,
}

/// Decoded pong data plus the wall-clock RTT measured against the ground send.
#[derive(Debug, Clone)]
pub struct PongInfo {
    pub seq: u32,
    pub scid: u32,
    pub orb: u8,
    pub sat: u8,
    pub met_seconds: u32,
    pub met_subseconds: u32,
    pub rtt_ms: u64,
}

pub fn sat_port_base(orb: u8, sat: u8, num_sats: u8) -> u16 {
    PORT_BASE + (orb as u16 * num_sats as u16 + sat as u16) * PORTS_PER_SAT
}

pub fn sat_ground_port(orb: u8, sat: u8, num_sats: u8) -> u16 {
    sat_port_base(orb, sat, num_sats) + GROUND_OFFSET
}

/// Block until `reachable.is_reachable(origin, target)` becomes true.
/// Prints the oracle's predicted wait once at the start. Returns the
/// elapsed wait. Returns [`Duration::ZERO`] immediately if the target
/// is already reachable.
pub async fn wait_for_contact(
    reachable: &impl Reachable,
    origin: Address,
    target: Address,
) -> Duration {
    if reachable.is_reachable(origin, target) {
        return Duration::ZERO;
    }
    match reachable.seconds_until_contact(origin, target) {
        Some(secs) => eprintln!(
            "waiting ~{}s for line-of-sight with {:?}",
            secs, target
        ),
        None => eprintln!("waiting for line-of-sight with {:?} (no estimate)", target),
    }
    let start = std::time::Instant::now();
    while !reachable.is_reachable(origin, target) {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    start.elapsed()
}

fn epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub async fn ping(
    orb: u8,
    sat: u8,
    num_sats: u8,
    rto_ms: u32,
    timeout_s: u64,
) -> Result<PongInfo, String> {
    ping_with_reachable(orb, sat, num_sats, rto_ms, timeout_s, &AlwaysReachable).await
}

/// Same as [`ping`] but takes a custom reachability oracle. If no
/// satellite is currently reachable, blocks until one becomes
/// reachable, printing the expected wait. Always routes directly to
/// the target's ground port — see [`ping_via_gateway`] for LOS-aware
/// first-hop selection.
pub async fn ping_with_reachable(
    orb: u8,
    sat: u8,
    num_sats: u8,
    rto_ms: u32,
    timeout_s: u64,
    reachable: &impl Reachable,
) -> Result<PongInfo, String> {
    let target = Address::Satellite(leodos_protocols::network::isl::torus::Point::new(orb, sat));
    let source = Address::Ground { station: GROUND_STATION_ID };

    let waited = wait_for_contact(reachable, source, target).await;
    if waited > Duration::ZERO {
        eprintln!(
            "waited {:.1}s for line-of-sight (target sat({},{}))",
            waited.as_secs_f32(),
            orb,
            sat,
        );
    }

    ping_via_gateway(orb, sat, num_sats, rto_ms, timeout_s, orb, sat).await
}

/// Send a ping to `(target_orb, target_sat)` via a specific first-hop
/// gateway at `(gateway_orb, gateway_sat)`. The UDP datagram is
/// addressed to the gateway's ground-facing port, but the SRSPP
/// target carried inside is the actual destination — the gateway's
/// on-orbit router forwards via ISL if the target is elsewhere.
///
/// Set `gateway_orb == target_orb && gateway_sat == target_sat` for
/// the direct-routing case (the gateway is the target itself).
pub async fn ping_via_gateway(
    target_orb: u8,
    target_sat: u8,
    num_sats: u8,
    rto_ms: u32,
    timeout_s: u64,
    gateway_orb: u8,
    gateway_sat: u8,
) -> Result<PongInfo, String> {
    let target =
        Address::Satellite(leodos_protocols::network::isl::torus::Point::new(target_orb, target_sat));
    let source = Address::Ground { station: GROUND_STATION_ID };

    let local: SocketAddr = format!("127.0.0.1:{}", GROUND_LOCAL_PORT)
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    let remote: SocketAddr =
        format!("127.0.0.1:{}", sat_ground_port(gateway_orb, gateway_sat, num_sats))
            .parse()
            .map_err(|e: std::net::AddrParseError| e.to_string())?;

    let apid = Apid::new(PING_APID).map_err(|e| format!("bad APID: {e:?}"))?;

    let socket = GroundSocket::bind(local, remote)
        .await
        .map_err(|e| format!("bind: {e}"))?;
    let (sender_link, receiver_link, dispatcher) = socket.split();
    let dispatcher_handle = tokio::spawn(dispatcher);
    // RAII guard: abort the dispatcher (and drop the UDP socket) on
    // every return path so the next call can rebind 127.0.0.1:9000.
    // Without this, the dispatcher stays parked on recv_from holding
    // an Arc<UdpSocket>, so the kernel can deliver packets to a stale
    // socket whose channels are already closed.
    struct DispatcherGuard(Option<tokio::task::JoinHandle<()>>);
    impl Drop for DispatcherGuard {
        fn drop(&mut self) {
            if let Some(h) = self.0.take() {
                h.abort();
            }
        }
    }
    let _dispatcher_guard = DispatcherGuard(Some(dispatcher_handle));

    let sender_config = SenderConfig::builder()
        .source_address(source)
        .apid(apid)
        .function_code(0)
        .rto_ticks(rto_ms)
        .max_retransmits(SRSPP_MAX_RETRANSMITS)
        .header_overhead(SrsppDataPacket::HEADER_SIZE)
        .build();
    let pool = HeapBufferPool::new(POOL_BYTES);
    let mut sender: SrsppSender<_, _, _, SRSPP_WIN, SRSPP_MTU> = SrsppSender::new(
        sender_config,
        sender_link,
        FixedRto::new(rto_ms),
        SRSPP_TICKS_PER_SEC,
        &pool,
        SRSPP_BUF_SIZE,
    )
    .map_err(|_| "sender pool alloc failed".to_string())?;

    let receiver_config = ReceiverConfig::builder()
        .local_address(source)
        .apid(apid)
        .function_code(0)
        .immediate_ack(true)
        .ack_delay_ticks(100)
        .build();
    let mut receiver: SrsppReceiver<
        _,
        ReceiverMachine<SRSPP_WIN, SRSPP_BUF_SIZE, RX_REASM_BUF>,
        SRSPP_MTU,
    > = SrsppReceiver::new(receiver_config, target, receiver_link, SRSPP_TICKS_PER_SEC);

    let seq_u32: u32 = 1;
    let send_ms = epoch_millis();
    let ping_msg = PingPayload {
        seq: U32::new(seq_u32),
        sent_ms: U64::new(send_ms),
    };

    sender
        .send(target, ping_msg.as_bytes())
        .await
        .map_err(|e| format!("send: {e}"))?;
    sender
        .send_eos(target)
        .await
        .map_err(|e| format!("send_eos: {e}"))?;

    let result: Result<PongPayload, String> = tokio::time::timeout(
        Duration::from_secs(timeout_s),
        async {
            let flush_task = async {
                sender.flush().await.map_err(|e| format!("flush: {e}"))
            };
            // Drive the receiver until both the pong (Ok(Some(n)))
            // and the sat's end-of-stream (Ok(None)) have been
            // observed. Looping past the pong is what keeps the
            // protocol's close handshake alive: the satellite's EOS
            // gets ACKed as a side effect of the second recv, so
            // sat's flush finishes promptly and the next ping isn't
            // delayed by retransmits.
            let recv_task = async {
                let mut buf = [0u8; 128];
                let mut pong: Option<PongPayload> = None;
                loop {
                    match receiver
                        .recv(&mut buf)
                        .await
                        .map_err(|e| format!("recv: {e}"))?
                    {
                        Some(len) => {
                            pong = Some(
                                PongPayload::read_from_bytes(&buf[..len])
                                    .map_err(|_| "bad pong payload".to_string())?,
                            );
                        }
                        None => break,
                    }
                }
                pong.ok_or_else(|| "stream closed before pong arrived".to_string())
            };
            let pong = {
                tokio::pin!(flush_task, recv_task);
                let mut pong: Option<PongPayload> = None;
                let mut flushed = false;
                while pong.is_none() || !flushed {
                    tokio::select! {
                        r = &mut flush_task, if !flushed => {
                            r?;
                            flushed = true;
                        }
                        r = &mut recv_task, if pong.is_none() => {
                            pong = Some(r?);
                        }
                    }
                }
                pong.unwrap()
            };
            Ok(pong)
        },
    )
    .await
    .unwrap_or_else(|_| Err(format!("timed out after {timeout_s}s")));

    let pong = result?;
    Ok(PongInfo {
        seq: pong.seq.get(),
        scid: pong.scid.get(),
        orb: pong.orb,
        sat: pong.sat,
        met_seconds: pong.met_seconds.get(),
        met_subseconds: pong.met_subseconds.get(),
        rtt_ms: epoch_millis().saturating_sub(send_ms),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    /// Reachability oracle whose `is_reachable` reads a shared atomic;
    /// `seconds_until_contact` returns a configurable estimate.
    struct ToggleOracle {
        reachable: Arc<AtomicBool>,
        eta_secs: u32,
    }

    impl Reachable for ToggleOracle {
        fn is_reachable(&self, _origin: Address, _target: Address) -> bool {
            self.reachable.load(Ordering::SeqCst)
        }
        fn seconds_until_contact(
            &self,
            _origin: Address,
            _target: Address,
        ) -> Option<u32> {
            if self.is_reachable(_origin, _target) {
                None
            } else {
                Some(self.eta_secs)
            }
        }
    }

    fn origin() -> Address {
        Address::Ground { station: 0 }
    }

    fn target() -> Address {
        Address::Satellite(leodos_protocols::network::isl::torus::Point::new(0, 0))
    }

    #[tokio::test]
    async fn wait_for_contact_returns_zero_when_reachable() {
        let oracle = ToggleOracle {
            reachable: Arc::new(AtomicBool::new(true)),
            eta_secs: 0,
        };
        let waited = wait_for_contact(&oracle, origin(), target()).await;
        assert_eq!(waited, Duration::ZERO);
    }

    #[tokio::test]
    async fn wait_for_contact_blocks_until_reachable_flips() {
        let reachable = Arc::new(AtomicBool::new(false));
        let oracle = ToggleOracle {
            reachable: reachable.clone(),
            eta_secs: 2,
        };
        let flip_after = Duration::from_millis(400);
        let flipper = {
            let r = reachable.clone();
            tokio::spawn(async move {
                tokio::time::sleep(flip_after).await;
                r.store(true, Ordering::SeqCst);
            })
        };
        let start = Instant::now();
        let waited = wait_for_contact(&oracle, origin(), target()).await;
        flipper.await.unwrap();
        assert!(waited >= flip_after, "must have waited at least the flip delay");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "must not have busy-waited past the flip event"
        );
    }
}
