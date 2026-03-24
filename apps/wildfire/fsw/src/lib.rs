#![no_std]

use core::cell::Cell;
use core::time::Duration;
use leodos_libcfs::app::App;
use leodos_libcfs::app::Event;
use leodos_libcfs::cfe::es::cds::CdsBlock;
use leodos_libcfs::cfe::es::system::wait_for_startup_sync;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::tbl::Table;
use leodos_libcfs::cfe::tbl::TableOptions;
use leodos_libcfs::cfe::tbl::Validate;
use leodos_libcfs::err;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::info;
use leodos_libcfs::join;
use leodos_libcfs::nos3::drivers::novatel::Gps;
use leodos_libcfs::nos3::drivers::thermal_cam::ThermalCamError;
use leodos_libcfs::nos3::drivers::thermal_cam::ThermalCamera;
use leodos_libcfs::runtime::Runtime;

use leodos_analysis::geo::GeoBounds;
use leodos_analysis::thermal::detect_fire;
use leodos_analysis::thermal::FireThresholds;
use leodos_analysis::thermal::Hotspot;

use leodos_protocols::datalink::link::cfs::sb::SbDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::address::SpacecraftId;
use leodos_protocols::network::ptp::PointToPoint;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::SrsppSender;
use leodos_protocols::transport::srspp::api::cfs::SrsppTxHandle;
use leodos_protocols::transport::srspp::api::cfs::TransportError;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

use zerocopy::IntoBytes;

type TxHandle<'a> = SrsppTxHandle<'a, CfsError, NoStore, AlwaysReachable, 8, 4096, 512>;
type Camera = ThermalCamera<MAX_PIXELS>;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const MAX_PIXELS: usize = 512 * 512;
const MAX_HOTSPOTS: usize = 64;
const NUM_SATS: u8 = 3;
const RTO_MS: u32 = 1000;

// ── Table-based configuration ───────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct WildfireConfig {
    aoi: GeoBounds,
    bt_threshold_k: f32,
    min_cluster_pixels: u32,
}

impl Default for WildfireConfig {
    fn default() -> Self {
        Self {
            aoi: GeoBounds {
                west: -122.5,
                south: 38.0,
                east: -121.5,
                north: 39.0,
            },
            bt_threshold_k: 330.0,
            min_cluster_pixels: 3,
        }
    }
}

impl Validate for WildfireConfig {
    fn validate(&self) -> bool {
        self.aoi.south < self.aoi.north
            && self.aoi.west < self.aoi.east
            && self.bt_threshold_k > 0.0
            && self.min_cluster_pixels > 0
    }
}

// ── CDS-persisted state ─────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct WildfireState {
    pass_count: u32,
    alerts_sent: u32,
}

/// Housekeeping telemetry published on HK wakeup.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WildfireHk {
    cmd_count: u16,
    err_count: u16,
    pass_count: u32,
    alerts_sent: u32,
}

// ── Sensor abstraction ──────────────────────────────────────

#[repr(C)]
#[derive(zerocopy::IntoBytes, zerocopy::Immutable)]
struct AlertTlm {
    lat: f32,
    lon: f32,
    hot_pixel_count: u32,
    max_temp_k: f32,
    pass_number: u32,
}

// ── Error ───────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
enum WildfireError {
    #[error(transparent)]
    Cfs(#[from] CfsError),
    #[error(transparent)]
    Transport(#[from] TransportError<CfsError>),
    #[error(transparent)]
    Camera(#[from] ThermalCamError),
}

// ── App entry ───────────────────────────────────────────────

async fn main() -> Result<(), WildfireError> {
    let mut app = App::builder()
        .name("WILDFIRE")
        .cmd_topic(bindings::WILDFIRE_CMD_TOPICID as u16)
        .send_hk_topic(bindings::WILDFIRE_SEND_HK_TOPICID as u16)
        .hk_tlm_topic(bindings::WILDFIRE_HK_TLM_TOPICID as u16)
        .version("0.1.0")
        .build()?;

    // Table config (ground-updatable)
    let table = Table::<WildfireConfig>::new("WILDFIRE.Config", TableOptions::DEFAULT)?;

    // CDS persistence
    let (cds, state_init) = CdsBlock::<WildfireState>::restore_or_default("WILDFIRE.State")?;
    let state = Cell::new(state_init);

    // Derive address from cFS spacecraft ID
    let scid = SpacecraftId::new(leodos_libcfs::cfe::es::system::get_spacecraft_id());
    let address = scid.to_address(NUM_SATS);

    // SRSPP transport via router app's Software Bus
    let router_send = MsgId::local_cmd(bindings::WILDFIRE_CMD_TOPICID as u16);
    let router_recv = MsgId::local_tlm(bindings::WILDFIRE_HK_TLM_TOPICID as u16);
    let sb = SbDatalink::new("WF_SB", 8, router_recv, router_send)?;
    let mut network = PointToPoint::new(sb);

    let sender_config = SenderConfig::builder()
        .source_address(address)
        .apid(Apid::new(bindings::WILDFIRE_APID as u16).unwrap())
        .function_code(0)
        .rto_ticks(RTO_MS)
        .max_retransmits(3)
        .header_overhead(SrsppDataPacket::HEADER_SIZE)
        .build();
    let sender: SrsppSender<_, _, _, 8, 4096, 512> =
        SrsppSender::new(sender_config, address, NoStore, AlwaysReachable);
    let (mut tx, mut driver) = sender.split(FixedRto::new(RTO_MS));

    let mut camera = Camera::builder()
        .device(c"spi_3")
        .chip_select_line(3)
        .baudrate(1_000_000)
        .build()?;
    let mut gps = Gps::builder().device(c"/dev/ttyS1").baud(115_200).build()?;

    wait_for_startup_sync(Duration::from_millis(10_000));

    let (wf, drv, cmd) = join!(
        workflow(&table, &cds, &state, &mut camera, &mut gps, &mut tx),
        driver.run(&mut network),
        command_loop(&mut app, &state),
    )
    .await;

    cds.store(&state.get())?;

    wf?;
    drv.map_err(WildfireError::Transport)?;
    cmd?;

    Ok(())
}

async fn command_loop(app: &mut App, state: &Cell<WildfireState>) -> Result<(), WildfireError> {
    let mut buf = [0u8; 256];
    loop {
        match app.recv(&mut buf).await? {
            Event::Hk => {
                let s = state.get();
                app.send_hk(&WildfireHk {
                    cmd_count: app.cmd_count(),
                    err_count: app.err_count(),
                    pass_count: s.pass_count,
                    alerts_sent: s.alerts_sent,
                })?;
            }
            Event::Command(msg) => app.reject(msg)?,
        }
    }
}

async fn workflow(
    table: &Table<WildfireConfig>,
    cds: &CdsBlock<WildfireState>,
    state: &Cell<WildfireState>,
    camera: &mut Camera,
    gps: &mut Gps,
    tx: &mut TxHandle<'_>,
) -> Result<(), WildfireError> {
    let mut was_over_aoi = false;

    loop {
        table.manage()?;
        let cfg = table.get_or_default();

        let data = gps.request_data().await?;
        let over_aoi = cfg.aoi.contains(data.lat, data.lon);

        if over_aoi && !was_over_aoi {
            let mut s = state.get();
            s.pass_count += 1;
            info!("Entering AOI pass {}", s.pass_count)?;
            if let Err(e) = scan_and_downlink(camera, &cfg, &mut s, tx).await {
                err!("Scan failed: {}", e)?;
            }
            state.set(s);
            cds.store(&s)?;
        } else if !over_aoi && was_over_aoi {
            info!("Leaving AOI")?;
        }

        was_over_aoi = over_aoi;
    }
}

async fn scan_and_downlink(
    camera: &mut Camera,
    cfg: &WildfireConfig,
    state: &mut WildfireState,
    tx: &mut TxHandle<'_>,
) -> Result<(), WildfireError> {
    let frame = camera.capture().await?;

    let thresholds = FireThresholds {
        t4_abs: cfg.bt_threshold_k,
        ..Default::default()
    };
    let mut hotspots = [Hotspot::default(); MAX_HOTSPOTS];
    let det = detect_fire(
        frame.mwir,
        frame.lwir,
        frame.width as usize,
        frame.height as usize,
        &thresholds,
        &mut hotspots,
    );

    if det.count < cfg.min_cluster_pixels as usize {
        info!("No fire detected")?;
        return Ok(());
    }

    let centroid = cfg.aoi.pixel_to_latlon(
        det.centroid_x,
        det.centroid_y,
        frame.width as f32,
        frame.height as f32,
    );

    state.alerts_sent += 1;
    info!(
        "ALERT #{}: {} px, max {} K at ({},{})",
        state.alerts_sent, det.count, det.max_temp, centroid.lat, centroid.lon,
    )?;

    let tlm = AlertTlm {
        lat: centroid.lat,
        lon: centroid.lon,
        hot_pixel_count: det.count as u32,
        max_temp_k: det.max_temp,
        pass_number: state.pass_count,
    };

    tx.send(Address::Ground { station: 0 }, tlm.as_bytes())
        .await?;
    info!("Downlinked alert ({} bytes)", tlm.as_bytes().len())?;

    Ok(())
}

#[no_mangle]
pub extern "C" fn WILDFIRE_AppMain() {
    Runtime::new()
        .perf_id(bindings::WILDFIRE_APPMAIN_PERF_ID)
        .run(main());
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
