//! SpaceCoMP job coordinator (Section II of the paper).
//!
//! The coordinator runs on the LOS (Line-of-Sight) node and orchestrates the
//! three processing phases. Given a job request, it produces a complete
//! execution plan: which satellites collect, which map, where to reduce, and
//! the optimal collector-to-mapper assignment via bipartite matching.

use heapless::Vec;

use crate::application::spacecomp::job::Job;
use crate::application::spacecomp::scheduler::aoi::Aoi;
use crate::application::spacecomp::scheduler::{
    CostModel, Hungarian, JobCost, JobParams, Solver, SpaceCompCost,
};
use crate::network::isl::projection::Projection;
use crate::network::isl::shell::Shell;
use crate::network::isl::torus::Point;

/// Strategy for placing the reducer satellite in a MapReduce job.
#[derive(Debug, Clone, Copy)]
pub enum ReducerPlacement {
    /// Place the reducer at the line-of-sight (ground station) node.
    LineOfSight,
    /// Place the reducer at the center of the area of interest.
    CenterOfAoi,
}

/// The result of planning a SpaceCoMP job.
#[derive(Debug, Clone)]
pub struct JobPlan<const N: usize> {
    /// Satellite positions assigned as data collectors.
    pub collectors: Vec<Point, N>,
    /// Satellite positions assigned as data mappers.
    pub mappers: Vec<Point, N>,
    /// Collector-to-mapper assignment: `assignment[i]` is the mapper index for collector `i`.
    pub assignment: Vec<usize, N>,
    /// Position of the reducer satellite.
    pub reducer: Point,
    /// Grid-space bounding box of the area of interest.
    pub grid_aoi: Aoi,
    /// Estimated total job cost in microseconds.
    pub estimated_cost: u64,
}

/// Orchestrates SpaceCoMP MapReduce jobs from the LOS node.
pub struct Coordinator {
    shell: Shell,
    reducer_placement: ReducerPlacement,
}

impl Coordinator {
    /// Creates a new coordinator for the given orbital shell and reducer strategy.
    pub fn new(shell: Shell, reducer_placement: ReducerPlacement) -> Self {
        Self { shell, reducer_placement }
    }

    /// Plans a SpaceCoMP job, producing collector/mapper assignments and
    /// reducer placement. `N` is the maximum number of satellites that can
    /// participate (compile-time bound for heapless allocation).
    pub fn plan<const N: usize>(
        &self,
        job: &Job,
        los_node: Point,
    ) -> Result<JobPlan<N>, &'static str> {
        let projection = Projection::new(self.shell);

        // Step 1: identify collectors (satellites covering the AOI)
        let all_covering: Vec<Point, N> = projection.satellites_in_geo_aoi(&job.geo_aoi);
        if all_covering.is_empty() {
            return Err("no satellites cover the AOI");
        }

        // Step 2: filter by ascending/descending constraint
        let collectors = if job.ascending_only {
            self.filter_ascending(&all_covering)
        } else {
            all_covering
        };
        if collectors.is_empty() {
            return Err("no satellites match direction constraint");
        }

        // Step 3: compute grid AOI from covering satellites
        let grid_aoi = projection
            .geo_to_grid_aoi(&job.geo_aoi)
            .ok_or("failed to compute grid AOI")?;

        // Step 4: select mappers (all AOI satellites, at least as many as collectors)
        let mappers = self.select_mappers::<N>(&grid_aoi);
        if mappers.len() < collectors.len() {
            return Err("not enough mapper nodes for collectors");
        }

        // Step 5: bipartite matching (collector→mapper assignment)
        let assignment = self.solve_assignment::<N>(&collectors, &mappers, job)?;

        // Step 6: place reducer
        let reducer = match self.reducer_placement {
            ReducerPlacement::LineOfSight => los_node,
            ReducerPlacement::CenterOfAoi => grid_aoi.center(&self.shell.torus),
        };

        // Step 7: estimate total cost
        let params = JobParams {
            map_processing_factor: job.map_processing_factor,
            reduce_processing_factor: job.reduce_processing_factor,
            map_reduction_factor: job.map_reduction_factor,
            reduce_reduction_factor: job.reduce_reduction_factor,
            data_volume_bytes: job.data_volume_bytes,
            ..Default::default()
        };
        let estimated_cost = JobCost::total_cost(
            &self.shell.torus,
            &params,
            collectors.as_slice(),
            mappers.as_slice(),
            assignment.as_slice(),
            reducer,
            los_node,
        );

        Ok(JobPlan {
            collectors,
            mappers,
            assignment,
            reducer,
            grid_aoi,
            estimated_cost,
        })
    }

    fn filter_ascending<const N: usize>(&self, nodes: &Vec<Point, N>) -> Vec<Point, N> {
        let half = self.shell.torus.num_orbs / 2;
        let mut result = Vec::new();
        for &node in nodes {
            if node.orb < half {
                let _ = result.push(node);
            }
        }
        result
    }

    fn select_mappers<const N: usize>(&self, grid_aoi: &Aoi) -> Vec<Point, N> {
        let mut mappers = Vec::new();
        for x in 0..self.shell.torus.num_sats {
            for y in 0..self.shell.torus.num_orbs {
                let point = Point::new(y, x);
                if grid_aoi.contains(&self.shell.torus, point) {
                    if mappers.push(point).is_err() {
                        return mappers;
                    }
                }
            }
        }
        mappers
    }

    fn solve_assignment<const N: usize>(
        &self,
        collectors: &Vec<Point, N>,
        mappers: &Vec<Point, N>,
        job: &Job,
    ) -> Result<Vec<usize, N>, &'static str> {
        const MAX: usize = 64;

        if collectors.len() > MAX || mappers.len() > MAX {
            return Err("problem size exceeds solver capacity");
        }

        let cost_model = SpaceCompCost {
            data_volume_bytes: job.data_volume_bytes,
            ..Default::default()
        };

        let mut cost_matrix = [[0u64; MAX]; MAX];
        for (i, &collector) in collectors.iter().enumerate() {
            for (j, &mapper) in mappers.iter().enumerate() {
                cost_matrix[i][j] = cost_model.cost(&self.shell.torus, collector, mapper);
            }
        }

        let mut solver = Hungarian::<u64, MAX>::new();
        let raw = solver.solve(&cost_matrix, collectors.len());

        let mut result = Vec::new();
        for &idx in &raw {
            let _ = result.push(idx);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::isl::geo::{GeoAoi, LatLon};
    use crate::network::isl::torus::Torus;

    fn test_shell() -> Shell {
        let torus = Torus::new(20, 72);
        Shell::new(torus, 550_000.0, 87.0)
    }

    #[test]
    fn test_plan_produces_valid_assignment() {
        let shell = test_shell();
        let coord = Coordinator::new(shell, ReducerPlacement::CenterOfAoi);

        let job = Job::builder()
            .geo_aoi(GeoAoi::new(LatLon::new(10.0, -5.0), LatLon::new(-10.0, 5.0)))
            .data_volume_bytes(1_000_000)
            .build();

        let plan = coord.plan::<64>(&job, Point::new(0, 0));
        assert!(plan.is_ok(), "plan failed: {:?}", plan.err());

        let plan = plan.unwrap();
        assert!(!plan.collectors.is_empty());
        assert!(plan.mappers.len() >= plan.collectors.len());
        assert_eq!(plan.assignment.len(), plan.collectors.len());
    }

    #[test]
    fn test_center_reducer_cheaper_than_los() {
        let shell = test_shell();

        let job = Job::builder()
            .geo_aoi(GeoAoi::new(LatLon::new(10.0, -5.0), LatLon::new(-10.0, 5.0)))
            .data_volume_bytes(1_000_000)
            .build();

        let los = Point::new(0, 36);

        let center_coord = Coordinator::new(shell, ReducerPlacement::CenterOfAoi);
        let los_coord = Coordinator::new(shell, ReducerPlacement::LineOfSight);

        let center_plan = center_coord.plan::<64>(&job, los).unwrap();
        let los_plan = los_coord.plan::<64>(&job, los).unwrap();

        assert!(
            center_plan.estimated_cost <= los_plan.estimated_cost,
            "center {} should be <= LOS {}",
            center_plan.estimated_cost,
            los_plan.estimated_cost
        );
    }
}
