use std::sync::Arc;

use crate::ground_station::GroundStation;
use crate::messages::BoundingBox;

pub async fn run(gs: Arc<GroundStation>) {
    println!("[MCP Host] Query requires LEO satellite data");

    let bbox = BoundingBox {
        lat_min: -60.0,
        lat_max: 60.0,
        lon_min: -180.0,
        lon_max: 180.0,
    };

    println!(
        "[MCP Host] Requesting bbox: lat [{:.0}, {:.0}], lon [{:.0}, {:.0}]",
        bbox.lat_min, bbox.lat_max, bbox.lon_min, bbox.lon_max
    );

    match gs.request(bbox).await {
        Some(result) => println!("[MCP Host] Result: {}", result),
        None => println!("[MCP Host] Request failed"),
    }
}
