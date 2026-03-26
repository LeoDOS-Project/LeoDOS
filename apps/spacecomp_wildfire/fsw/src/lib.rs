#![no_std]

use leodos_libcfs::info;
use leodos_libcfs::nos3::drivers::geo_camera::GeoCamera;

use leodos_analysis::cluster::SpatialClusterer;
use leodos_analysis::frame::TileMessage;
use leodos_analysis::thermal::detect_fire;
use leodos_analysis::thermal::FireThresholds;

use leodos_libcfs::warn;
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
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
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
    camera: GeoCamera,
    thresholds: FireThresholds,
}

impl WildfireApp {
    fn new() -> Result<Self, SpaceCompError> {
        Ok(Self {
            camera: GeoCamera::builder()
                .device(c"spi_3")
                .chip_select_line(3)
                .baudrate(1_000_000)
                .gps_device(c"/dev/ttyS1")
                .gps_baud(115_200)
                .altitude_m(ALTITUDE_M)
                .focal_length_mm(FOCAL_LENGTH_MM)
                .pixel_pitch_um(PIXEL_PITCH_UM)
                .build()?,
            thresholds: FireThresholds {
                t4_abs: 330.0,
                ..Default::default()
            },
        })
    }
}

impl SpaceComp for WildfireApp {
    async fn collect(&mut self, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut mwir = [0.0f32; MAX_PIXELS];
        let mut lwir = [0.0f32; MAX_PIXELS];
        let geo_frame = self.camera.capture(&mut mwir, &mut lwir).await?;

        let mut buf = [0u8; 8192];
        let mut tile_count = 0;
        for tile in geo_frame.tiles(TILE_SIZE, TILE_OVERLAP) {
            let len = tile.write_to(&mut buf);
            tx.send(&buf[..len]).await?;
            tile_count += 1;
        }

        info!("Collector: sent {} tiles", tile_count)?;
        Ok(())
    }

    async fn map(&mut self, data: &[u8], mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let Some(tile) = TileMessage::from_bytes(data) else {
            return Ok(());
        };
        let mut tx = tx.batched::<HotspotRecord>();
        for hs in detect_fire(&tile, &self.thresholds) {
            tx.write(&HotspotRecord::new(hs.lat, hs.lon, hs.t4)).await?;
        }
        tx.flush().await?;
        Ok(())
    }

    async fn reduce(&mut self, mut rx: impl Rx, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut tx = tx.batched::<WildfireEvent>();
        let mut clusterer = SpatialClusterer::<MAX_HOTSPOTS>::new(CLUSTER_RADIUS_DEG);

        let mut buf = [0u8; 4096];
        while let Some(Ok(records)) = rx.recv_batch::<HotspotRecord>(&mut buf).await {
            for hr in records {
                if clusterer
                    .add(hr.lat.get(), hr.lon.get(), hr.t4.get())
                    .is_err()
                {
                    warn!("Reducer: Clusterer full, skipping remaining hotspots")?;
                    break;
                }
            }
        }

        let mut fire_count = 0;
        for c in clusterer.clusters() {
            tx.write(&WildfireEvent::new(
                c.centroid_x,
                c.centroid_y,
                c.max_value,
                c.count,
            ))
            .await?;
            fire_count += 1;
        }
        tx.flush().await?;

        info!("Reducer: Clustered {} fires", fire_count)?;
        Ok(())
    }
}

// ── Entry point ─────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn SPACECOMP_WILDFIRE_AppMain() {
    SpaceCompNode::builder()
        .app_fn(move || WildfireApp::new())
        .config(
            SpaceCompConfig::builder()
                .num_orbits(bindings::SPACECOMP_WILDFIRE_NUM_ORBITS as u8)
                .num_sats(bindings::SPACECOMP_WILDFIRE_NUM_SATS as u8)
                .altitude_m(550_000.0)
                .inclination_deg(87.0)
                .apid(Apid::new(bindings::SPACECOMP_WILDFIRE_APID as u16).unwrap())
                .rto_ms(1000)
                .router_send_topic(0)
                .router_recv_topic(0)
                .build(),
        )
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
