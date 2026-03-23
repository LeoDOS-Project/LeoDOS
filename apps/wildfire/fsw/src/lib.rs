#![no_std]

use leodos_libcfs::app::App;
use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::es::cds::CdsBlock;
use leodos_libcfs::cfe::es::cds::CdsInfo;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::tbl::Table;
use leodos_libcfs::cfe::tbl::TableOptions;
use leodos_libcfs::err;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::info;
use leodos_libcfs::nos3::buses::spi::Spi;
use leodos_libcfs::nos3::buses::uart::Uart;
use leodos_libcfs::nos3::drivers::novatel;
use leodos_libcfs::runtime::join::join;
use leodos_libcfs::runtime::time::sleep;
use leodos_libcfs::runtime::Runtime;

use leodos_protocols::application::compression::rice;
use leodos_protocols::datalink::link::cfs::sb::SbDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::ptp::PointToPoint;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::SrsppSender;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

// ── Thermal camera SPI registers ────────────────────────────

const REG_STATUS: u8 = 0x01;
const REG_TRIGGER: u8 = 0x02;
const REG_NUM_BANDS: u8 = 0x0F;
const REG_WIDTH: u8 = 0x10;
const REG_HEIGHT: u8 = 0x11;
const REG_FIFO_SIZE_0: u8 = 0x12;
const REG_FIFO_SIZE_1: u8 = 0x13;
const REG_FIFO_SIZE_2: u8 = 0x14;
const REG_FIFO_READ: u8 = 0x20;

const MAX_PIXELS: usize = 512 * 512;
const MAX_HOTSPOTS: usize = 64;
const RTO_MS: u32 = 1000;
const QUANT_OFFSET: f32 = 200.0;
const QUANT_SCALE: f32 = 100.0;

// ── Table-based configuration ───────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct WildfireConfig {
    aoi_west: f32,
    aoi_south: f32,
    aoi_east: f32,
    aoi_north: f32,
    bt_threshold_k: f32,
    min_cluster_pixels: u32,
    poll_interval_s: u32,
}

impl Default for WildfireConfig {
    fn default() -> Self {
        Self {
            aoi_west: -122.5,
            aoi_south: 38.0,
            aoi_east: -121.5,
            aoi_north: 39.0,
            bt_threshold_k: 330.0,
            min_cluster_pixels: 3,
            poll_interval_s: 2,
        }
    }
}

// ── CDS-persisted state ─────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct WildfireState {
    pass_count: u32,
    alerts_sent: u32,
}

// ── Sensor abstraction ──────────────────────────────────────

struct Alert {
    lat: f32,
    lon: f32,
    hot_pixel_count: u32,
    max_temp_k: f32,
    temps: [f32; MAX_HOTSPOTS],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct AlertTlm {
    lat: f32,
    lon: f32,
    hot_pixel_count: u32,
    max_temp_k: f32,
    pass_number: u32,
}

/// Capture result: (width, height, num_bands).
/// Band 0 = MWIR, Band 1 = LWIR (if present).
trait ThermalSensor {
    fn capture(&mut self, mwir: &mut [f32], lwir: &mut [f32]) -> Result<(u32, u32, u8), CfsError>;
}

struct SpiCamera {
    spi: Spi,
}

impl SpiCamera {
    fn read_reg(&mut self, reg: u8) -> u8 {
        let tx = [reg, 0x00];
        let mut rx = [0u8; 2];
        self.spi.transfer(&tx, &mut rx, 2, 0, 8, true).ok();
        rx[0]
    }

    fn write_reg(&mut self, reg: u8, val: u8) {
        let tx = [reg | 0x80, val];
        self.spi.write(&tx).ok();
    }
}

impl ThermalSensor for SpiCamera {
    fn capture(&mut self, mwir: &mut [f32], lwir: &mut [f32]) -> Result<(u32, u32, u8), CfsError> {
        let status = self.read_reg(REG_STATUS);
        if status & 0x02 == 0 {
            return Err(CfsError::IncorrectState);
        }

        self.write_reg(REG_TRIGGER, 0x01);

        let num_bands = self.read_reg(REG_NUM_BANDS).max(1);
        let width = self.read_reg(REG_WIDTH) as u32;
        let height = self.read_reg(REG_HEIGHT) as u32;

        let s0 = self.read_reg(REG_FIFO_SIZE_0) as u32;
        let s1 = self.read_reg(REG_FIFO_SIZE_1) as u32;
        let s2 = self.read_reg(REG_FIFO_SIZE_2) as u32;
        let fifo_bytes = s0 | (s1 << 8) | (s2 << 16);
        let pixels_per_band = (width * height) as usize;
        let total_pixels = (fifo_bytes / 4) as usize;

        let mut bytes = [0u8; 4];

        // Read MWIR band
        let n_mwir = pixels_per_band.min(mwir.len()).min(total_pixels);
        for pixel in mwir.iter_mut().take(n_mwir) {
            for b in &mut bytes {
                *b = self.read_reg(REG_FIFO_READ);
            }
            *pixel = f32::from_le_bytes(bytes);
        }

        // Read LWIR band (if present)
        if num_bands >= 2 {
            let remaining = total_pixels.saturating_sub(pixels_per_band);
            let n_lwir = pixels_per_band.min(lwir.len()).min(remaining);
            for pixel in lwir.iter_mut().take(n_lwir) {
                for b in &mut bytes {
                    *b = self.read_reg(REG_FIFO_READ);
                }
                *pixel = f32::from_le_bytes(bytes);
            }
        } else {
            // Single band: use MWIR as LWIR estimate
            let n = n_mwir.min(lwir.len());
            lwir[..n].copy_from_slice(&mwir[..n]);
        }

        Ok((width, height, num_bands))
    }
}

// ── Detection (using leodos-analysis) ───────────────────────

fn detect_hotspots(
    mwir: &[f32],
    lwir: &[f32],
    width: u32,
    height: u32,
    cfg: &WildfireConfig,
) -> Option<Alert> {
    let thresholds = leodos_analysis::thermal::FireThresholds {
        t4_abs: cfg.bt_threshold_k,
        ..leodos_analysis::thermal::FireThresholds::day()
    };

    let mut hotspots = [leodos_analysis::thermal::Hotspot {
        x: 0,
        y: 0,
        t4: 0.0,
        t11: 0.0,
        dt4: 0.0,
        dt4_t11: 0.0,
        frp: 0.0,
        confidence: 0.0,
    }; 64];

    let n = leodos_analysis::thermal::detect_fire(
        mwir,
        lwir,
        width as usize,
        height as usize,
        &thresholds,
        &mut hotspots,
    );

    if (n as u32) < cfg.min_cluster_pixels {
        return None;
    }

    let mut sum_lat = 0.0f32;
    let mut sum_lon = 0.0f32;
    let mut max_temp = 0.0f32;

    for h in &hotspots[..n] {
        let lat = cfg.aoi_north
            - h.y as f32 / height as f32 * (cfg.aoi_north - cfg.aoi_south);
        let lon = cfg.aoi_west
            + h.x as f32 / width as f32 * (cfg.aoi_east - cfg.aoi_west);
        sum_lat += lat;
        sum_lon += lon;
        if h.t4 > max_temp {
            max_temp = h.t4;
        }
    }

    let mut temps = [0.0f32; MAX_HOTSPOTS];
    for (i, h) in hotspots[..n].iter().enumerate() {
        temps[i] = h.t4;
    }

    Some(Alert {
        lat: sum_lat / n as f32,
        lon: sum_lon / n as f32,
        hot_pixel_count: n as u32,
        max_temp_k: max_temp,
        temps,
    })
}

// ── App entry ───────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn WILDFIRE_AppMain() {
    Runtime::new().run(async {
        let mut app = App::builder()
            .name("WILDFIRE")
            .cmd_topic(bindings::WILDFIRE_CMD_TOPICID as u16)
            .send_hk_topic(bindings::WILDFIRE_SEND_HK_TOPICID as u16)
            .hk_tlm_topic(bindings::WILDFIRE_HK_TLM_TOPICID as u16)
            .version("0.1.0")
            .build()?;

        // Table config (ground-updatable)
        let table = Table::<WildfireConfig>::new("WILDFIRE.Config", TableOptions::DEFAULT, None)?;
        table.load_from_slice(core::slice::from_ref(&WildfireConfig::default()))?;

        // CDS persistence
        let (cds, cds_info) = CdsBlock::<WildfireState>::new("WILDFIRE.State")?;
        let mut state = match cds_info {
            CdsInfo::Restored => cds.restore().unwrap_or_default(),
            CdsInfo::Created => WildfireState::default(),
        };

        // SRSPP transport via router app's Software Bus
        let router_send = MsgId::from_local_cmd(bindings::WILDFIRE_CMD_TOPICID as u16);
        let router_recv = MsgId::from_local_tlm(bindings::WILDFIRE_HK_TLM_TOPICID as u16);
        let sb = SbDatalink::new("WF_SB", 8, router_recv, router_send)?;
        let mut network = PointToPoint::new(sb);

        let apid = Apid::new(bindings::WILDFIRE_APID as u16).unwrap();
        let sender_config = SenderConfig::builder()
            .source_address(Address::satellite(0, 1))
            .apid(apid)
            .function_code(0)
            .rto_ticks(RTO_MS)
            .max_retransmits(3)
            .header_overhead(SrsppDataPacket::HEADER_SIZE)
            .build();
        let origin = Address::satellite(0, 1);
        let sender = SrsppSender::new(sender_config, origin, NoStore, AlwaysReachable);
        let (mut tx, mut driver) = sender.split(FixedRto::new(RTO_MS));

        // Hardware
        let mut camera: Option<SpiCamera> = Spi::open(c"spi_3", 0, 3, 1_000_000, 0, 8)
            .map(|spi| SpiCamera { spi })
            .ok();

        let mut gps = Uart::open(c"/dev/ttyS1", 115_200, leodos_libcfs::nos3::buses::uart::Access::ReadWrite).ok();

        let mut was_over_aoi = false;

        let workflow = async {
            let mut cmd_buf = [0u8; 256];

            loop {
                // Check for commands (non-blocking via sleep)
                // The App::recv handles NoOp, Reset, HK
                // automatically. We use sleep for the poll loop.

                table.manage().ok();

                let cfg = match table.get_accessor() {
                    Ok(acc) => *acc,
                    Err(_) => WildfireConfig::default(),
                };

                let (lat, lon) = if let Some(ref mut dev) = gps {
                    match novatel::request_data(dev) {
                        Ok(d) => (d.lat, d.lon),
                        Err(_) => {
                            sleep(Duration::from_secs(cfg.poll_interval_s)).await;
                            continue;
                        }
                    }
                } else {
                    sleep(Duration::from_secs(cfg.poll_interval_s)).await;
                    continue;
                };

                let over_aoi = lat >= cfg.aoi_south
                    && lat <= cfg.aoi_north
                    && lon >= cfg.aoi_west
                    && lon <= cfg.aoi_east;

                if over_aoi && !was_over_aoi {
                    state.pass_count += 1;
                    info!("Entering AOI pass {}", state.pass_count).ok();

                    if let Some(ref mut cam) = camera {
                        let mut mwir = [0.0f32; MAX_PIXELS];
                        let mut lwir = [0.0f32; MAX_PIXELS];
                        match cam.capture(&mut mwir, &mut lwir) {
                            Ok((w, h, _bands)) => {
                                let n = (w * h) as usize;
                                if let Some(alert) = detect_hotspots(&mwir[..n], &lwir[..n], w, h, &cfg) {
                                    state.alerts_sent += 1;
                                    info!(
                                        "ALERT #{}: {} px, \
                                         max {} K at ({},{})",
                                        state.alerts_sent,
                                        alert.hot_pixel_count,
                                        alert.max_temp_k,
                                        alert.lat,
                                        alert.lon,
                                    )
                                    .ok();

                                    let tlm = AlertTlm {
                                        lat: alert.lat,
                                        lon: alert.lon,
                                        hot_pixel_count: alert.hot_pixel_count,
                                        max_temp_k: alert.max_temp_k,
                                        pass_number: state.pass_count,
                                    };
                                    let tlm_bytes = unsafe {
                                        core::slice::from_raw_parts(
                                            &tlm as *const _ as *const u8,
                                            core::mem::size_of::<AlertTlm>(),
                                        )
                                    };

                                    // Rice-compress hotspot temperatures
                                    let n_hot = (alert.hot_pixel_count as usize).min(MAX_HOTSPOTS);
                                    let padded = ((n_hot + 7) / 8) * 8;
                                    let mut samples = [0u32; MAX_HOTSPOTS];
                                    for (i, t) in alert.temps[..n_hot].iter().enumerate() {
                                        samples[i] = ((t - QUANT_OFFSET) * QUANT_SCALE) as u32;
                                    }
                                    let rice_cfg = rice::Config {
                                        bits_per_sample: 16,
                                        block_size: 8,
                                        ref_interval: 0,
                                        preprocessor: true,
                                    };
                                    // Pack alert + compressed pixels into one message
                                    const TLM_SIZE: usize = core::mem::size_of::<AlertTlm>();
                                    let mut msg = [0u8; TLM_SIZE + 256];
                                    msg[..TLM_SIZE].copy_from_slice(tlm_bytes);
                                    let compressed_len = rice::compress(
                                        &rice_cfg,
                                        &samples[..padded],
                                        &mut msg[TLM_SIZE..],
                                    ).unwrap_or(0);
                                    let total = TLM_SIZE + compressed_len;
                                    tx.send(Address::Ground { station: 0 }, &msg[..total]).await.ok();
                                    info!("Downlinked alert + {} hotspot temps ({} bytes compressed)", n_hot, compressed_len).ok();
                                } else {
                                    info!("No fire detected").ok();
                                }
                            }
                            Err(e) => {
                                err!("Capture: {}", e).ok();
                            }
                        }
                    }

                    cds.store(&state).ok();
                } else if !over_aoi && was_over_aoi {
                    info!("Leaving AOI").ok();
                }

                was_over_aoi = over_aoi;
                sleep(Duration::from_secs(cfg.poll_interval_s)).await;
            }
        };

        let _ = join(workflow, driver.run(&mut network)).await;

        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
