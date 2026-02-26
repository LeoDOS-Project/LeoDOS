//! Area of Interest (AOI) for SpaceCoMP task scheduling.
//!
//! An AOI defines a rectangular region on the satellite mesh where data
//! collection and processing occurs. The AOI is specified as a bounding box
//! with upper-left and lower-right corners, handling wraparound on the torus.

use crate::network::isl::torus::{Point, Torus};

#[derive(Debug, Clone, Copy)]
struct WrappedRange {
    start: u8,
    end: u8,
    modulus: u8,
}

impl WrappedRange {
    fn new(start: u8, end: u8, modulus: u8) -> Self {
        Self { start, end, modulus }
    }

    fn contains(&self, val: u8) -> bool {
        if self.start <= self.end {
            val >= self.start && val <= self.end
        } else {
            val >= self.start || val <= self.end
        }
    }

    fn span(&self) -> u8 {
        if self.end >= self.start {
            self.end - self.start + 1
        } else {
            self.modulus - self.start + self.end + 1
        }
    }

    fn midpoint(&self) -> u8 {
        let forward = Torus::distance(self.start, self.end, self.modulus);
        let backward = Torus::distance(self.end, self.start, self.modulus);

        if forward <= backward {
            (self.start as u16 + forward as u16 / 2) as u8 % self.modulus
        } else {
            self.start.wrapping_sub(backward / 2) % self.modulus
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Aoi {
    pub upper_left: Point,
    pub lower_right: Point,
}

impl Aoi {
    pub fn new(upper_left: Point, lower_right: Point) -> Self {
        Self { upper_left, lower_right }
    }

    fn y_range(&self, torus: &Torus) -> WrappedRange {
        WrappedRange::new(self.upper_left.orb, self.lower_right.orb, torus.num_orbs)
    }

    fn x_range(&self, torus: &Torus) -> WrappedRange {
        WrappedRange::new(self.upper_left.sat, self.lower_right.sat, torus.num_sats)
    }

    pub fn center(&self, torus: &Torus) -> Point {
        Point::new(self.y_range(torus).midpoint(), self.x_range(torus).midpoint())
    }

    pub fn contains(&self, torus: &Torus, point: Point) -> bool {
        self.y_range(torus).contains(point.orb) && self.x_range(torus).contains(point.sat)
    }

    pub fn width(&self, torus: &Torus) -> u8 {
        self.x_range(torus).span()
    }

    pub fn height(&self, torus: &Torus) -> u8 {
        self.y_range(torus).span()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_center_simple() {
        let torus = Torus::new(8, 8);
        let aoi = Aoi::new(Point::new(1, 1), Point::new(3, 3));
        assert_eq!(aoi.center(&torus), Point::new(2, 2));
    }

    #[test]
    fn test_center_wraparound() {
        let torus = Torus::new(8, 8);
        let aoi = Aoi::new(Point::new(6, 6), Point::new(2, 2));
        assert_eq!(aoi.center(&torus), Point::new(0, 0));
    }

    #[test]
    fn test_contains_simple() {
        let torus = Torus::new(8, 8);
        let aoi = Aoi::new(Point::new(1, 1), Point::new(3, 3));

        assert!(aoi.contains(&torus, Point::new(2, 2)));
        assert!(aoi.contains(&torus, Point::new(1, 1)));
        assert!(aoi.contains(&torus, Point::new(3, 3)));
        assert!(!aoi.contains(&torus, Point::new(0, 0)));
        assert!(!aoi.contains(&torus, Point::new(4, 4)));
    }

    #[test]
    fn test_contains_wraparound() {
        let torus = Torus::new(8, 8);
        let aoi = Aoi::new(Point::new(6, 6), Point::new(2, 2));

        assert!(aoi.contains(&torus, Point::new(0, 0)));
        assert!(aoi.contains(&torus, Point::new(7, 7)));
        assert!(aoi.contains(&torus, Point::new(6, 6)));
        assert!(aoi.contains(&torus, Point::new(2, 2)));
        assert!(!aoi.contains(&torus, Point::new(4, 4)));
    }

    #[test]
    fn test_dimensions() {
        let torus = Torus::new(8, 8);

        let aoi = Aoi::new(Point::new(1, 1), Point::new(3, 5));
        assert_eq!(aoi.height(&torus), 3);
        assert_eq!(aoi.width(&torus), 5);

        let aoi_wrap = Aoi::new(Point::new(6, 6), Point::new(2, 2));
        assert_eq!(aoi_wrap.height(&torus), 5);
        assert_eq!(aoi_wrap.width(&torus), 5);
    }
}
