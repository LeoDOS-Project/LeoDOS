use clap::Parser;
use colonyos::core::Executor;
use colonyos::core::Log;
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

/// LeoDOS ground coordinator executor for ColonyOS.
///
/// Registers as a ColonyOS executor, polls for SpaceCoMP jobs,
/// and dispatches them to the satellite constellation via SRSPP.
#[derive(Parser)]
struct Args {
    /// Colony name.
    #[arg(long, env = "COLONIES_COLONY_NAME")]
    colony: String,

    /// Executor private key (hex).
    #[arg(long, env = "COLONIES_EXECUTOR_PRVKEY")]
    executor_key: String,

    /// Colony owner private key (hex), for initial registration.
    #[arg(long, env = "COLONIES_COLONY_PRVKEY")]
    colony_key: String,

    /// ColonyOS server URL.
    #[arg(long, env = "COLONIES_SERVER_URL", default_value = "http://localhost:50080")]
    server_url: String,

    /// Executor name.
    #[arg(long, default_value = "leodos-ground")]
    name: String,

    /// ci_lab UDP address (command uplink to constellation).
    #[arg(long, default_value = "127.0.0.1:5012")]
    ci_lab: String,

    /// Local UDP bind address for receiving telemetry.
    #[arg(long, default_value = "127.0.0.1:5013")]
    local_addr: String,

    /// APID for SpaceCoMP messages.
    #[arg(long, default_value_t = 0x61)]
    apid: u16,

    /// Poll timeout in seconds.
    #[arg(long, default_value_t = 10)]
    poll_timeout: i32,
}

const EXECUTOR_TYPE: &str = "leodos";
const FUNC_SPACECOMP: &str = "spacecomp";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    colonyos::set_server_url(&args.server_url);

    let executor_id = colonyos::crypto::gen_id(&args.executor_key);
    println!("Executor ID: {executor_id}");

    register(&args, &executor_id).await;

    println!("Polling for SpaceCoMP jobs...");
    loop {
        let process = match colonyos::assign(&args.colony, args.poll_timeout, &args.executor_key).await {
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

        let result = match process.spec.funcname.as_str() {
            FUNC_SPACECOMP => handle_spacecomp(&args, &process).await,
            other => {
                eprintln!("Unknown function: {other}");
                colonyos::fail(&process.processid, &args.executor_key).await?;
                continue;
            }
        };

        match result {
            Ok(output) => {
                colonyos::set_output(&process.processid, output, &args.executor_key).await?;
                colonyos::close(&process.processid, &args.executor_key).await?;
                println!("Job completed: {}", process.processid);
            }
            Err(e) => {
                let log = Log {
                    processid: process.processid.clone(),
                    colonyname: args.colony.clone(),
                    executorname: args.name.clone(),
                    message: format!("Failed: {e}"),
                    timestamp: 0,
                };
                let _ = colonyos::add_log(&log, &args.executor_key).await;
                let _ = colonyos::fail(&process.processid, &args.executor_key).await;
                eprintln!("Job failed: {e}");
            }
        }
    }
}

async fn register(args: &Args, executor_id: &str) {
    let executor = Executor::new(&args.name, executor_id, EXECUTOR_TYPE, &args.colony);

    match colonyos::add_executor(&executor, &args.executor_key).await {
        Ok(_) => println!("Registered executor: {}", args.name),
        Err(e) => println!("Registration: {e}"),
    }

    match colonyos::approve_executor(&args.colony, &args.name, &args.colony_key).await {
        Ok(_) => println!("Executor approved"),
        Err(e) => println!("Approval: {e}"),
    }
}

/// Builds a SubmitJob message and sends it to the coordinator via SRSPP.
async fn handle_spacecomp(
    args: &Args,
    process: &colonyos::core::Process,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let log = Log {
        processid: process.processid.clone(),
        colonyname: args.colony.clone(),
        executorname: args.name.clone(),
        message: "Processing SpaceCoMP job".to_string(),
        timestamp: 0,
    };
    colonyos::add_log(&log, &args.executor_key).await?;

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

    println!("  AOI: {:?} to {:?}", aoi.upper_left, aoi.lower_right);

    let job = Job::builder()
        .geo_aoi(aoi)
        .data_volume_bytes(data_volume)
        .build();

    // Build the SpaceCoMP SubmitJob message
    let header = SpaceCompHeader::new(OpCode::SubmitJob, 1);
    let mut msg_buf = [0u8; 256];
    let header_bytes = header.as_bytes();
    let job_bytes = job.as_bytes();
    let msg_len = header_bytes.len() + job_bytes.len();
    msg_buf[..header_bytes.len()].copy_from_slice(header_bytes);
    msg_buf[header_bytes.len()..msg_len].copy_from_slice(job_bytes);

    // Send via SRSPP to the coordinator satellite (1,1)
    let coordinator = Address::satellite(0, 0);
    let link = udp_link::UdpLink::new(&args.local_addr, &args.ci_lab).await?;

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
    sender.flush().await
        .map_err(|e| format!("SRSPP flush: {e}"))?;

    println!("  SubmitJob sent to coordinator");

    // TODO: Wait for JobResult via SrsppReceiver
    // For now, return immediately after sending

    Ok(vec![format!("SubmitJob sent for AOI {aoi:?}")])
}

fn get_f64(kwargs: &std::collections::HashMap<String, serde_json::Value>, key: &str) -> Option<f64> {
    kwargs.get(key).and_then(|v| v.as_f64())
}
