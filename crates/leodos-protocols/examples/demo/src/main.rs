#![allow(unused)]

mod ground_station;
mod isl_router;
mod mcp_host;
mod messages;
mod satellite;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{Duration, sleep};

use ground_station::GroundStation;
use isl_router::{IslConfig, SatAddr};
use messages::Packet;
use satellite::{Links, Orbit, Satellite};

struct Constellation {
    channels: HashMap<SatAddr, mpsc::Sender<Packet>>,
    receivers: HashMap<SatAddr, mpsc::Receiver<Packet>>,
    links: HashMap<SatAddr, Arc<RwLock<Links>>>,
    cfg: IslConfig,
}

impl Constellation {
    fn new(cfg: IslConfig) -> Self {
        let mut channels = HashMap::new();
        let mut receivers = HashMap::new();
        let mut links = HashMap::new();

        for orb in 1..=cfg.max_orb {
            for sat in 1..=cfg.max_sat {
                let addr = SatAddr::new(sat, orb);
                let (tx, rx) = mpsc::channel(64);
                channels.insert(addr, tx);
                receivers.insert(addr, rx);
                links.insert(
                    addr,
                    Arc::new(RwLock::new(Links {
                        north: None,
                        south: None,
                        east: None,
                        west: None,
                    })),
                );
            }
        }

        Self {
            channels,
            receivers,
            links,
            cfg,
        }
    }

    async fn connect_links(&mut self) {
        let cfg = &self.cfg;
        for orb in 1..=cfg.max_orb {
            for sat in 1..=cfg.max_sat {
                let addr = SatAddr::new(sat, orb);

                let n_orb = if orb == 1 { cfg.max_orb } else { orb - 1 };
                let s_orb = if orb == cfg.max_orb { 1 } else { orb + 1 };
                let w_sat = if sat == 1 { cfg.max_sat } else { sat - 1 };
                let e_sat = if sat == cfg.max_sat { 1 } else { sat + 1 };

                let mut lk = self.links.get(&addr).unwrap().write().await;
                lk.north = self.channels.get(&SatAddr::new(sat, n_orb)).cloned();
                lk.south = self.channels.get(&SatAddr::new(sat, s_orb)).cloned();
                lk.west = self.channels.get(&SatAddr::new(w_sat, orb)).cloned();
                lk.east = self.channels.get(&SatAddr::new(e_sat, orb)).cloned();
            }
        }
    }

    fn spawn_satellites(&mut self, gateway: SatAddr, ground_tx: mpsc::Sender<Packet>) {
        let cfg = &self.cfg;
        for orb in 1..=cfg.max_orb {
            for sat in 1..=cfg.max_sat {
                let addr = SatAddr::new(sat, orb);

                let orbit = Orbit {
                    alt: 550.0,
                    inc: 53.0,
                    raan: ((orb - 1) as f64) * (180.0 / cfg.max_orb as f64),
                    anomaly: ((sat - 1) as f64) * (360.0 / cfg.max_sat as f64),
                };

                let sat_cfg = IslConfig {
                    max_sat: cfg.max_sat,
                    max_orb: cfg.max_orb,
                    ..Default::default()
                };

                let rx = self.receivers.remove(&addr).unwrap();
                let links = self.links.get(&addr).unwrap().clone();
                let ground = if addr == gateway {
                    Some(ground_tx.clone())
                } else {
                    None
                };

                let satellite = Satellite::new(addr, orbit, sat_cfg, links, ground);
                tokio::spawn(satellite.run(rx));
            }
        }
    }

    fn get_tx(&self, addr: SatAddr) -> mpsc::Sender<Packet> {
        self.channels.get(&addr).unwrap().clone()
    }
}

#[tokio::main]
async fn main() {
    println!("=== LEO Satellite Demo with ISL Routing ===\n");

    let cfg = IslConfig {
        max_sat: 22,
        max_orb: 72,
        radius: 6371.0,
        alt: 550.0,
        optimize: true,
        crossover: false,
    };

    let mut constellation = Constellation::new(cfg);
    constellation.connect_links().await;

    let gateway = SatAddr::new(1, 1);
    let (downlink_tx, downlink_rx) = mpsc::channel(32);

    constellation.spawn_satellites(gateway, downlink_tx);

    let gs = Arc::new(GroundStation::new(gateway, constellation.get_tx(gateway)));

    let gs_clone = gs.clone();
    tokio::spawn(async move { gs_clone.handle_downlink(downlink_rx).await });

    sleep(Duration::from_millis(500)).await;

    println!("\n--- MCP Host Request ---\n");
    mcp_host::run(gs).await;

    sleep(Duration::from_millis(100)).await;
    println!("\n=== Done ===");
}
