use clap::Parser;
use clap::Subcommand;

use leodos_ground::bridge_loop;
use leodos_ground::ping;
use leodos_ground::ping_via_gateway;
use leodos_ground::BridgeConfig;

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
        /// First-hop gateway orbit. Defaults to target orb.
        #[arg(long)]
        gw_orb: Option<u8>,
        /// First-hop gateway sat. Defaults to target sat.
        #[arg(long)]
        gw_sat: Option<u8>,
        /// Overall timeout in seconds.
        #[arg(long, default_value_t = 10)]
        timeout: u64,
    },
    /// Run as a long-lived bridge daemon: receive PingRequestFrames
    /// from leo-viz, run pings, ship results back as EventFrames.
    Bridge {
        /// host:port of the leo-viz bridge server.
        #[arg(long)]
        bridge_addr: String,
        /// Ground station id (matches `Hello.scid` for ground endpoints).
        #[arg(long, default_value_t = 0)]
        station_id: u8,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    match args.command {
        Command::Ping { orb, sat, gw_orb, gw_sat, timeout } => {
            let go = gw_orb.unwrap_or(orb);
            let gs = gw_sat.unwrap_or(sat);
            let res = if (go, gs) == (orb, sat) {
                ping(orb, sat, args.num_sats, args.rto_ms, timeout).await
            } else {
                ping_via_gateway(
                    orb, sat, args.num_sats, args.rto_ms, timeout, go, gs,
                )
                .await
            };
            match res {
                Ok(pong) => {
                    println!(
                        "pong: sat({}, {}) scid={} seq={} met={}.{} rtt_ms={}",
                        pong.orb,
                        pong.sat,
                        pong.scid,
                        pong.seq,
                        pong.met_seconds,
                        pong.met_subseconds,
                        pong.rtt_ms,
                    );
                    Ok(())
                }
                Err(e) => Err(e.into()),
            }
        }
        Command::Bridge { bridge_addr, station_id } => {
            let cfg = BridgeConfig {
                bridge_addr,
                station_id,
                num_sats_per_plane: args.num_sats,
            };
            bridge_loop(cfg).await?;
            Ok(())
        }
    }
}
