/// A generic point on a 2D grid, with x and y coordinates.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Point {
    pub(crate) y: u8,
    pub(crate) x: u8,
}

impl Point {
    pub fn new(y: u8, x: u8) -> Self {
        Self { y, x }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
    Local,
}

/// A pure mathematical representation of a 2D toroidal grid.
/// This struct knows NOTHING about physics, orbits, or routing logic.
#[derive(Debug, Copy, Clone)]
pub struct Torus {
    pub num_cols: u8,
    pub num_rows: u8,
}

impl Torus {
    pub fn new(num_rows: u8, num_cols: u8) -> Self {
        Self { num_rows, num_cols }
    }

    /// Calculates the position of a neighbor in a given direction from a starting point.
    pub fn neighbor(&self, position: Point, direction: Direction) -> Point {
        match direction {
            Direction::North => Point::new(Self::prev(position.y, self.num_rows), position.x),
            Direction::South => Point::new(Self::next(position.y, self.num_rows), position.x),
            Direction::East => Point::new(position.y, Self::next(position.x, self.num_cols)),
            Direction::West => Point::new(position.y, Self::prev(position.x, self.num_cols)),
            Direction::Local => position,
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

    pub fn next_x(&self, p: Point) -> u8 {
        Self::next(p.x, self.num_cols)
    }

    pub fn prev_x(&self, p: Point) -> u8 {
        Self::prev(p.x, self.num_cols)
    }

    pub fn next_y(&self, p: Point) -> u8 {
        Self::next(p.y, self.num_rows)
    }

    pub fn prev_y(&self, p: Point) -> u8 {
        Self::prev(p.y, self.num_rows)
    }

    pub fn distance_y(&self, from: Point, to: Point) -> u8 {
        Self::distance(from.y, to.y, self.num_rows)
    }

    pub fn distance_x(&self, from: Point, to: Point) -> u8 {
        Self::distance(from.x, to.x, self.num_cols)
    }

    pub fn adjacent_x(&self, current: Point, dir: Direction) -> (u8, u8) {
        if dir == Direction::East {
            (self.next_x(current), self.prev_x(current))
        } else {
            (self.prev_x(current), self.next_x(current))
        }
    }

    pub fn adjacent_y(&self, current: Point, dir: Direction) -> (u8, u8) {
        if dir == Direction::South {
            (self.next_y(current), self.prev_y(current))
        } else {
            (self.prev_y(current), self.next_y(current))
        }
    }

    pub fn shortest_path_direction_x(&self, current: Point, target: Point) -> Direction {
        let west_dist = self.distance_x(target, current);
        let east_dist = self.distance_x(current, target);
        if west_dist < east_dist {
            Direction::West
        } else {
            Direction::East
        }
    }

    pub fn shortest_path_direction_y(&self, current: Point, target: Point) -> Direction {
        let north_dist = self.distance_y(target, current);
        let south_dist = self.distance_y(current, target);
        if north_dist < south_dist {
            Direction::North
        } else {
            Direction::South
        }
    }
}
