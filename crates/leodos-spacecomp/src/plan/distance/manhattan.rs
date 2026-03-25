//! Manhattan distance cost model.
//!
//! Computes cost as the number of hops (Manhattan distance) on the torus,
//! multiplied by a configurable per-hop cost.

use leodos_protocols::network::isl::torus::{Point, Torus};

use super::CostModel;

/// Manhattan distance cost model using per-hop cost on the torus.
#[derive(Debug, Clone, Copy)]
pub struct ManhattanCost {
    /// Cost multiplier per hop.
    pub hop_cost: u32,
}

impl Default for ManhattanCost {
    fn default() -> Self {
        Self { hop_cost: 1 }
    }
}

impl CostModel for ManhattanCost {
    type Cost = u32;

    fn cost(&self, torus: &Torus, task: Point, processor: Point) -> u32 {
        let dx = torus.distance_sat(task, processor).min(torus.distance_sat(processor, task));
        let dy = torus.distance_orb(task, processor).min(torus.distance_orb(processor, task));
        (dx + dy) as u32 * self.hop_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_point() {
        let torus = Torus::new(4, 4);
        let cost_model = ManhattanCost::default();
        let p = Point::new(1, 2);
        assert_eq!(cost_model.cost(&torus, p, p), 0);
    }

    #[test]
    fn test_adjacent() {
        let torus = Torus::new(4, 4);
        let cost_model = ManhattanCost { hop_cost: 10 };
        let a = Point::new(1, 1);
        let b = Point::new(1, 2);
        assert_eq!(cost_model.cost(&torus, a, b), 10);
    }

    #[test]
    fn test_wraparound() {
        let torus = Torus::new(4, 4);
        let cost_model = ManhattanCost::default();
        let a = Point::new(0, 0);
        let b = Point::new(0, 3);
        assert_eq!(cost_model.cost(&torus, a, b), 1);
    }
}
