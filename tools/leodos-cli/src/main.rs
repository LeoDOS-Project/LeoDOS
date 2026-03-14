use anyhow::Result;
use clap::{Parser, Subcommand};

mod build;
mod dashboard;
mod datagen;
mod definitions;
mod deploy;
mod logs;
mod sim;
mod status;
mod tc;
mod tm;

#[derive(Parser)]
#[command(name = "leodos", version, about = "LeoDOS CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build everything (Docker image, simulators, FSW).
    Build,

    /// Manage the simulated constellation.
    Sim {
        #[command(subcommand)]
        action: SimAction,
    },

    /// Generate synthetic sensor data for testing.
    Datagen {
        /// Path to scenario YAML file.
        scenario: String,

        /// Output directory.
        #[arg(short, long, default_value = "tools/eosim/output")]
        output: String,

        /// Output format: bin, tif, or both.
        #[arg(long, default_value = "bin")]
        fmt: String,
    },

    /// Deploy a new app binary to a running satellite.
    Deploy {
        /// Name of the app to deploy.
        app: String,

        /// Target satellite (e.g. "0.1"). Default: all.
        #[arg(long)]
        sat: Option<String>,

        /// Path to the .so file (default: auto-detect).
        #[arg(short, long)]
        file: Option<String>,

        /// CI_LAB command port.
        #[arg(long, default_value_t = 1234)]
        port: u16,
    },

    /// Query constellation status.
    Status {
        /// Target satellite (e.g. "0.1"). Default: all.
        #[arg(long)]
        sat: Option<String>,

        /// CI_LAB command port.
        #[arg(long, default_value_t = 1234)]
        cmd_port: u16,

        /// Telemetry listen port.
        #[arg(long, default_value_t = 1235)]
        tlm_port: u16,
    },

    /// Stream EVS event log.
    Logs {
        /// Target satellite (e.g. "0.1"). Default: all.
        #[arg(long)]
        sat: Option<String>,

        /// Telemetry listen port.
        #[arg(long, default_value_t = 1235)]
        port: u16,
    },

    /// Send a raw cFS telecommand.
    Send {
        /// Target satellite (e.g. "0.1"). Default: "0.0".
        #[arg(long, default_value = "0.0")]
        sat: String,

        /// CI_LAB command port.
        #[arg(long, default_value_t = 1234)]
        port: u16,

        /// Message ID (hex, e.g. "0x1806").
        #[arg(long)]
        mid: String,

        /// Function code.
        #[arg(long)]
        cc: u8,

        /// Wait for telemetry response.
        #[arg(long, default_value_t = false)]
        expect_response: bool,

        /// Payload bytes (hex string).
        #[arg(default_value = "")]
        payload: String,
    },

    /// Listen for raw telemetry packets.
    Listen {
        /// UDP port to listen on.
        #[arg(long, default_value_t = 1235)]
        port: u16,
    },

    /// Interactive dashboard with logs, constellation view,
    /// and satellite status table.
    Dashboard {
        /// Telemetry listen port.
        #[arg(long, default_value_t = 1235)]
        port: u16,

        /// Number of orbital planes (for constellation view).
        #[arg(long, default_value_t = 3)]
        orbits: u8,

        /// Satellites per plane.
        #[arg(long, default_value_t = 3)]
        sats: u8,
    },
}

#[derive(Subcommand)]
enum SimAction {
    /// Start the simulated constellation.
    Start {
        /// Number of orbital planes.
        #[arg(long, default_value_t = 3)]
        orbits: u8,
        /// Number of satellites per plane.
        #[arg(long, default_value_t = 3)]
        sats: u8,
    },
    /// Stop the simulation.
    Stop,
    /// Open a shell in a satellite container.
    Shell {
        /// Target satellite (e.g. "0.1"). Default: "0.0".
        #[arg(long, default_value = "0.0")]
        sat: String,
    },
}

/// Resolve a satellite grid position to its Docker IP.
fn sat_ip(sat: &str) -> Result<String> {
    let parts: Vec<&str> = sat.split('.').collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "Invalid satellite address '{sat}'. \
             Use 'orbit.sat' format (e.g. '0.1')."
        );
    }
    let orb: u8 = parts[0].parse()?;
    let _sat: u8 = parts[1].parse()?;
    Ok(format!("172.20.{orb}.10"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build => build::run().await,
        Commands::Sim { action } => match action {
            SimAction::Start { orbits, sats } => {
                sim::start(orbits, sats).await
            }
            SimAction::Stop => sim::stop().await,
            SimAction::Shell { sat } => {
                sim::shell(&sat).await
            }
        },
        Commands::Datagen {
            scenario,
            output,
            fmt,
        } => datagen::run(&scenario, &output, &fmt).await,
        Commands::Deploy {
            app,
            sat,
            file,
            port,
        } => {
            let host = sat_ip(sat.as_deref().unwrap_or("0.0"))?;
            deploy::run(&app, file.as_deref(), &host, port)
                .await
        }
        Commands::Status {
            sat,
            cmd_port,
            tlm_port,
        } => {
            let host = sat_ip(sat.as_deref().unwrap_or("0.0"))?;
            status::run(&host, cmd_port, tlm_port).await
        }
        Commands::Logs { sat: _, port } => {
            logs::run(port).await
        }
        Commands::Send {
            sat,
            port,
            mid,
            cc,
            expect_response,
            payload,
        } => {
            let host = sat_ip(&sat)?;
            let mid_val = u16::from_str_radix(
                mid.trim_start_matches("0x"),
                16,
            )?;
            tc::send(
                &host,
                port,
                mid_val,
                cc,
                payload.as_bytes(),
                expect_response,
            )
            .await
        }
        Commands::Listen { port } => tm::listen(port).await,
        Commands::Dashboard {
            port,
            orbits,
            sats,
        } => dashboard::run(port, orbits, sats).await,
    }
}
