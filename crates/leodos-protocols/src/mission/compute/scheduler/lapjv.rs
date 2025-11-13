//! Jonker-Volgenant algorithm (LAPJV) for the linear sum assignment problem.
//!
//! Given n tasks and n nodes with a cost matrix where cost[i][j] is the cost of
//! assigning task i to node j, find the assignment that minimizes total cost. Each
//! task must be assigned to exactly one node and each node handles exactly one
//! task.
//!
//! Time complexity: O(n³) worst case, but typically faster than Hungarian in practice.
//! Space complexity: O(n) working arrays + O(n²) input matrix.
//!
//! The algorithm maintains node prices v[j]. The reduced cost of pair (i,j) is
//! cost[i][j] - v[j]. For each unassigned task, it finds the shortest path to an unassigned
//! node through the reduced cost graph using a Dijkstra-like search, then augments
//! along that path. After augmentation, prices are updated to maintain non-negative reduced
//! costs.
//!
//! Reference: R. Jonker and A. Volgenant, "A Shortest Augmenting Path Algorithm for
//! Dense and Sparse Linear Assignment Problems", Computing 38, 325-340 (1987).

use core::ops::{Add, Sub};

use heapless::Vec;

use super::solver::{Bounded, Solver};

const UNASSIGNED: usize = usize::MAX;

/// Jonker-Volgenant (LAPJV) solver with pre-allocated working arrays.
pub struct JonkerVolgenant<C, const N: usize> {
    /// Processor prices. Reduced cost of pair (task, node) is cost[task][node] - v[node].
    v: [C; N],
    /// Current assignment: node_for_task[i] = node assigned to task i, or UNASSIGNED.
    node_for_task: [usize; N],
    /// Inverse assignment: task_for_node[j] = task assigned to node j, or UNASSIGNED.
    task_for_node: [usize; N],
    /// Shortest path distance from source task to node j in the reduced cost graph.
    dist: [C; N],
    /// Predecessor: pred_task[j] = the task from which we reached node j on shortest path.
    pred_task: [usize; N],
    /// Whether node j has been permanently labeled (settled) in Dijkstra search.
    scanned: [bool; N],
}

impl<C, const N: usize> JonkerVolgenant<C, N>
where
    C: Copy + Default + Bounded,
{
    pub fn new() -> Self {
        Self {
            v: [C::default(); N],
            node_for_task: [UNASSIGNED; N],
            task_for_node: [UNASSIGNED; N],
            dist: [C::MAX; N],
            pred_task: [UNASSIGNED; N],
            scanned: [false; N],
        }
    }

    /// Clears all task-to-node and node-to-task assignments.
    fn reset_matching(&mut self, n: usize) {
        for i in 0..n {
            self.node_for_task[i] = UNASSIGNED;
            self.task_for_node[i] = UNASSIGNED;
        }
    }

    /// Resets dist to MAX, pred_task to UNASSIGNED, and scanned to false for a new search.
    fn reset_search_state(&mut self, n: usize) {
        for node in 0..n {
            self.dist[node] = C::MAX;
            self.pred_task[node] = UNASSIGNED;
            self.scanned[node] = false;
        }
    }
}

impl<C, const N: usize> JonkerVolgenant<C, N>
where
    C: Copy + Default + Ord + Add<Output = C> + Sub<Output = C> + Bounded,
{
    /// Initializes node prices v[j] as the minimum cost to assign any task to node j.
    fn init_node_prices(&mut self, cost: &[[C; N]; N], n: usize) {
        for node in 0..n {
            self.v[node] = (0..n)
                .map(|task| cost[task][node])
                .min()
                .unwrap_or(C::default());
        }
    }

    /// Returns the reduced cost for pair (task, node): cost - v[node].
    fn reduced_cost(&self, cost: &[[C; N]; N], task: usize, node: usize) -> C {
        cost[task][node] - self.v[node]
    }

    /// Initializes distances from source task using reduced costs.
    fn init_distances_from_task(&mut self, cost: &[[C; N]; N], task: usize, n: usize) {
        for node in 0..n {
            self.dist[node] = self.reduced_cost(cost, task, node);
            self.pred_task[node] = task;
        }
    }

    /// Returns Some((distance, node)) for the unscanned node with minimum distance.
    fn find_min_unscanned_node(&self, n: usize) -> Option<(C, usize)> {
        let mut min_dist = C::MAX;
        let mut best_node = UNASSIGNED;
        for node in 0..n {
            if !self.scanned[node] && self.dist[node] < min_dist {
                min_dist = self.dist[node];
                best_node = node;
            }
        }
        if best_node == UNASSIGNED {
            None
        } else {
            Some((min_dist, best_node))
        }
    }

    /// Relaxes edges from a task, updating dist[node] if a shorter path is found.
    fn relax_edges_from_task(&mut self, cost: &[[C; N]; N], task: usize, base_dist: C, n: usize) {
        for node in 0..n {
            if self.scanned[node] {
                continue;
            }
            let new_dist = base_dist + self.reduced_cost(cost, task, node);
            if new_dist < self.dist[node] {
                self.dist[node] = new_dist;
                self.pred_task[node] = task;
            }
        }
    }

    /// Updates node prices for scanned nodes to maintain reduced cost invariants.
    fn update_node_prices(&mut self, min_dist: C, sink: usize, n: usize) {
        for node in 0..n {
            if self.scanned[node] {
                self.v[node] = self.v[node] + self.dist[node] - min_dist;
            }
        }
        if sink != UNASSIGNED {
            self.v[sink] = self.v[sink] + self.dist[sink] - min_dist;
        }
    }

    /// Traces back from sink node to source task, flipping assignments along the path.
    fn augment_from_sink(&mut self, sink_node: usize, source_task: usize) {
        let mut node = sink_node;
        loop {
            let task = self.pred_task[node];
            self.task_for_node[node] = task;
            let prev_node = self.node_for_task[task];
            self.node_for_task[task] = node;
            if task == source_task {
                break;
            }
            node = prev_node;
        }
    }
}

impl<C, const N: usize> Default for JonkerVolgenant<C, N>
where
    C: Copy + Default + Bounded,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C, const N: usize> Solver<C, N> for JonkerVolgenant<C, N>
where
    C: Copy + Default + Ord + Add<Output = C> + Sub<Output = C> + Bounded,
{
    fn solve(&mut self, cost: &[[C; N]; N], n: usize) -> Vec<usize, N> {
        self.reset_matching(n);
        self.init_node_prices(cost, n);

        for task in 0..n {
            self.reset_search_state(n);
            self.init_distances_from_task(cost, task, n);

            let mut sink = UNASSIGNED;
            let mut min_dist = C::default();

            loop {
                let Some((dist, node)) = self.find_min_unscanned_node(n) else {
                    break;
                };

                self.scanned[node] = true;
                min_dist = dist;

                if self.task_for_node[node] == UNASSIGNED {
                    sink = node;
                    break;
                }

                self.relax_edges_from_task(cost, self.task_for_node[node], min_dist, n);
            }

            self.update_node_prices(min_dist, sink, n);

            if sink != UNASSIGNED {
                self.augment_from_sink(sink, task);
            }
        }

        let mut result: Vec<usize, N> = Vec::new();
        for task in 0..n {
            let _ = result.push(self.node_for_task[task]);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_3x3() {
        let cost: [[u32; 4]; 4] = [
            [10, 5, 13, 0],
            [3, 9, 18, 0],
            [18, 7, 2, 0],
            [0, 0, 0, 0],
        ];

        let mut solver = JonkerVolgenant::<u32, 4>::new();
        let result = solver.solve(&cost, 3);

        assert_eq!(result.len(), 3);
        let total: u32 = result.iter().enumerate().map(|(i, &j)| cost[i][j]).sum();
        assert_eq!(total, 5 + 3 + 2);
    }

    #[test]
    fn test_diagonal_optimal() {
        let cost: [[u32; 4]; 4] = [
            [0, 1, 1, 0],
            [1, 0, 1, 0],
            [1, 1, 0, 0],
            [0, 0, 0, 0],
        ];

        let mut solver = JonkerVolgenant::<u32, 4>::new();
        let result = solver.solve(&cost, 3);

        let total: u32 = result.iter().enumerate().map(|(i, &j)| cost[i][j]).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn test_2x2() {
        let cost: [[u32; 4]; 4] = [
            [1, 2, 0, 0],
            [3, 4, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
        ];

        let mut solver = JonkerVolgenant::<u32, 4>::new();
        let result = solver.solve(&cost, 2);

        assert_eq!(result.len(), 2);
        let total: u32 = result.iter().enumerate().map(|(i, &j)| cost[i][j]).sum();
        assert_eq!(total, 1 + 4);
    }

    #[test]
    fn test_matches_hungarian() {
        let cost: [[u32; 8]; 8] = [
            [7, 2, 1, 9, 4, 0, 0, 0],
            [9, 6, 9, 5, 5, 0, 0, 0],
            [3, 8, 3, 1, 8, 0, 0, 0],
            [7, 9, 4, 2, 2, 0, 0, 0],
            [8, 4, 7, 4, 8, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0],
        ];

        let mut jv = JonkerVolgenant::<u32, 8>::new();
        let result_jv = jv.solve(&cost, 5);

        let mut hungarian = super::super::hungarian::Hungarian::<u32, 8>::new();
        let result_h = hungarian.solve(&cost, 5);

        let total_jv: u32 = result_jv.iter().enumerate().map(|(i, &j)| cost[i][j]).sum();
        let total_h: u32 = result_h.iter().enumerate().map(|(i, &j)| cost[i][j]).sum();

        assert_eq!(total_jv, total_h, "JV={} vs Hungarian={}", total_jv, total_h);
    }
}
