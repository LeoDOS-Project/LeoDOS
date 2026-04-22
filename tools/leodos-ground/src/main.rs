use clap::Parser;
use clap::Subcommand;
use std::net::SocketAddr;
use std::time::Duration;

use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::tokio::SrsppReceiver;
use leodos_protocols::transport::srspp::api::tokio::SrsppSender;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverMachine;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

use zerocopy::network_endian::U32;
use zerocopy::network_endian::U64;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

mod udp_link;

use udp_link::GroundSocket;

const PORT_BASE: u16 = 6000;
const PORTS_PER_SAT: u16 = 5;
const GROUND_OFFSET: u16 = 4;

/// Ground station address — must match the satellites' expected source.
const GROUND_STATION_ID: u8 = 0;

/// Local UDP port the ground binds (all sats write here).
const GROUND_LOCAL_PORT: u16 = 9000;

const PING_APID: u16 = 0x62;

/// Ping request: identifies the message and records the ground send time.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone)]
struct PingPayload {
    seq: U32,
    sent_ms: U64,
}

/// Pong reply: echoes the ping seq and reports the responding satellite.
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

#[derive(Parser)]
#[command(about = "Ground station for the LeoDOS ping demo")]
struct Args {
    /// Number of sats per orbit (must match the constellation).
    #[arg(long, default_value_t = 3)]
    num_sats: u8,

    /// RTO in milliseconds.
    #[arg(long, default_value_t = 1000)]
    rto_ms: u32,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Send a single ping to sat(orb, sat) and wait for the reply.
    Ping {
        /// Target orbit.
        #[arg(long, default_value_t = 0)]
        orb: u8,
        /// Target sat.
        #[arg(long, default_value_t = 0)]
        sat: u8,
        /// Overall timeout in seconds.
        #[arg(long, default_value_t = 10)]
        timeout: u64,
    },
}

fn sat_port_base(orb: u8, sat: u8, num_sats: u8) -> u16 {
    PORT_BASE + (orb as u16 * num_sats as u16 + sat as u16) * PORTS_PER_SAT
}

fn sat_ground_port(orb: u8, sat: u8, num_sats: u8) -> u16 {
    sat_port_base(orb, sat, num_sats) + GROUND_OFFSET
}

fn epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

async fn ping(
    orb: u8,
    sat: u8,
    num_sats: u8,
    rto_ms: u32,
    timeout_s: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = Address::Satellite(leodos_protocols::network::isl::torus::Point::new(orb, sat));
    let source = Address::Ground { station: GROUND_STATION_ID };

    let local: SocketAddr = format!("127.0.0.1:{}", GROUND_LOCAL_PORT).parse()?;
    let remote: SocketAddr = format!("127.0.0.1:{}", sat_ground_port(orb, sat, num_sats)).parse()?;
    println!("ground: local={} -> sat{},{} ({})", local, orb, sat, remote);

    let apid = Apid::new(PING_APID).unwrap();

    // Shared socket with a dispatcher that routes ACK vs DATA.
    let socket = GroundSocket::bind(local, remote).await?;
    let (sender_link, receiver_link, dispatcher) = socket.split();
    tokio::spawn(dispatcher);

    let sender_config = SenderConfig::builder()
        .source_address(source)
        .apid(apid)
        .function_code(0)
        .rto_ticks(rto_ms)
        .max_retransmits(5)
        .header_overhead(SrsppDataPacket::HEADER_SIZE)
        .build();
    let mut sender: SrsppSender<_, _, 8, 4096, 512> =
        SrsppSender::new(sender_config, sender_link, FixedRto::new(rto_ms), 1000);

    let receiver_config = ReceiverConfig::builder()
        .local_address(source)
        .apid(apid)
        .function_code(0)
        .immediate_ack(true)
        .ack_delay_ticks(100)
        .build();
    let mut receiver: SrsppReceiver<_, ReceiverMachine<8, 4096, 8192>, 512> =
        SrsppReceiver::new(receiver_config, target, receiver_link, 1000);

    // Build ping payload
    let seq_u32: u32 = 1;
    let ping_msg = PingPayload {
        seq: U32::new(seq_u32),
        sent_ms: U64::new(epoch_millis()),
    };
    println!("sending ping seq={} to {:?}", seq_u32, target);

    sender
        .send(target, ping_msg.as_bytes())
        .await
        .map_err(|e| format!("send: {e}"))?;

    // Run sender.flush and receiver.recv concurrently. The
    // dispatcher routes ACKs to the sender and DATA to the
    // receiver, so they no longer steal each other's packets.
    let result: Result<PongPayload, String> = tokio::time::timeout(
        Duration::from_secs(timeout_s),
        async {
            let flush_task = async {
                sender
                    .flush()
                    .await
                    .map_err(|e| format!("flush: {e}"))
            };
            let recv_task = async {
                let mut buf = [0u8; 128];
                let len = receiver
                    .recv(&mut buf)
                    .await
                    .map_err(|e| format!("recv: {e}"))?;
                PongPayload::read_from_bytes(&buf[..len])
                    .map_err(|_| "bad pong payload".to_string())
            };
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
            Ok(pong.unwrap())
        },
    )
    .await
    .unwrap_or_else(|_| Err(format!("timed out after {timeout_s}s")));

    match result {
        Ok(pong) => {
            println!(
                "pong: sat({}, {}) scid={} seq={} met={}.{} rtt_ms={}",
                pong.orb,
                pong.sat,
                pong.scid.get(),
                pong.seq.get(),
                pong.met_seconds.get(),
                pong.met_subseconds.get(),
                epoch_millis().saturating_sub(pong.sent_ms.get()),
            );
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    match args.command {
        Command::Ping { orb, sat, timeout } => {
            ping(orb, sat, args.num_sats, args.rto_ms, timeout).await
        }
    }
}
