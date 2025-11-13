use crate::isl_router::SatAddr;

#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lon_min: f64,
    pub lon_max: f64,
}

impl BoundingBox {
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.lat_min && lat <= self.lat_max && lon >= self.lon_min && lon <= self.lon_max
    }
}

#[derive(Debug, Clone)]
pub struct Position {
    pub addr: SatAddr,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

#[derive(Debug, Clone)]
pub enum Packet {
    Forward {
        dest: SatAddr,
        msg: Msg,
        hops: usize,
    },
    Broadcast {
        origin: SatAddr,
        ttl: usize,
        msg: Msg,
    },
}

#[derive(Debug, Clone)]
pub enum Msg {
    Uplink {
        req_id: u64,
        bbox: BoundingBox,
    },
    Downlink {
        req_id: u64,
        data: String,
    },
    Collect {
        bbox: BoundingBox,
        proxy: SatAddr,
        reply_to: SatAddr,
    },
    CollectAck {
        from: SatAddr,
        data_id: String,
    },
    BidReq {
        size: usize,
        reply_to: SatAddr,
    },
    BidAck {
        from: SatAddr,
        score: f64,
    },
    Map {
        data_id: String,
        reply_to: SatAddr,
    },
    MapAck {
        from: SatAddr,
        result: String,
    },
}

#[derive(Debug, Clone)]
pub struct CollectResult {
    pub sat: SatAddr,
    pub data_id: String,
}

#[derive(Debug, Clone)]
pub struct Bid {
    pub sat: SatAddr,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub struct MapResult {
    pub sat: SatAddr,
    pub result: String,
}
