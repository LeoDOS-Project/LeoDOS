/// A generic point on a 2D grid, with x and y coordinates.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Point {
    pub orb: u8,
    pub sat: u8,
}

impl Point {
    pub fn new(orb: u8, sat: u8) -> Self {
        Self { orb, sat }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
    Ground,
    Local,
}

/// A 2D toroidal grid.
#[derive(Debug, Copy, Clone)]
pub struct Torus {
    pub num_orbs: u8,
    pub num_sats: u8,
}

impl Torus {
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

    pub fn next(index: u8, modulus: u8) -> u8 {
        if index == modulus - 1 { 0 } else { index + 1 }
    }

    pub fn prev(index: u8, modulus: u8) -> u8 {
        if index == 0 { modulus - 1 } else { index - 1 }
    }

    pub fn distance(from: u8, to: u8, modulus: u8) -> u8 {
        if to >= from {
            to - from
        } else {
            modulus - from + to
        }
    }

    pub fn next_sat(&self, p: Point) -> u8 {
        Self::next(p.sat, self.num_sats)
    }

    pub fn prev_sat(&self, p: Point) -> u8 {
        Self::prev(p.sat, self.num_sats)
    }

    pub fn next_orb(&self, p: Point) -> u8 {
        Self::next(p.orb, self.num_orbs)
    }

    pub fn prev_orb(&self, p: Point) -> u8 {
        Self::prev(p.orb, self.num_orbs)
    }

    pub fn distance_orb(&self, from: Point, to: Point) -> u8 {
        Self::distance(from.orb, to.orb, self.num_orbs)
    }

    pub fn distance_sat(&self, from: Point, to: Point) -> u8 {
        Self::distance(from.sat, to.sat, self.num_sats)
    }

    pub fn shortest_path_direction_sat(&self, current: Point, target: Point) -> Direction {
        let north_dist = self.distance_sat(target, current);
        let south_dist = self.distance_sat(current, target);
        if north_dist < south_dist {
            Direction::North
        } else {
            Direction::South
        }
    }

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
