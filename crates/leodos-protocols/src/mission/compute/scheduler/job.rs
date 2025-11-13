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

use crate::network::isl::torus::{Point, Torus};

#[derive(Debug, Clone, Copy)]
pub struct JobParams {
    pub map_processing_factor: f32,
    pub reduce_processing_factor: f32,
    pub hop_overhead_us: u64,
    pub map_reduction_factor: f32,
    pub reduce_reduction_factor: f32,
    pub data_volume_bytes: u64,
    pub base_processing_us: u64,
}

impl Default for JobParams {
    fn default() -> Self {
        Self {
            map_processing_factor: 1.0,
            reduce_processing_factor: 1.0,
            hop_overhead_us: 100,
            map_reduction_factor: 1.0,
            reduce_reduction_factor: 5.0,
            data_volume_bytes: 10_000_000_000,
            base_processing_us: 1000,
        }
    }
}

impl JobParams {
    fn hop_cost(&self, hops: u32) -> u64 {
        hops as u64 * self.hop_overhead_us
    }

    fn map_processing_cost(&self) -> u64 {
        (self.map_processing_factor * self.base_processing_us as f32) as u64
    }

    fn reduce_processing_cost(&self) -> u64 {
        (self.reduce_processing_factor * self.base_processing_us as f32) as u64
    }
}

pub struct JobCost;

impl JobCost {
    pub fn map_cost(
        torus: &Torus,
        params: &JobParams,
        collectors: &[Point],
        mappers: &[Point],
        assignment: &[usize],
    ) -> u64 {
        collectors
            .iter()
            .enumerate()
            .map(|(i, &collector)| {
                let mapper = mappers[assignment[i]];
                let hops = Self::hop_distance(torus, collector, mapper);
                params.map_processing_cost().saturating_add(params.hop_cost(hops))
            })
            .fold(0u64, |acc, cost| acc.saturating_add(cost))
    }

    pub fn reduce_cost(
        torus: &Torus,
        params: &JobParams,
        mappers: &[Point],
        reducer: Point,
        los: Point,
    ) -> u64 {
        let reducer_to_los_cost = params.hop_cost(Self::hop_distance(torus, reducer, los));

        let aggregation_cost: u64 = mappers
            .iter()
            .map(|&m| params.hop_cost(Self::hop_distance(torus, m, reducer)))
            .sum();

        params
            .reduce_processing_cost()
            .saturating_add(reducer_to_los_cost)
            .saturating_add(aggregation_cost)
    }

    pub fn total_cost(
        torus: &Torus,
        params: &JobParams,
        collectors: &[Point],
        mappers: &[Point],
        assignment: &[usize],
        reducer: Point,
        los: Point,
    ) -> u64 {
        let map = Self::map_cost(torus, params, collectors, mappers, assignment);
        let reduce = Self::reduce_cost(torus, params, mappers, reducer, los);
        map.saturating_add(reduce)
    }

    pub fn hop_distance(torus: &Torus, from: Point, to: Point) -> u32 {
        let dx = Torus::distance(from.x, to.x, torus.num_cols)
            .min(Torus::distance(to.x, from.x, torus.num_cols));
        let dy = Torus::distance(from.y, to.y, torus.num_rows)
            .min(Torus::distance(to.y, from.y, torus.num_rows));
        (dx + dy) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_cost_same_location() {
        let torus = Torus::new(8, 8);
        let params = JobParams::default();
        let collectors = [Point::new(1, 1), Point::new(2, 2)];
        let mappers = [Point::new(1, 1), Point::new(2, 2)];
        let assignment = [0, 1];

        let cost = JobCost::map_cost(&torus, &params, &collectors, &mappers, &assignment);
        let expected = 2 * params.base_processing_us;
        assert_eq!(cost, expected);
    }

    #[test]
    fn test_map_cost_with_hops() {
        let torus = Torus::new(8, 8);
        let params = JobParams {
            hop_overhead_us: 100,
            base_processing_us: 1000,
            ..Default::default()
        };
        let collectors = [Point::new(0, 0)];
        let mappers = [Point::new(2, 2)];
        let assignment = [0];

        let cost = JobCost::map_cost(&torus, &params, &collectors, &mappers, &assignment);
        let expected = 1000 + 4 * 100;
        assert_eq!(cost, expected);
    }

    #[test]
    fn test_reduce_cost() {
        let torus = Torus::new(8, 8);
        let params = JobParams {
            hop_overhead_us: 100,
            base_processing_us: 1000,
            ..Default::default()
        };
        let mappers = [Point::new(1, 1), Point::new(3, 3)];
        let reducer = Point::new(2, 2);
        let los = Point::new(0, 0);

        let cost = JobCost::reduce_cost(&torus, &params, &mappers, reducer, los);
        let processing = 1000;
        let reducer_to_los = 4 * 100;
        let m1_to_reducer = 2 * 100;
        let m2_to_reducer = 2 * 100;
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
            .map(|&m| JobCost::hop_distance(&torus, m, center))
            .sum();
        let agg_corner: u32 = mappers
            .iter()
            .map(|&m| JobCost::hop_distance(&torus, m, corner))
            .sum();

        assert!(agg_center < agg_corner, "center {} < corner {}", agg_center, agg_corner);
    }
}
