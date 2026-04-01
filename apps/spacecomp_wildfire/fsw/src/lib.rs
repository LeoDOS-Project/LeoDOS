#![no_std]

use leodos_libcfs::cell::TaskLocalCell;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::log;
use leodos_libcfs::nos3::drivers::geo_camera::GeoCamera;

use leodos_analysis::cluster::SpatialClusterer;
use leodos_analysis::frame::TileMessage;
use leodos_analysis::thermal::detect_fire;
use leodos_analysis::thermal::FireThresholds;

use leodos_protocols::fmt_cstr;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;

use leodos_spacecomp::node::SpaceComp;
use leodos_spacecomp::transport::Rx;
use leodos_spacecomp::transport::Tx;
use leodos_spacecomp::SpaceCompConfig;
use leodos_spacecomp::SpaceCompError;
use leodos_spacecomp::SpaceCompNode;

use zerocopy::network_endian::F32;
use zerocopy::network_endian::U16;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

mod bindings {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const MAX_PIXELS: usize = 512 * 512;
const MAX_HOTSPOTS: usize = 256;

const TILE_SIZE: usize = 32;
const TILE_OVERLAP: usize = 3;
const FOCAL_LENGTH_MM: f32 = 50.0;
const PIXEL_PITCH_UM: f32 = 15.0;
const ALTITUDE_M: f32 = 550_000.0;
const CLUSTER_RADIUS_DEG: f32 = 0.01;

// ── Wire types ──────────────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
struct HotspotRecord {
    lat: F32,
    lon: F32,
    t4: F32,
}

impl HotspotRecord {
    fn new(lat: f32, lon: f32, t4: f32) -> Self {
        Self {
            lat: lat.into(),
            lon: lon.into(),
            t4: t4.into(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
struct WildfireEvent {
    lat: F32,
    lon: F32,
    max_temp: F32,
    pixel_count: U16,
}

impl WildfireEvent {
    fn new(lat: f32, lon: f32, max_temp: f32, pixel_count: u16) -> Self {
        Self {
            lat: lat.into(),
            lon: lon.into(),
            max_temp: max_temp.into(),
            pixel_count: pixel_count.into(),
        }
    }
}

// ── SpaceComp implementation ────────────────────────────────

struct WildfireApp {
    camera: Option<GeoCamera>,
    thresholds: FireThresholds,
}

/// Converts SCID (e.g. 1001, 2003) to 0-based linear index.
fn scid_to_index(scid: u32, num_sats: u32) -> u32 {
    let orbit = scid / 1000 - 1;
    let sat = scid % 1000 - 1;
    orbit * num_sats + sat
}

impl WildfireApp {
    fn new() -> Result<Self, SpaceCompError> {
        Ok(Self {
            camera: None,
            thresholds: FireThresholds {
                t4_abs: 330.0,
                ..Default::default()
            },
        })
    }

    fn camera(&mut self) -> &mut GeoCamera {
        self.camera.as_mut().expect("camera not initialized")
    }
}

impl SpaceComp for WildfireApp {
    fn init(&mut self) -> Result<(), SpaceCompError> {
        let scid = system::get_spacecraft_id();
        let num_sats = bindings::SPACECOMP_WILDFIRE_NUM_SATS as u32;
        let sc_index = scid_to_index(scid, num_sats);

        self.camera = Some(
            GeoCamera::builder()
                .device(&fmt_cstr!(32, "spi_sc{}", sc_index)?)
                .chip_select_line(3)
                .baudrate(1_000_000)
                .altitude_m(ALTITUDE_M)
                .focal_length_mm(FOCAL_LENGTH_MM)
                .pixel_pitch_um(PIXEL_PITCH_UM)
                .build()?,
        );
        log!("Wildfire app initialized (SCID={})", scid)?;
        Ok(())
    }

    async fn collect(&mut self, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        static MWIR: TaskLocalCell<[f32; MAX_PIXELS]> = TaskLocalCell::new([0.0; MAX_PIXELS]);
        static LWIR: TaskLocalCell<[f32; MAX_PIXELS]> = TaskLocalCell::new([0.0; MAX_PIXELS]);
        let mut buf = [0u8; 8192];
        let geo_frame = self.camera().capture(MWIR.get_mut(), LWIR.get_mut()).await?;

        let mut tile_count = 0;
        for tile in geo_frame.tiles(TILE_SIZE, TILE_OVERLAP) {
            let len = tile.write_to(&mut buf);
            tx.send(&buf[..len]).await?;
            tile_count += 1;
        }

        log!("Collector: sent {} tiles", tile_count)?;
        Ok(())
    }

    async fn map(&mut self, data: &[u8], mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut buf = [0u8; 4096];
        let Some(tile) = TileMessage::from_bytes(data) else {
            return Ok(());
        };
        let mut w = tx.batched::<HotspotRecord>(&mut buf);
        for h in detect_fire(&tile, &self.thresholds) {
            w.write(&HotspotRecord::new(h.lat, h.lon, h.t4)).await?;
        }
        w.flush().await?;
        Ok(())
    }

    async fn reduce(&mut self, mut rx: impl Rx, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut buf = [0u8; 4096];
        let mut clusterer = SpatialClusterer::<MAX_HOTSPOTS>::new(CLUSTER_RADIUS_DEG);
        while let Some(Ok(records)) = rx.recv_batch::<HotspotRecord>(&mut buf).await {
            for hr in records {
                if clusterer
                    .add(hr.lat.get(), hr.lon.get(), hr.t4.get())
                    .is_err()
                {
                    log!("Reducer: Clusterer full, skipping remaining hotspots")?;
                    break;
                }
            }
        }

        let mut w = tx.batched::<WildfireEvent>(&mut buf);
        let mut fire_count = 0;
        for c in clusterer.clusters() {
            w.write(&WildfireEvent::new(
                c.centroid_x,
                c.centroid_y,
                c.max_value,
                c.count,
            ))
            .await?;
            fire_count += 1;
        }
        w.flush().await?;

        log!("Reducer: Clustered {} fires", fire_count)?;
        Ok(())
    }
}

// ── Entry point ─────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn SC_WILDFIRE_AppMain() {
    SpaceCompNode::builder()
        .config(
            SpaceCompConfig::builder()
                .num_orbits(bindings::SPACECOMP_WILDFIRE_NUM_ORBITS as u8)
                .num_sats(bindings::SPACECOMP_WILDFIRE_NUM_SATS as u8)
                .altitude_m(550_000.0)
                .inclination_deg(87.0)
                .apid(Apid::new(bindings::SPACECOMP_WILDFIRE_APID as u16).unwrap())
                .rto_ms(1000)
                .router_send_topic(bindings::ROUTER_SEND_TOPICID as u16)
                .router_recv_topic(bindings::ROUTER_RECV_TOPICID as u16)
                .build(),
        )
        .app_fn(WildfireApp::new)
        .store(NoStore)
        .reachable(AlwaysReachable)
        .build()
        .start();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
