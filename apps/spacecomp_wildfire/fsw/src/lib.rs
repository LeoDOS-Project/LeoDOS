#![no_std]

use leodos_libcfs::error::CfsError;
use leodos_libcfs::info;
use leodos_libcfs::nos3::drivers::thermal_cam::ThermalCamera;
use leodos_libcfs::runtime::Runtime;

use leodos_analysis::geo::GeoBounds;
use leodos_analysis::thermal::detect_fire;
use leodos_analysis::thermal::FireThresholds;
use leodos_analysis::thermal::Hotspot;

use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;

use leodos_spacecomp::bufwriter::BufWriter;
use leodos_spacecomp::node::SpaceComp;
use leodos_spacecomp::packet::OpCode;
use leodos_spacecomp::packet::SpaceCompMessage;
use leodos_spacecomp::transport::Rx;
use leodos_spacecomp::transport::Tx;
use leodos_spacecomp::SpaceCompConfig;
use leodos_spacecomp::SpaceCompError;
use leodos_spacecomp::SpaceCompNode;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U16;
use zerocopy::network_endian::U32;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const MAX_PIXELS: usize = 512 * 512;
const MAX_HOTSPOTS: usize = 64;
const CHUNK_SIZE: usize = 256;
const NUM_SATS: u8 = bindings::SPACECOMP_WILDFIRE_NUM_SATS as u8;

type Camera = ThermalCamera<MAX_PIXELS>;

// ── Wire types ──────────────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
struct FrameHeader {
    width: U16,
    height: U16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
struct HotspotRecord {
    x: U16,
    y: U16,
    t4: U32,
}

impl HotspotRecord {
    fn from_hotspot(h: &Hotspot) -> Self {
        Self {
            x: U16::new(h.x),
            y: U16::new(h.y),
            t4: U32::new(h.t4.to_bits()),
        }
    }

    fn temp(&self) -> f32 {
        f32::from_bits(self.t4.get())
    }
}

// ── Helpers ─────────────────────────────────────────────────

async fn send_chunks(
    tx: &mut impl Tx,
    buf: &mut [u8],
    data: &[u8],
    job_id: u16,
    target: Address,
) -> Result<(), SpaceCompError> {
    for chunk in data.chunks(CHUNK_SIZE) {
        let m = SpaceCompMessage::builder()
            .buffer(buf)
            .op_code(OpCode::DataChunk)
            .job_id(job_id)
            .payload_len(chunk.len())
            .build()?;
        m.payload_mut().copy_from_slice(chunk);
        tx.send(target, m.as_bytes()).await.ok();
    }
    Ok(())
}

async fn recv_pixels(
    rx: &mut impl Rx,
    dst: &mut [f32],
    n_pixels: usize,
) -> Result<(), SpaceCompError> {
    let mut offset = 0;
    let total_bytes = n_pixels * 4;
    while offset < total_bytes {
        let Ok(maybe_len) = rx
            .recv_with(|data| -> Option<usize> {
                let msg = SpaceCompMessage::parse(data).ok()?;
                if msg.op_code() != Ok(OpCode::DataChunk) {
                    return None;
                }
                let payload = msg.payload();
                let bytes = dst.as_mut_bytes();
                let n = payload.len().min(bytes.len() - offset);
                bytes[offset..offset + n].copy_from_slice(&payload[..n]);
                Some(n)
            })
            .await
        else {
            return Ok(());
        };
        let Some(n) = maybe_len else { continue };
        offset += n;
    }
    Ok(())
}

// ── SpaceComp implementation ────────────────────────────────

struct WildfireCompute {
    thresholds: FireThresholds,
    aoi: GeoBounds,
}

impl SpaceComp for WildfireCompute {
    async fn collect(
        &self,
        mut tx: impl Tx,
        job_id: u16,
        mapper_addr: Address,
        _partition_id: u8,
    ) -> Result<(), SpaceCompError> {
        let mut camera = Camera::builder()
            .device(c"spi_3")
            .chip_select_line(3)
            .baudrate(1_000_000)
            .build()?;
        let frame = camera
            .capture()
            .await
            .map_err(|_| SpaceCompError::Cfs(CfsError::ExternalResourceFail))?;

        let mut buf = [0u8; 512];

        // Send frame header
        let header = FrameHeader {
            width: U16::new(frame.width as u16),
            height: U16::new(frame.height as u16),
        };
        let m = SpaceCompMessage::builder()
            .buffer(&mut buf)
            .op_code(OpCode::DataChunk)
            .job_id(job_id)
            .payload_len(header.as_bytes().len())
            .build()?;
        m.payload_mut().copy_from_slice(header.as_bytes());
        tx.send(mapper_addr, m.as_bytes()).await.ok();

        // Send MWIR + LWIR pixels
        send_chunks(&mut tx, &mut buf, frame.mwir.as_bytes(), job_id, mapper_addr).await?;
        send_chunks(&mut tx, &mut buf, frame.lwir.as_bytes(), job_id, mapper_addr).await?;

        info!(
            "Collector: sent {}x{} frame ({} bytes)",
            frame.width,
            frame.height,
            frame.mwir.as_bytes().len() * 2,
        )?;
        Ok(())
    }

    async fn map(
        &self,
        mut rx: impl Rx,
        mut tx: impl Tx,
        job_id: u16,
        reducer_addr: Address,
        collector_count: u8,
    ) -> Result<(), SpaceCompError> {
        let mut buf = [0u8; 512];
        let mut received = 0u8;
        {
            let mut writer = BufWriter::<HotspotRecord, _>::new(
                &mut tx, &mut buf, reducer_addr, job_id, OpCode::DataChunk,
            );

            loop {
                // Receive frame header
                let Ok(maybe_hdr) = rx
                    .recv_with(|data| -> Option<FrameHeader> {
                        let msg = SpaceCompMessage::parse(data).ok()?;
                        if msg.op_code() != Ok(OpCode::DataChunk) {
                            return None;
                        }
                        FrameHeader::read_from_bytes(msg.payload()).ok()
                    })
                    .await
                else {
                    return Ok(());
                };
                let Some(hdr) = maybe_hdr else { continue };
                let w = hdr.width.get() as usize;
                let h = hdr.height.get() as usize;
                let n_pixels = w * h;

                // Receive MWIR + LWIR pixels
                let mut mwir = [0.0f32; MAX_PIXELS];
                recv_pixels(&mut rx, &mut mwir, n_pixels).await?;
                let mut lwir = [0.0f32; MAX_PIXELS];
                recv_pixels(&mut rx, &mut lwir, n_pixels).await?;

                // Run fire detection
                let mut hotspots = [Hotspot::default(); MAX_HOTSPOTS];
                let det = detect_fire(&mwir[..n_pixels], &lwir[..n_pixels], w, h, &self.thresholds, &mut hotspots);

                for hs in det.hotspots {
                    writer.write(&HotspotRecord::from_hotspot(hs)).await?;
                }
                writer.flush().await?;

                info!("Mapper: detected {} hotspots in {}x{} frame", det.count, w, h)?;

                received += 1;
                if received >= collector_count {
                    break;
                }
            }
        }

        let done = SpaceCompMessage::builder()
            .buffer(&mut buf)
            .op_code(OpCode::PhaseDone)
            .job_id(job_id)
            .payload_len(0)
            .build()?;
        tx.send(reducer_addr, done.as_bytes()).await.ok();
        Ok(())
    }

    async fn reduce(
        &self,
        mut rx: impl Rx,
        mut tx: impl Tx,
        job_id: u16,
        los_addr: Address,
        mapper_count: u8,
    ) -> Result<(), SpaceCompError> {
        let mut buf = [0u8; 512];
        let mut all_hotspots = [HotspotRecord {
            x: U16::new(0),
            y: U16::new(0),
            t4: U32::new(0),
        }; MAX_HOTSPOTS];
        let mut total = 0usize;
        let mut done_count = 0u8;

        loop {
            let Ok(op) = rx
                .recv_with(|data| {
                    let Ok(msg) = SpaceCompMessage::parse(data) else {
                        return None;
                    };
                    match msg.op_code() {
                        Ok(OpCode::DataChunk) => {
                            for rec in msg.records::<HotspotRecord>() {
                                if total < MAX_HOTSPOTS {
                                    all_hotspots[total] = *rec;
                                    total += 1;
                                }
                            }
                            None
                        }
                        Ok(op) => Some(op),
                        _ => None,
                    }
                })
                .await
            else {
                return Ok(());
            };

            if op == Some(OpCode::PhaseDone) {
                done_count += 1;
                if done_count >= mapper_count {
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
                        "Reduced: {} hotspots from {} mappers, max {} K at ({},{})",
                        total, mapper_count, max_temp, centroid.lat, centroid.lon,
                    )?;

                    let mut writer = BufWriter::<HotspotRecord, _>::new(
                        &mut tx, &mut buf, los_addr, job_id, OpCode::JobResult,
                    );
                    for rec in &all_hotspots[..total] {
                        writer.write(rec).await?;
                    }
                    writer.flush().await?;
                    return Ok(());
                }
            }
        }
    }
}

// ── Entry point ─────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn SPACECOMP_WILDFIRE_AppMain() {
    Runtime::new().run(async {
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
        node.run(&app).await
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
