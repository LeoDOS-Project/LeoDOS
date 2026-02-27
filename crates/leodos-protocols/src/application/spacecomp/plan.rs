//! SpaceCoMP job planning (Section II of the paper).
//!
//! Given a job request and orbital shell configuration, produces a complete
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

impl Job {
    /// Plans this job, producing collector/mapper assignments and
    /// reducer placement. `N` is the maximum number of satellites that can
    /// participate (compile-time bound for heapless allocation).
    pub fn plan<const N: usize>(
        &self,
        shell: Shell,
        reducer_placement: ReducerPlacement,
        los_node: Point,
    ) -> Result<JobPlan<N>, &'static str> {
        let projection = Projection::new(shell);

        let all_covering: Vec<Point, N> = projection.satellites_in_geo_aoi(&self.geo_aoi);
        if all_covering.is_empty() {
            return Err("no satellites cover the AOI");
        }

        let collectors = if self.ascending_only {
            filter_ascending(shell, &all_covering)
        } else {
            all_covering
        };
        if collectors.is_empty() {
            return Err("no satellites match direction constraint");
        }

        let grid_aoi = projection
            .geo_to_grid_aoi(&self.geo_aoi)
            .ok_or("failed to compute grid AOI")?;

        let mappers = select_mappers::<N>(shell, &grid_aoi);
        if mappers.len() < collectors.len() {
            return Err("not enough mapper nodes for collectors");
        }

        let assignment = solve_assignment::<N>(shell, &collectors, &mappers, self)?;

        let reducer = match reducer_placement {
            ReducerPlacement::LineOfSight => los_node,
            ReducerPlacement::CenterOfAoi => grid_aoi.center(&shell.torus),
        };

        let params = JobParams {
            map_processing_factor: self.map_processing_factor,
            reduce_processing_factor: self.reduce_processing_factor,
            map_reduction_factor: self.map_reduction_factor,
            reduce_reduction_factor: self.reduce_reduction_factor,
            data_volume_bytes: self.data_volume_bytes,
            ..Default::default()
        };
        let estimated_cost = JobCost::total_cost(
            &shell.torus,
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
}

fn filter_ascending<const N: usize>(shell: Shell, nodes: &Vec<Point, N>) -> Vec<Point, N> {
    let half = shell.torus.num_orbs / 2;
    let mut result = Vec::new();
    for &node in nodes {
        if node.orb < half {
            let _ = result.push(node);
        }
    }
    result
}

fn select_mappers<const N: usize>(shell: Shell, grid_aoi: &Aoi) -> Vec<Point, N> {
    let mut mappers = Vec::new();
    for x in 0..shell.torus.num_sats {
        for y in 0..shell.torus.num_orbs {
            let point = Point::new(y, x);
            if grid_aoi.contains(&shell.torus, point) {
                if mappers.push(point).is_err() {
                    return mappers;
                }
            }
        }
    }
    mappers
}

fn solve_assignment<const N: usize>(
    shell: Shell,
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
            cost_matrix[i][j] = cost_model.cost(&shell.torus, collector, mapper);
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

        let job = Job::builder()
            .geo_aoi(GeoAoi::new(LatLon::new(10.0, -5.0), LatLon::new(-10.0, 5.0)))
            .data_volume_bytes(1_000_000)
            .build();

        let result = job.plan::<64>(shell, ReducerPlacement::CenterOfAoi, Point::new(0, 0));
        assert!(result.is_ok(), "plan failed: {:?}", result.err());

        let p = result.unwrap();
        assert!(!p.collectors.is_empty());
        assert!(p.mappers.len() >= p.collectors.len());
        assert_eq!(p.assignment.len(), p.collectors.len());
    }

    #[test]
    fn test_center_reducer_cheaper_than_los() {
        let shell = test_shell();

        let job = Job::builder()
            .geo_aoi(GeoAoi::new(LatLon::new(10.0, -5.0), LatLon::new(-10.0, 5.0)))
            .data_volume_bytes(1_000_000)
            .build();

        let los = Point::new(0, 36);

        let center_plan = job.plan::<64>(shell, ReducerPlacement::CenterOfAoi, los).unwrap();
        let los_plan = job.plan::<64>(shell, ReducerPlacement::LineOfSight, los).unwrap();

        assert!(
            center_plan.estimated_cost <= los_plan.estimated_cost,
            "center {} should be <= LOS {}",
            center_plan.estimated_cost,
            los_plan.estimated_cost
        );
    }
}
