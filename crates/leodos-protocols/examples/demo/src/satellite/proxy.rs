use std::collections::HashMap;
use tokio::time::{Duration, sleep};

use crate::isl_router::SatAddr;
use crate::messages::*;

use super::Satellite;

pub struct State {
    collects: HashMap<SatAddr, Vec<CollectResult>>,
    bids: HashMap<SatAddr, Vec<Bid>>,
    maps: HashMap<SatAddr, Vec<MapResult>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            collects: HashMap::new(),
            bids: HashMap::new(),
            maps: HashMap::new(),
        }
    }
}

pub fn on_collect_ack(addr: SatAddr, from: SatAddr, data_id: String, state: &mut State) {
    println!("[Proxy {}] Got collect from {}", addr, from);
    state
        .collects
        .entry(addr)
        .or_default()
        .push(CollectResult { sat: from, data_id });
}

pub fn on_bid_ack(addr: SatAddr, from: SatAddr, score: f64, state: &mut State) {
    state
        .bids
        .entry(addr)
        .or_default()
        .push(Bid { sat: from, score });
}

pub fn on_map_ack(addr: SatAddr, from: SatAddr, result: String, state: &mut State) {
    println!("[Proxy {}] Got map result from {}", addr, from);
    state
        .maps
        .entry(addr)
        .or_default()
        .push(MapResult { sat: from, result });
}

pub async fn become_proxy(sat: &Satellite, req_id: u64, bbox: BoundingBox, state: &mut State) {
    let addr = sat.addr;
    let (lat, lon) = sat.pos().await;
    println!("[Sat {}] Becoming proxy at ({:.1}, {:.1})", addr, lat, lon);

    let collected = collect_phase(sat, addr, &bbox, state).await;
    let mappers = map_phase(sat, addr, &collected, state).await;
    reduce_phase(sat, req_id, addr, &mappers).await;
}

async fn collect_phase(
    sat: &Satellite,
    addr: SatAddr,
    bbox: &BoundingBox,
    state: &mut State,
) -> Vec<CollectResult> {
    println!("[Proxy {}] PHASE 1: COLLECT", addr);
    state.collects.insert(addr, Vec::new());

    sat.broadcast(
        Msg::Collect {
            bbox: bbox.clone(),
            proxy: addr,
            reply_to: addr,
        },
        10,
    )
    .await;
    sleep(Duration::from_millis(500)).await;

    let collected = state.collects.remove(&addr).unwrap_or_default();
    println!("[Proxy {}] Collected {} responses", addr, collected.len());
    collected
}

async fn map_phase(
    sat: &Satellite,
    addr: SatAddr,
    collected: &[CollectResult],
    state: &mut State,
) -> Vec<MapResult> {
    println!("[Proxy {}] PHASE 2: MAP", addr);
    state.bids.insert(addr, Vec::new());

    sat.broadcast(
        Msg::BidReq {
            size: 100,
            reply_to: addr,
        },
        10,
    )
    .await;
    sleep(Duration::from_millis(300)).await;

    let mut bids = state.bids.remove(&addr).unwrap_or_default();
    bids.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    let mappers: Vec<_> = bids.iter().take(collected.len().max(1)).collect();
    println!("[Proxy {}] Selected {} mappers", addr, mappers.len());

    state.maps.insert(addr, Vec::new());
    for (mapper, data) in mappers.iter().zip(collected.iter()) {
        println!("[Proxy {}] Assigning {} as mapper", addr, mapper.sat);
        sat.send(
            mapper.sat,
            Msg::Map {
                data_id: data.data_id.clone(),
                reply_to: addr,
            },
        )
        .await;
    }
    sleep(Duration::from_millis(500)).await;

    state.maps.remove(&addr).unwrap_or_default()
}

async fn reduce_phase(sat: &Satellite, req_id: u64, addr: SatAddr, results: &[MapResult]) {
    println!(
        "[Proxy {}] PHASE 3: REDUCE ({} results)",
        addr,
        results.len()
    );

    let combined: Vec<_> = results.iter().map(|r| r.result.clone()).collect();
    let final_data = format!("FINAL[{}]", combined.join(", "));

    if let Some(ref gtx) = sat.ground {
        let _ = gtx
            .send(Packet::Forward {
                dest: addr,
                msg: Msg::Downlink {
                    req_id,
                    data: final_data,
                },
                hops: 0,
            })
            .await;
    }
}
