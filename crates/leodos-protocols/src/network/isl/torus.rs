/// A generic point on a 2D grid, with x and y coordinates.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Point {
    /// Orbital plane index.
    pub orb: u8,
    /// Satellite index within the orbital plane.
    pub sat: u8,
}

impl Point {
    /// Creates a new point from orbital plane and satellite indices.
    pub fn new(orb: u8, sat: u8) -> Self {
        Self { orb, sat }
    }
}

/// Cardinal direction on the toroidal grid, plus ground and local links.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Direction {
    /// Toward lower satellite index (intra-plane).
    North,
    /// Toward higher satellite index (intra-plane).
    South,
    /// Toward higher orbital plane index (cross-plane).
    East,
    /// Toward lower orbital plane index (cross-plane).
    West,
    /// Toward the ground station.
    Ground,
    /// Destined for the local node itself.
    Local,
}

/// A 2D toroidal grid.
#[derive(Debug, Copy, Clone)]
pub struct Torus {
    /// Number of orbital planes.
    pub num_orbs: u8,
    /// Number of satellites per orbital plane.
    pub num_sats: u8,
}

impl Torus {
    /// Creates a new torus with the given dimensions.
    pub fn new(num_orbs: u8, num_sats: u8) -> Self {
        Self { num_orbs, num_sats }
    }

    /// Calculates the position of a neighbor in a given direction from a starting point.
    pub fn neighbor(&self, point: Point, direction: Direction) -> Point {
        match direction {
            Direction::North => Point::new(point.orb, Self::prev(point.sat, self.num_sats)),
            Direction::South => Point::new(point.orb, Self::next(point.sat, self.num_sats)),
            Direction::East => Point::new(Self::next(point.orb, self.num_orbs), point.sat),
            Direction::West => Point::new(Self::prev(point.orb, self.num_orbs), point.sat),
            Direction::Ground | Direction::Local => point,
        }
    }

    // --- Topology Helpers (Used by Strategies) ---

    /// Returns the next index, wrapping around at `modulus`.
    pub fn next(index: u8, modulus: u8) -> u8 {
        if index == modulus - 1 { 0 } else { index + 1 }
    }

    /// Returns the previous index, wrapping around at `modulus`.
    pub fn prev(index: u8, modulus: u8) -> u8 {
        if index == 0 { modulus - 1 } else { index - 1 }
    }

    /// Returns the forward distance from `from` to `to` on a circular axis.
    pub fn distance(from: u8, to: u8, modulus: u8) -> u8 {
        if to >= from {
            to - from
        } else {
            modulus - from + to
        }
    }

    /// Returns the satellite index after `p.sat`, wrapping around.
    pub fn next_sat(&self, p: Point) -> u8 {
        Self::next(p.sat, self.num_sats)
    }

    /// Returns the satellite index before `p.sat`, wrapping around.
    pub fn prev_sat(&self, p: Point) -> u8 {
        Self::prev(p.sat, self.num_sats)
    }

    /// Returns the orbital plane index after `p.orb`, wrapping around.
    pub fn next_orb(&self, p: Point) -> u8 {
        Self::next(p.orb, self.num_orbs)
    }

    /// Returns the orbital plane index before `p.orb`, wrapping around.
    pub fn prev_orb(&self, p: Point) -> u8 {
        Self::prev(p.orb, self.num_orbs)
    }

    /// Forward distance between two points along the orbital plane axis.
    pub fn distance_orb(&self, from: Point, to: Point) -> u8 {
        Self::distance(from.orb, to.orb, self.num_orbs)
    }

    /// Forward distance between two points along the satellite axis.
    pub fn distance_sat(&self, from: Point, to: Point) -> u8 {
        Self::distance(from.sat, to.sat, self.num_sats)
    }

    /// Returns the shortest-path direction along the satellite axis.
    pub fn shortest_path_direction_sat(&self, current: Point, target: Point) -> Direction {
        let north_dist = self.distance_sat(target, current);
        let south_dist = self.distance_sat(current, target);
        if north_dist < south_dist {
            Direction::North
        } else {
            Direction::South
        }
    }

    /// Returns the shortest-path direction along the orbital plane axis.
    pub fn shortest_path_direction_orb(&self, current: Point, target: Point) -> Direction {
        let west_dist = self.distance_orb(target, current);
        let east_dist = self.distance_orb(current, target);
        if west_dist < east_dist {
            Direction::West
        } else {
            Direction::East
        }
    }
}
