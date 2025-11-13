use tokio::time::{Duration, sleep};

use crate::isl_router::SatAddr;
use crate::messages::*;

use super::Satellite;

pub async fn collect(sat: &Satellite, bbox: &BoundingBox, reply_to: SatAddr) {
    let (lat, lon) = sat.pos().await;
    if !bbox.contains(lat, lon) {
        return;
    }

    println!("[Sat {}] Collecting at ({:.1}, {:.1})", sat.addr, lat, lon);
    sleep(Duration::from_millis(50)).await;

    sat.send(
        reply_to,
        Msg::CollectAck {
            from: sat.addr,
            data_id: format!("data_{}", sat.addr),
        },
    )
    .await;
}

pub async fn bid(sat: &Satellite, reply_to: SatAddr) {
    let score = (sat.addr.sat * 10 + sat.addr.orb) as f64 * 0.7;
    sat.send(
        reply_to,
        Msg::BidAck {
            from: sat.addr,
            score,
        },
    )
    .await;
}

pub async fn map(sat: &Satellite, data_id: &str, reply_to: SatAddr) {
    let (lat, lon) = sat.pos().await;
    println!(
        "[Sat {}] Mapping {} at ({:.1}, {:.1})",
        sat.addr, data_id, lat, lon
    );

    sleep(Duration::from_millis(100)).await;

    sat.send(
        reply_to,
        Msg::MapAck {
            from: sat.addr,
            result: format!("mapped_{}", data_id),
        },
    )
    .await;
}
