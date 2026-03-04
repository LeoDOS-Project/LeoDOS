//! End-to-end MapReduce job cost calculation.
//!
//! Computes total job cost combining map and reduce phases (Equations 15-17):
//!
//! ```text
//! C = C_m + C_r
//!
//! C_m = Σ (m_p × K + h_i × t_h + S(d_col→map, V))
//!
//! C_r = r_p × K + S(V/F_R, d_red→los) + Σ S(V/F_M, d_map→red)
//! ```

use crate::application::spacecomp::job::Job;
use crate::network::isl::torus::{Point, Torus};

const HOP_OVERHEAD_US: u64 = 100;
const BASE_PROCESSING_US: u64 = 1000;

fn hop_cost(hops: u32) -> u64 {
    hops as u64 * HOP_OVERHEAD_US
}

/// Returns the Manhattan distance (minimum hops) between two points on the torus.
pub fn hop_distance(torus: &Torus, from: Point, to: Point) -> u32 {
    let dx = Torus::distance(from.sat, to.sat, torus.num_sats)
        .min(Torus::distance(to.sat, from.sat, torus.num_sats));
    let dy = Torus::distance(from.orb, to.orb, torus.num_orbs)
        .min(Torus::distance(to.orb, from.orb, torus.num_orbs));
    (dx + dy) as u32
}

impl Job {
    /// Computes the total map-phase cost across all collector-mapper pairs.
    pub fn map_cost(
        &self,
        torus: &Torus,
        collectors: &[Point],
        mappers: &[Point],
        assignment: &[usize],
    ) -> u64 {
        let processing = (self.map_processing_factor() * BASE_PROCESSING_US as f32) as u64;
        collectors
            .iter()
            .enumerate()
            .map(|(i, &collector)| {
                let mapper = mappers[assignment[i]];
                let hops = hop_distance(torus, collector, mapper);
                processing.saturating_add(hop_cost(hops))
            })
            .fold(0u64, |acc, cost| acc.saturating_add(cost))
    }

    /// Computes the reduce-phase cost including aggregation and result delivery.
    pub fn reduce_cost(
        &self,
        torus: &Torus,
        mappers: &[Point],
        reducer: Point,
        los: Point,
    ) -> u64 {
        let processing = (self.reduce_processing_factor() * BASE_PROCESSING_US as f32) as u64;

        let reducer_to_los = hop_cost(hop_distance(torus, reducer, los));

        let aggregation: u64 = mappers
            .iter()
            .map(|&m| hop_cost(hop_distance(torus, m, reducer)))
            .sum();

        processing
            .saturating_add(reducer_to_los)
            .saturating_add(aggregation)
    }

    /// Computes the total job cost as the sum of map and reduce costs.
    pub fn estimated_cost(
        &self,
        torus: &Torus,
        collectors: &[Point],
        mappers: &[Point],
        assignment: &[usize],
        reducer: Point,
        los: Point,
    ) -> u64 {
        let map = self.map_cost(torus, collectors, mappers, assignment);
        let reduce = self.reduce_cost(torus, mappers, reducer, los);
        map.saturating_add(reduce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::isl::geo::{GeoAoi, LatLon};

    fn test_job() -> Job {
        Job::builder()
            .geo_aoi(GeoAoi::new(LatLon::new(10.0, -5.0), LatLon::new(-10.0, 5.0)))
            .data_volume_bytes(1_000_000)
            .build()
    }

    #[test]
    fn test_map_cost_same_location() {
        let torus = Torus::new(8, 8);
        let job = test_job();
        let collectors = [Point::new(1, 1), Point::new(2, 2)];
        let mappers = [Point::new(1, 1), Point::new(2, 2)];
        let assignment = [0, 1];

        let cost = job.map_cost(&torus, &collectors, &mappers, &assignment);
        let expected = 2 * BASE_PROCESSING_US;
        assert_eq!(cost, expected);
    }

    #[test]
    fn test_map_cost_with_hops() {
        let torus = Torus::new(8, 8);
        let job = test_job();
        let collectors = [Point::new(0, 0)];
        let mappers = [Point::new(2, 2)];
        let assignment = [0];

        let cost = job.map_cost(&torus, &collectors, &mappers, &assignment);
        let expected = BASE_PROCESSING_US + 4 * HOP_OVERHEAD_US;
        assert_eq!(cost, expected);
    }

    #[test]
    fn test_reduce_cost() {
        let torus = Torus::new(8, 8);
        let job = test_job();
        let mappers = [Point::new(1, 1), Point::new(3, 3)];
        let reducer = Point::new(2, 2);
        let los = Point::new(0, 0);

        let cost = job.reduce_cost(&torus, &mappers, reducer, los);
        let processing = BASE_PROCESSING_US;
        let reducer_to_los = 4 * HOP_OVERHEAD_US;
        let m1_to_reducer = 2 * HOP_OVERHEAD_US;
        let m2_to_reducer = 2 * HOP_OVERHEAD_US;
        assert_eq!(cost, processing + reducer_to_los + m1_to_reducer + m2_to_reducer);
    }

    #[test]
    fn test_center_reducer_lower_aggregation() {
        let torus = Torus::new(16, 16);
        let mappers = [Point::new(4, 4), Point::new(4, 8), Point::new(8, 4), Point::new(8, 8)];

        let center = Point::new(6, 6);
        let corner = Point::new(0, 0);

        let agg_center: u32 = mappers
            .iter()
            .map(|&m| hop_distance(&torus, m, center))
            .sum();
        let agg_corner: u32 = mappers
            .iter()
            .map(|&m| hop_distance(&torus, m, corner))
            .sum();

        assert!(agg_center < agg_corner, "center {} < corner {}", agg_center, agg_corner);
    }
}
