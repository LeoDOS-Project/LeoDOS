//! Hungarian algorithm (Kuhn-Munkres) for the linear sum assignment problem.
//!
//! Given n tasks and n nodes with a cost matrix where cost[i][j] is the cost of
//! assigning task i to node j, find the assignment that minimizes total cost. Each
//! task must be assigned to exactly one node and each node handles exactly one
//! task.
//!
//! Time complexity: O(n³)
//! Space complexity: O(n) working arrays + O(n²) input matrix
//!
//! The algorithm maintains dual variables called potentials: u[i] for tasks and v[j] for
//! nodes. These satisfy u[i] + v[j] ≤ cost[i][j] for all pairs. The reduced cost of
//! a pair is cost[i][j] - u[i] - v[j], which is always ≥ 0. A pair is called tight when
//! its reduced cost is zero.
//!
//! For each unassigned task, the algorithm searches for an augmenting path through tight
//! pairs. The slack for node j is the minimum reduced cost to reach it from any
//! visited task. When no augmenting path exists, potentials are adjusted by the minimum
//! slack (delta) to create new tight pairs and continue the search.

use core::ops::{Add, Sub};

use heapless::Vec;

use super::solver::{Bounded, Solver};

/// Hungarian algorithm solver with pre-allocated working arrays.
pub struct Hungarian<C, const N: usize> {
    /// Task potentials. u[i] + v[j] ≤ cost[i][j] for all pairs; equality means "tight".
    u: [C; N],
    /// Processor potentials. Together with u, defines which pairs are tight.
    v: [C; N],
    /// Current assignment: node_for_task[i] = node assigned to task i, or UNMATCHED.
    node_for_task: [usize; N],
    /// Inverse assignment: task_for_node[j] = task assigned to node j, or UNMATCHED.
    task_for_node: [usize; N],
    /// Slack: minimum reduced cost to reach node j from any visited task.
    slack: [C; N],
    /// Which task achieved the minimum slack for node j.
    slack_task: [usize; N],
    /// Whether task i has been visited in current augmentation search.
    visited_task: [bool; N],
    /// Whether node j has been visited in current augmentation search.
    visited_node: [bool; N],
}

const UNMATCHED: usize = usize::MAX;

impl<C, const N: usize> Hungarian<C, N>
where
    C: Copy + Default + Bounded,
{
    /// Creates a new solver with zeroed working arrays.
    pub fn new() -> Self {
        Self {
            u: [C::default(); N],
            v: [C::default(); N],
            node_for_task: [UNMATCHED; N],
            task_for_node: [UNMATCHED; N],
            slack: [C::MAX; N],
            slack_task: [0; N],
            visited_task: [false; N],
            visited_node: [false; N],
        }
    }

    /// Clears all task-to-node and node-to-task assignments.
    fn reset_matching(&mut self, n: usize) {
        for i in 0..n {
            self.node_for_task[i] = UNMATCHED;
            self.task_for_node[i] = UNMATCHED;
        }
    }

    /// Resets slack values to MAX and clears visited flags for a new augmentation.
    fn reset_augment_state(&mut self, n: usize) {
        for i in 0..n {
            self.slack[i] = C::MAX;
            self.visited_task[i] = false;
            self.visited_node[i] = false;
        }
    }
}

impl<C, const N: usize> Hungarian<C, N>
where
    C: Copy + Default + Ord + Add<Output = C> + Sub<Output = C> + Bounded,
{
    /// Initializes potentials: u[i] = min cost for task i, v[j] = min reduced cost for node j.
    fn init_potentials(&mut self, cost: &[[C; N]; N], n: usize) {
        for i in 0..n {
            self.u[i] = (0..n).map(|j| cost[i][j]).min().unwrap_or(C::default());
        }
        for j in 0..n {
            self.v[j] = (0..n)
                .map(|i| cost[i][j] - self.u[i])
                .min()
                .unwrap_or(C::default());
        }
    }

    /// Returns the reduced cost for pair (task, node): cost - u[task] - v[node].
    fn reduced_cost(&self, cost: &[[C; N]; N], task: usize, node: usize) -> C {
        cost[task][node] - self.u[task] - self.v[node]
    }

    /// Updates slack for each node based on reduced costs from the given task.
    fn update_slack_from_task(&mut self, cost: &[[C; N]; N], task: usize, n: usize) {
        for node in 0..n {
            let reduced = self.reduced_cost(cost, task, node);
            if reduced < self.slack[node] {
                self.slack[node] = reduced;
                self.slack_task[node] = task;
            }
        }
    }

    /// Returns (delta, node) for the unvisited node with minimum slack.
    fn find_min_slack_node(&self, n: usize) -> (C, usize) {
        let mut delta = C::MAX;
        let mut best_node = 0;
        for node in 0..n {
            if !self.visited_node[node] && self.slack[node] < delta {
                delta = self.slack[node];
                best_node = node;
            }
        }
        (delta, best_node)
    }

    /// Applies delta to potentials: increases u for visited tasks, decreases v for visited nodes.
    fn update_potentials(&mut self, delta: C, source_task: usize, n: usize) {
        for node in 0..n {
            if self.visited_node[node] {
                let task = self.task_for_node[node];
                self.u[task] = self.u[task] + delta;
                self.v[node] = self.v[node] - delta;
            } else {
                self.slack[node] = self.slack[node] - delta;
            }
        }
        self.u[source_task] = self.u[source_task] + delta;
    }

    /// Traces back from sink node to source task, flipping assignments along the path.
    fn augment_path(&mut self, sink_node: usize, source_task: usize) {
        let mut node = sink_node;
        loop {
            let task = self.slack_task[node];
            let prev_node = self.node_for_task[task];
            self.node_for_task[task] = node;
            self.task_for_node[node] = task;
            if task == source_task {
                break;
            }
            node = prev_node;
        }
    }
}

impl<C, const N: usize> Default for Hungarian<C, N>
where
    C: Copy + Default + Bounded,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C, const N: usize> Solver<C, N> for Hungarian<C, N>
where
    C: Copy + Default + Ord + Add<Output = C> + Sub<Output = C> + Bounded,
{
    fn solve(&mut self, cost: &[[C; N]; N], n: usize) -> Vec<usize, N> {
        self.reset_matching(n);
        self.init_potentials(cost, n);

        for task in 0..n {
            self.reset_augment_state(n);
            self.update_slack_from_task(cost, task, n);

            loop {
                let (delta, node) = self.find_min_slack_node(n);
                self.update_potentials(delta, task, n);

                self.visited_node[node] = true;
                self.visited_task[self.slack_task[node]] = true;

                if self.task_for_node[node] == UNMATCHED {
                    self.augment_path(node, task);
                    break;
                }

                self.update_slack_from_task(cost, self.task_for_node[node], n);
            }
        }

        let mut result: Vec<usize, N> = Vec::new();
        for i in 0..n {
            let _ = result.push(self.node_for_task[i]);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_3x3() {
        let cost: [[u32; 4]; 4] = [[10, 5, 13, 0], [3, 9, 18, 0], [18, 7, 2, 0], [0, 0, 0, 0]];

        let mut solver = Hungarian::<u32, 4>::new();
        let result = solver.solve(&cost, 3);

        assert_eq!(result.len(), 3);
        let total: u32 = result.iter().enumerate().map(|(i, &j)| cost[i][j]).sum();
        assert_eq!(total, 5 + 3 + 2);
    }

    #[test]
    fn test_diagonal_optimal() {
        let cost: [[u32; 4]; 4] = [[0, 1, 1, 0], [1, 0, 1, 0], [1, 1, 0, 0], [0, 0, 0, 0]];

        let mut solver = Hungarian::<u32, 4>::new();
        let result = solver.solve(&cost, 3);

        let total: u32 = result.iter().enumerate().map(|(i, &j)| cost[i][j]).sum();
        assert_eq!(total, 0);
    }

    #[test]
    fn test_2x2() {
        let cost: [[u32; 4]; 4] = [[1, 2, 0, 0], [3, 4, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]];

        let mut solver = Hungarian::<u32, 4>::new();
        let result = solver.solve(&cost, 2);

        assert_eq!(result.len(), 2);
        let total: u32 = result.iter().enumerate().map(|(i, &j)| cost[i][j]).sum();
        assert_eq!(total, 1 + 4);
    }
}
