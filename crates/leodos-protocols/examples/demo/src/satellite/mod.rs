mod proxy;
mod worker;

use std::f64::consts::PI;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{Duration, sleep};

use crate::isl_router::{Direction, IslConfig, IslRouter, SatAddr};
use crate::messages::*;

const EARTH_RADIUS: f64 = 6371.0;

#[derive(Clone)]
pub struct Orbit {
    pub alt: f64,
    pub inc: f64,
    pub raan: f64,
    pub anomaly: f64,
}

impl Orbit {
    fn period(&self) -> f64 {
        let a = EARTH_RADIUS + self.alt;
        2.0 * PI * (a.powi(3) / 398600.4418).sqrt()
    }

    pub fn position(&self, t: f64) -> (f64, f64) {
        let n = 2.0 * PI / self.period();
        let v = (self.anomaly.to_radians() + n * t) % (2.0 * PI);

        let inc = self.inc.to_radians();
        let raan = self.raan.to_radians();

        let x = v.cos() * raan.cos() - v.sin() * inc.cos() * raan.sin();
        let y = v.cos() * raan.sin() + v.sin() * inc.cos() * raan.cos();
        let z = v.sin() * inc.sin();

        (z.asin().to_degrees(), y.atan2(x).to_degrees())
    }
}

pub struct Links {
    pub north: Option<mpsc::Sender<Packet>>,
    pub south: Option<mpsc::Sender<Packet>>,
    pub east: Option<mpsc::Sender<Packet>>,
    pub west: Option<mpsc::Sender<Packet>>,
}

impl Links {
    pub fn get(&self, dir: Direction) -> Option<&mpsc::Sender<Packet>> {
        match dir {
            Direction::North => self.north.as_ref(),
            Direction::South => self.south.as_ref(),
            Direction::East => self.east.as_ref(),
            Direction::West => self.west.as_ref(),
        }
    }

    pub fn all(&self) -> impl Iterator<Item = &mpsc::Sender<Packet>> {
        [&self.north, &self.south, &self.east, &self.west]
            .into_iter()
            .flatten()
    }
}

pub struct Satellite {
    pub addr: SatAddr,
    orbit: Orbit,
    pos: Arc<RwLock<Position>>,
    router: IslRouter,
    links: Arc<RwLock<Links>>,
    ground: Option<mpsc::Sender<Packet>>,
}

impl Satellite {
    pub fn new(
        addr: SatAddr,
        orbit: Orbit,
        cfg: IslConfig,
        links: Arc<RwLock<Links>>,
        ground: Option<mpsc::Sender<Packet>>,
    ) -> Self {
        let (lat, lon) = orbit.position(0.0);
        Self {
            addr,
            pos: Arc::new(RwLock::new(Position {
                addr,
                lat,
                lon,
                alt: orbit.alt,
            })),
            orbit,
            router: IslRouter::new(cfg),
            links,
            ground,
        }
    }

    pub async fn run(self, mut rx: mpsc::Receiver<Packet>) {
        self.start_orbit_updater();
        println!("[Sat {}] Online at {:.0} km", self.addr, self.orbit.alt);

        let mut proxy_state = proxy::State::new();

        while let Some(pkt) = rx.recv().await {
            match pkt {
                Packet::Forward { dest, msg, hops } => {
                    if dest == self.addr {
                        self.handle(msg, &mut proxy_state).await;
                    } else {
                        self.forward(dest, msg, hops).await;
                    }
                }
                Packet::Broadcast { origin, ttl, msg } => {
                    if ttl == 0 {
                        continue;
                    }
                    self.handle(msg.clone(), &mut proxy_state).await;
                    self.rebroadcast(origin, ttl - 1, msg).await;
                }
            }
        }
    }

    fn start_orbit_updater(&self) {
        let pos = self.pos.clone();
        let orbit = self.orbit.clone();
        tokio::spawn(async move {
            let mut t = 0.0f64;
            loop {
                sleep(Duration::from_millis(100)).await;
                t += 10.0;
                let (lat, lon) = orbit.position(t);
                let mut p = pos.write().await;
                p.lat = lat;
                p.lon = lon;
            }
        });
    }

    async fn handle(&self, msg: Msg, proxy_state: &mut proxy::State) {
        match msg {
            Msg::Uplink { req_id, bbox } => {
                proxy::become_proxy(self, req_id, bbox, proxy_state).await;
            }
            Msg::Collect { bbox, reply_to, .. } => {
                worker::collect(self, &bbox, reply_to).await;
            }
            Msg::CollectAck { from, data_id } => {
                proxy::on_collect_ack(self.addr, from, data_id, proxy_state);
            }
            Msg::BidReq { reply_to, .. } => {
                worker::bid(self, reply_to).await;
            }
            Msg::BidAck { from, score } => {
                proxy::on_bid_ack(self.addr, from, score, proxy_state);
            }
            Msg::Map { data_id, reply_to } => {
                worker::map(self, &data_id, reply_to).await;
            }
            Msg::MapAck { from, result } => {
                proxy::on_map_ack(self.addr, from, result, proxy_state);
            }
            Msg::Downlink { data, req_id } => {
                self.downlink(req_id, data).await;
            }
        }
    }

    pub async fn pos(&self) -> (f64, f64) {
        let p = self.pos.read().await;
        (p.lat, p.lon)
    }

    pub async fn send(&self, dest: SatAddr, msg: Msg) {
        if let Some(dir) = self.router.next_hop(self.addr, dest) {
            let lk = self.links.read().await;
            if let Some(tx) = lk.get(dir) {
                let _ = tx.send(Packet::Forward { dest, msg, hops: 0 }).await;
            }
        }
    }

    pub async fn broadcast(&self, msg: Msg, ttl: usize) {
        let lk = self.links.read().await;
        for tx in lk.all() {
            let _ = tx
                .send(Packet::Broadcast {
                    origin: self.addr,
                    ttl,
                    msg: msg.clone(),
                })
                .await;
        }
    }

    async fn forward(&self, dest: SatAddr, msg: Msg, hops: usize) {
        if let Some(dir) = self.router.next_hop(self.addr, dest) {
            let lk = self.links.read().await;
            if let Some(tx) = lk.get(dir) {
                println!(
                    "[Sat {}] Forward to {} via {} (hop {})",
                    self.addr,
                    dest,
                    dir,
                    hops + 1
                );
                let _ = tx
                    .send(Packet::Forward {
                        dest,
                        msg,
                        hops: hops + 1,
                    })
                    .await;
            }
        }
    }

    async fn rebroadcast(&self, origin: SatAddr, ttl: usize, msg: Msg) {
        let lk = self.links.read().await;
        for tx in lk.all() {
            let _ = tx
                .send(Packet::Broadcast {
                    origin,
                    ttl,
                    msg: msg.clone(),
                })
                .await;
        }
    }

    async fn downlink(&self, req_id: u64, data: String) {
        println!("[Sat {}] Downlink: {}", self.addr, data);
        if let Some(ref gtx) = self.ground {
            let _ = gtx
                .send(Packet::Forward {
                    dest: self.addr,
                    msg: Msg::Downlink { req_id, data },
                    hops: 0,
                })
                .await;
        }
    }
}
