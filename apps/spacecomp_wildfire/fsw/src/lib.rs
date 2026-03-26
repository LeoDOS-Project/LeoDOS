#![no_std]

use leodos_libcfs::info;
use leodos_libcfs::nos3::drivers::thermal_cam::ThermalCamera;

use leodos_analysis::frame::ReceivedTile;
use leodos_utils::lending_iterator::LendingIterator;
use leodos_analysis::geo::GeoBounds;
use leodos_analysis::thermal::detect_fire;
use leodos_analysis::thermal::FireThresholds;
use leodos_analysis::thermal::Hotspot;

use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;

use leodos_spacecomp::bufwriter::BufWriter;
use leodos_spacecomp::node::SpaceComp;
use leodos_spacecomp::transport::Rx;
use leodos_spacecomp::transport::Tx;
use leodos_spacecomp::SpaceCompConfig;
use leodos_spacecomp::SpaceCompError;
use leodos_spacecomp::SpaceCompNode;

use zerocopy::network_endian::U16;
use zerocopy::network_endian::U32;
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
const MAX_HOTSPOTS: usize = 64;
const NUM_SATS: u8 = bindings::SPACECOMP_WILDFIRE_NUM_SATS as u8;
const TILE_SIZE: usize = 64;
const TILE_OVERLAP: usize = 5;
const TILE_FULL: usize = TILE_SIZE + 2 * TILE_OVERLAP;
const MAX_TILE_PIXELS: usize = TILE_FULL * TILE_FULL;

type Camera = ThermalCamera<MAX_PIXELS>;

// ── Wire types ──────────────────────────────────────────────


#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
struct HotspotRecord {
    x: U16,
    y: U16,
    t4: U32,
}

impl HotspotRecord {
    fn new(x: u16, y: u16, t4: f32) -> Self {
        Self {
            x: U16::new(x),
            y: U16::new(y),
            t4: U32::new(t4.to_bits()),
        }
    }

    fn temp(&self) -> f32 {
        f32::from_bits(self.t4.get())
    }
}

// ── SpaceComp implementation ────────────────────────────────

struct WildfireCompute {
    thresholds: FireThresholds,
    aoi: GeoBounds,
}

impl SpaceComp for WildfireCompute {
    async fn collect(&self, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut camera = Camera::builder()
            .device(c"spi_3")
            .chip_select_line(3)
            .baudrate(1_000_000)
            .build()?;
        let frame = camera.capture().await?;

        let mut mwir_buf = [0.0f32; MAX_TILE_PIXELS];
        let mut lwir_buf = [0.0f32; MAX_TILE_PIXELS];
        let mut send_buf = [0u8; 8192];
        let mut tile_count = 0u32;

        let mut tiles = frame.tiles(TILE_SIZE, TILE_OVERLAP, &mut mwir_buf, &mut lwir_buf);
        while let Some(tile) = tiles.next() {
            let len = tile.write_to(&mut send_buf);
            tx.send(&send_buf[..len]).await?;
            tile_count += 1;
        }

        info!("Collector: sent {} tiles from {}x{} frame", tile_count, frame.width, frame.height)?;
        Ok(())
    }

    async fn map(&self, mut rx: impl Rx, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut writer = BufWriter::<HotspotRecord, _>::new(&mut tx);
        let mut total_hotspots = 0usize;
        let mut tile_buf = [0u8; 8192];

        while let Some(Ok(len)) = rx.recv(&mut tile_buf).await {
            let Some(tile) = ReceivedTile::from_bytes(&tile_buf[..len]) else {
                continue;
            };
            let tw = tile.header.width as usize;
            let th = tile.header.height as usize;

            let mut hotspots = [Hotspot::default(); MAX_HOTSPOTS];
            let det = detect_fire(tile.mwir, tile.lwir, tw, th, &self.thresholds, &mut hotspots);

            let ox = tile.overlap_x();
            let oy = tile.overlap_y();

            for hs in det.hotspots {
                if hs.x >= ox && hs.x < ox + tile.header.inner_w
                    && hs.y >= oy && hs.y < oy + tile.header.inner_h
                {
                    let frame_x = tile.header.frame_x + (hs.x - ox);
                    let frame_y = tile.header.frame_y + (hs.y - oy);
                    writer.write(&HotspotRecord::new(frame_x, frame_y, hs.t4)).await?;
                    total_hotspots += 1;
                }
            }
            writer.flush().await?;
        }

        tx.done().await?;
        info!("Mapper: forwarded {} hotspots", total_hotspots)?;
        Ok(())
    }

    async fn reduce(&self, mut rx: impl Rx, mut tx: impl Tx) -> Result<(), SpaceCompError> {
        let mut all_hotspots = [HotspotRecord::new(0, 0, 0.0); MAX_HOTSPOTS];
        let mut total = 0usize;
        let mut recv_buf = [0u8; 4096];

        while let Some(Ok(len)) = rx.recv(&mut recv_buf).await {
            for rec in recv_buf[..len].chunks_exact(core::mem::size_of::<HotspotRecord>()) {
                let Ok(hr) = HotspotRecord::read_from_bytes(rec) else { continue };
                if total < MAX_HOTSPOTS {
                    all_hotspots[total] = hr.clone();
                    total += 1;
                }
            }
        }

        let mut max_temp = 0.0f32;
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        for rec in &all_hotspots[..total] {
            let t = rec.temp();
            if t > max_temp {
                max_temp = t;
            }
            sum_x += rec.x.get() as f32;
            sum_y += rec.y.get() as f32;
        }
        let n = (total as f32).max(1.0);
        let centroid = self.aoi.pixel_to_latlon(sum_x / n, sum_y / n, 512.0, 512.0);

        info!(
            "Reduced: {} hotspots, max {} K at ({},{})",
            total, max_temp, centroid.lat, centroid.lon,
        )?;

        let mut writer = BufWriter::<HotspotRecord, _>::new(&mut tx);
        for rec in &all_hotspots[..total] {
            writer.write(rec).await?;
        }
        writer.flush().await?;
        Ok(())
    }
}

// ── Entry point ─────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn SPACECOMP_WILDFIRE_AppMain() {
    let config = SpaceCompConfig {
        num_orbits: bindings::SPACECOMP_WILDFIRE_NUM_ORBITS as u8,
        num_sats: NUM_SATS,
        altitude_m: 550_000.0,
        inclination_deg: 87.0,
        apid: Apid::new(bindings::SPACECOMP_WILDFIRE_APID as u16).unwrap(),
        rto_ms: 1000,
        router_send_topic: 0,
        router_recv_topic: 0,
    };

    let app = WildfireCompute {
        thresholds: FireThresholds {
            t4_abs: 330.0,
            ..Default::default()
        },
        aoi: GeoBounds {
            west: -122.5,
            south: 38.0,
            east: -121.5,
            north: 39.0,
        },
    };

    let node: SpaceCompNode = SpaceCompNode::builder()
        .config(config)
        .store(NoStore)
        .reachable(AlwaysReachable)
        .build();
    node.start(&app);
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
