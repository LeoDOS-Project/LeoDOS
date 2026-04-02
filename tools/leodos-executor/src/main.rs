use clap::Parser;
use clap::Subcommand;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::geo::GeoAoi;
use leodos_protocols::network::isl::geo::LatLon;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::tokio::SrsppSender;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;
use leodos_spacecomp::job::Job;
use leodos_spacecomp::packet::OpCode;
use leodos_spacecomp::packet::SpaceCompHeader;
use zerocopy::IntoBytes;

mod udp_link;

#[derive(Parser)]
struct Args {
    /// ci_lab UDP address (command uplink to constellation).
    #[arg(long, default_value = "127.0.0.1:5012")]
    ci_lab: String,

    /// Local UDP bind address for receiving telemetry.
    #[arg(long, default_value = "0.0.0.0:5013")]
    local_addr: String,

    /// APID for SpaceCoMP messages.
    #[arg(long, default_value_t = 0x61)]
    apid: u16,

    /// Router send topic (MsgId topic for SB → router).
    #[arg(long, default_value_t = 0x94)]
    router_send_topic: u16,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Send a SubmitJob directly (no ColonyOS).
    Send {
        /// AOI west longitude (degrees).
        #[arg(long, default_value_t = -122.5)]
        west: f32,
        /// AOI south latitude (degrees).
        #[arg(long, default_value_t = 38.0)]
        south: f32,
        /// AOI east longitude (degrees).
        #[arg(long, default_value_t = -121.5)]
        east: f32,
        /// AOI north latitude (degrees).
        #[arg(long, default_value_t = 39.0)]
        north: f32,
        /// Data volume per collector (bytes).
        #[arg(long, default_value_t = 8192)]
        data_volume: u64,
        /// Job ID.
        #[arg(long, default_value_t = 1)]
        job_id: u16,
    },
    /// Poll ColonyOS for jobs and dispatch via SRSPP.
    Serve {
        /// Colony name.
        #[arg(long, env = "COLONIES_COLONY_NAME")]
        colony: String,
        /// Executor private key (hex).
        #[arg(long, env = "COLONIES_EXECUTOR_PRVKEY")]
        executor_key: String,
        /// Colony owner private key (hex).
        #[arg(long, env = "COLONIES_COLONY_PRVKEY")]
        colony_key: String,
        /// ColonyOS server URL.
        #[arg(long, env = "COLONIES_SERVER_URL", default_value = "http://localhost:50080")]
        server_url: String,
        /// Executor name.
        #[arg(long, default_value = "leodos-ground")]
        name: String,
        /// Poll timeout in seconds.
        #[arg(long, default_value_t = 10)]
        poll_timeout: i32,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match &args().command {
        Command::Send { west, south, east, north, data_volume, job_id } => {
            let a = args();
            let aoi = GeoAoi::new(
                LatLon::new(*north, *west),
                LatLon::new(*south, *east),
            );
            send_job(&a, aoi, *data_volume, *job_id).await
        }
        Command::Serve { colony, executor_key, colony_key, server_url, name, poll_timeout } => {
            serve_loop(colony, executor_key, colony_key, server_url, name, *poll_timeout).await
        }
    }
}

fn args() -> Args {
    Args::parse()
}

async fn send_job(
    args: &Args,
    aoi: GeoAoi,
    data_volume: u64,
    job_id: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Sending SubmitJob (id={job_id})");
    println!("  AOI: ({}, {}) to ({}, {})",
        aoi.upper_left.lat_deg, aoi.upper_left.lon_deg,
        aoi.lower_right.lat_deg, aoi.lower_right.lon_deg);

    let job = Job::builder()
        .geo_aoi(aoi)
        .data_volume_bytes(data_volume)
        .build();

    let header = SpaceCompHeader::new(OpCode::SubmitJob, job_id);
    let mut msg_buf = [0u8; 256];
    let hdr = header.as_bytes();
    let payload = job.as_bytes();
    let msg_len = hdr.len() + payload.len();
    msg_buf[..hdr.len()].copy_from_slice(hdr);
    msg_buf[hdr.len()..msg_len].copy_from_slice(payload);

    let coordinator = Address::satellite(0, 0);
    let link = udp_link::UdpLink::new(&args.local_addr, &args.ci_lab, args.router_send_topic).await?;

    let config = SenderConfig::builder()
        .source_address(Address::Ground { station: 0 })
        .apid(Apid::new(args.apid).unwrap())
        .function_code(0)
        .rto_ticks(1000)
        .max_retransmits(3)
        .header_overhead(SrsppDataPacket::HEADER_SIZE)
        .build();

    let mut sender: SrsppSender<_, _, 8, 4096, 512> =
        SrsppSender::new(config, link, FixedRto::new(1000), 100);

    sender.send(coordinator, &msg_buf[..msg_len]).await
        .map_err(|e| format!("SRSPP send: {e}"))?;

    println!("  Sent via SRSPP, waiting for ACK...");

    match tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        sender.flush(),
    ).await {
        Ok(Ok(())) => println!("  ACK received."),
        Ok(Err(e)) => println!("  Flush error: {e}"),
        Err(_) => println!("  Timeout waiting for ACK (packet may still have been delivered)."),
    }

    Ok(())
}

async fn serve_loop(
    colony: &str,
    executor_key: &str,
    colony_key: &str,
    server_url: &str,
    name: &str,
    poll_timeout: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use colonyos::core::Executor;
    use colonyos::core::Log;

    colonyos::set_server_url(server_url);

    let executor_id = colonyos::crypto::gen_id(executor_key);
    println!("Executor ID: {executor_id}");

    let executor = Executor::new(name, &executor_id, "leodos", colony);
    match colonyos::add_executor(&executor, executor_key).await {
        Ok(_) => println!("Registered executor: {name}"),
        Err(e) => println!("Registration: {e}"),
    }
    match colonyos::approve_executor(colony, name, colony_key).await {
        Ok(_) => println!("Executor approved"),
        Err(e) => println!("Approval: {e}"),
    }

    println!("Polling for SpaceCoMP jobs...");
    let args = Args::parse();

    loop {
        let process = match colonyos::assign(colony, poll_timeout, executor_key).await {
            Ok(p) => p,
            Err(e) => {
                if e.conn_err() {
                    eprintln!("Connection error: {e}");
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                continue;
            }
        };

        println!("Assigned: {} ({})", process.processid, process.spec.funcname);

        if process.spec.funcname != "spacecomp" {
            eprintln!("Unknown function: {}", process.spec.funcname);
            let _ = colonyos::fail(&process.processid, executor_key).await;
            continue;
        }

        let kwargs = &process.spec.kwargs;
        let aoi = GeoAoi::new(
            LatLon::new(
                get_f64(kwargs, "aoi_north").unwrap_or(39.0) as f32,
                get_f64(kwargs, "aoi_west").unwrap_or(-122.5) as f32,
            ),
            LatLon::new(
                get_f64(kwargs, "aoi_south").unwrap_or(38.0) as f32,
                get_f64(kwargs, "aoi_east").unwrap_or(-121.5) as f32,
            ),
        );
        let data_volume = get_f64(kwargs, "data_volume").unwrap_or(8192.0) as u64;

        let log = Log {
            processid: process.processid.clone(),
            colonyname: colony.to_string(),
            executorname: name.to_string(),
            message: "Processing SpaceCoMP job".to_string(),
            timestamp: 0,
        };
        let _ = colonyos::add_log(&log, executor_key).await;

        match send_job(&args, aoi, data_volume, 1).await {
            Ok(()) => {
                let _ = colonyos::set_output(
                    &process.processid,
                    vec!["SubmitJob sent".to_string()],
                    executor_key,
                ).await;
                let _ = colonyos::close(&process.processid, executor_key).await;
                println!("Job completed: {}", process.processid);
            }
            Err(e) => {
                let _ = colonyos::fail(&process.processid, executor_key).await;
                eprintln!("Job failed: {e}");
            }
        }
    }
}

fn get_f64(kwargs: &std::collections::HashMap<String, serde_json::Value>, key: &str) -> Option<f64> {
    kwargs.get(key).and_then(|v| v.as_f64())
}
