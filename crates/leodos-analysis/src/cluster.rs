//! Spatial clustering via union-find.
//!
//! Groups 2D points into clusters based on 8-connectivity
//! (Chebyshev distance <= 1). Each point carries a scalar
//! value for aggregation.

use heapless::Vec;

/// A computed cluster with aggregated statistics.
#[derive(Debug, Copy, Clone)]
pub struct Cluster {
    pub centroid_x: f32,
    pub centroid_y: f32,
    pub max_value: f32,
    pub count: u16,
}

/// Incrementally clusters 2D points by adjacency.
///
/// Points added via `add()` are automatically merged with
/// any existing adjacent points (8-connected). Call
/// `clusters()` to iterate the final clusters.
pub struct SpatialClusterer<const N: usize> {
    uf: UnionFind<N>,
    points: Vec<(f32, f32, f32), N>,
    radius: f32,
}

/// Errors that can occur when adding points to the clusterer.
pub enum ClusterError {
    Full,
}

impl<const N: usize> SpatialClusterer<N> {
    /// Creates a clusterer that merges points within `radius` units.
    pub fn new(radius: f32) -> Self {
        Self {
            uf: UnionFind::new(),
            points: Vec::new(),
            radius,
        }
    }

    pub fn add(&mut self, x: f32, y: f32, value: f32) -> Result<(), ClusterError> {
        let idx = self.points.len();
        if self.points.push((x, y, value)).is_err() {
            return Err(ClusterError::Full);
        }

        let r2 = self.radius * self.radius;
        for j in 0..idx {
            let dx = x - self.points[j].0;
            let dy = y - self.points[j].1;
            if dx * dx + dy * dy <= r2 {
                self.uf.union(idx as u16, j as u16);
            }
        }
        Ok(())
    }

    /// Iterates over clusters.
    pub fn clusters(&mut self) -> impl Iterator<Item = Cluster> + '_ {
        let mut accum = [Accum::EMPTY; N];
        for i in 0..self.points.len() {
            let root = self.uf.find(i as u16) as usize;
            let (x, y, v) = self.points[i];
            let a = &mut accum[root];
            a.sum_x += x as f32;
            a.sum_y += y as f32;
            if v > a.max_value {
                a.max_value = v;
            }
            a.count += 1;
        }
        accum.into_iter().filter(|a| a.count > 0).map(|a| {
            let n = a.count as f32;
            Cluster {
                centroid_x: a.sum_x / n,
                centroid_y: a.sum_y / n,
                max_value: a.max_value,
                count: a.count,
            }
        })
    }
}

#[derive(Copy, Clone)]
struct Accum {
    sum_x: f32,
    sum_y: f32,
    max_value: f32,
    count: u16,
}

impl Accum {
    const EMPTY: Self = Self {
        sum_x: 0.0,
        sum_y: 0.0,
        max_value: 0.0,
        count: 0,
    };
}

struct UnionFind<const N: usize> {
    parent: [u16; N],
    rank: [u8; N],
}

impl<const N: usize> UnionFind<N> {
    fn new() -> Self {
        let mut parent = [0u16; N];
        for i in 0..N {
            parent[i] = i as u16;
        }
        Self {
            parent,
            rank: [0; N],
        }
    }

    fn find(&mut self, mut x: u16) -> u16 {
        while self.parent[x as usize] != x {
            self.parent[x as usize] = self.parent[self.parent[x as usize] as usize];
            x = self.parent[x as usize];
        }
        x
    }

    fn union(&mut self, a: u16, b: u16) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra as usize] < self.rank[rb as usize] {
            self.parent[ra as usize] = rb;
        } else {
            self.parent[rb as usize] = ra;
            if self.rank[ra as usize] == self.rank[rb as usize] {
                self.rank[ra as usize] += 1;
            }
        }
    }
}
