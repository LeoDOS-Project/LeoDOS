#![no_std]

use leodos_libcfs::error::CfsError;
use leodos_libcfs::info;
use leodos_libcfs::nos3::drivers::thermal_cam::ThermalCamera;
use leodos_libcfs::runtime::Runtime;

use leodos_analysis::geo::GeoBounds;
use leodos_analysis::thermal::detect_fire;
use leodos_analysis::thermal::FireThresholds;
use leodos_analysis::thermal::Hotspot;

use leodos_spacecomp::bufwriter::BufWriter;
use leodos_spacecomp::packet::OpCode;
use leodos_spacecomp::packet::SpaceCompMessage;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::spp::Apid;

use leodos_spacecomp::node::RxHandle;
use leodos_spacecomp::node::SpaceComp;
use leodos_spacecomp::node::TxHandle;
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
const NUM_SATS: u8 = bindings::SPACECOMP_WILDFIRE_NUM_SATS as u8;

type Camera = ThermalCamera<MAX_PIXELS>;

// ── Wire types ──────────────────────────────────────────────

/// A hotspot record sent from mapper to reducer.
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

// ── SpaceComp implementation ────────────────────────────────

struct WildfireCompute {
    thresholds: FireThresholds,
    aoi: GeoBounds,
}

impl SpaceComp for WildfireCompute {
    /// Captures a thermal frame and sends pixel data to mapper.
    async fn collect(
        &self,
        tx: &mut TxHandle<'_>,
        job_id: u16,
        mapper_addr: Address,
        _partition_id: u8,
    ) -> Result<(), SpaceCompError> {
        let mut camera = Camera::builder()
            .device(c"spi_3")
            .chip_select_line(3)
            .baudrate(1_000_000)
            .build()?;
        let frame = camera.capture().await
            .map_err(|_| SpaceCompError::Cfs(CfsError::ExternalResourceFail))?;

        // Send raw pixel data in chunks to mapper
        let pixel_bytes = frame.mwir.as_bytes();
        let mut buf = [0u8; 512];
        for chunk in pixel_bytes.chunks(256) {
            let m = SpaceCompMessage::builder()
                .buffer(&mut buf)
                .op_code(OpCode::DataChunk)
                .job_id(job_id)
                .payload_len(chunk.len())
                .build()?;
            m.payload_mut().copy_from_slice(chunk);
            tx.send(mapper_addr, m.as_bytes()).await.ok();
        }
        info!("Collector: sent {} bytes of pixel data", pixel_bytes.len())?;
        Ok(())
    }

    /// Runs fire detection on received pixel data.
    async fn map(
        &self,
        rx: &mut RxHandle<'_>,
        tx: &mut TxHandle<'_>,
        job_id: u16,
        reducer_addr: Address,
        collector_count: u8,
    ) -> Result<(), SpaceCompError> {
        let mut buf = [0u8; 512];
        let mut received = 0u8;
        {
            let mut writer = BufWriter::<HotspotRecord, _>::new(
                tx, &mut buf, reducer_addr, job_id, OpCode::DataChunk,
            );

            loop {
                let mut pixel_buf = [0u8; 256];
                let Ok(maybe_len) = rx
                    .recv_with(|data| -> Option<usize> {
                        let msg = SpaceCompMessage::parse(data).ok()?;
                        if msg.op_code() != Ok(OpCode::DataChunk) {
                            return None;
                        }
                        let n = msg.payload().len().min(pixel_buf.len());
                        pixel_buf[..n].copy_from_slice(&msg.payload()[..n]);
                        Some(n)
                    })
                    .await
                else {
                    return Ok(());
                };
                let Some(_len) = maybe_len else { continue };

                // In a full implementation, pixel_buf contains raw
                // thermal pixels. We'd reconstruct mwir/lwir arrays
                // and run detect_fire. For now, demonstrate the flow:
                let mut hotspots = [Hotspot::default(); MAX_HOTSPOTS];
                let det = detect_fire(
                    &[0.0f32; 64], // placeholder
                    &[0.0f32; 64],
                    8,
                    8,
                    &self.thresholds,
                    &mut hotspots,
                );

                for hs in det.hotspots {
                    writer.write(&HotspotRecord::from_hotspot(hs)).await?;
                }
                writer.flush().await?;

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

    /// Merges hotspot detections from multiple mappers.
    async fn reduce(
        &self,
        rx: &mut RxHandle<'_>,
        tx: &mut TxHandle<'_>,
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
                    for rec in &all_hotspots[..total] {
                        let t = rec.temp();
                        if t > max_temp {
                            max_temp = t;
                        }
                    }

                    info!(
                        "Reduced: {} hotspots from {} mappers, max {} K",
                        total, mapper_count, max_temp,
                    )?;

                    let mut writer = BufWriter::<HotspotRecord, _>::new(
                        tx, &mut buf, los_addr, job_id, OpCode::JobResult,
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

        SpaceCompNode::builder()
            .config(config)
            .store(leodos_protocols::transport::srspp::dtn::NoStore)
            .reachable(leodos_protocols::transport::srspp::dtn::AlwaysReachable)
            .build()
            .run(&app)
            .await
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
