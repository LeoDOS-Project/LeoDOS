use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SatAddr {
    pub sat: usize,
    pub orb: usize,
}

impl SatAddr {
    pub fn new(sat: usize, orb: usize) -> Self {
        Self { sat, orb }
    }
}

impl std::fmt::Display for SatAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({},{})", self.sat, self.orb)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::North => write!(f, "N"),
            Direction::South => write!(f, "S"),
            Direction::East => write!(f, "E"),
            Direction::West => write!(f, "W"),
        }
    }
}

pub struct IslConfig {
    pub max_sat: usize,
    pub max_orb: usize,
    pub radius: f64,
    pub alt: f64,
    pub optimize: bool,
    pub crossover: bool,
}

impl Default for IslConfig {
    fn default() -> Self {
        Self {
            max_sat: 19,
            max_orb: 5,
            radius: 6357.0,
            alt: 550.0,
            optimize: true,
            crossover: false,
        }
    }
}

pub struct IslRouter {
    cfg: IslConfig,
}

impl IslRouter {
    pub fn new(cfg: IslConfig) -> Self {
        Self { cfg }
    }

    fn wrap(&self, sat: usize, orb: usize) -> SatAddr {
        let sat = match sat {
            0 => self.cfg.max_sat,
            s if s > self.cfg.max_sat => 1,
            s => s,
        };
        let orb = match orb {
            0 => self.cfg.max_orb,
            o if o > self.cfg.max_orb => 1,
            o => o,
        };
        SatAddr::new(sat, orb)
    }

    fn step(&self, addr: SatAddr, dir: Direction) -> SatAddr {
        let (sat, orb) = match dir {
            Direction::North => (addr.sat, addr.orb.wrapping_sub(1)),
            Direction::South => (addr.sat, addr.orb + 1),
            Direction::West => (addr.sat.wrapping_sub(1), addr.orb),
            Direction::East => (addr.sat + 1, addr.orb),
        };
        self.wrap(sat, orb)
    }

    fn dist_north(&self, from: usize, to: usize) -> usize {
        if to < from {
            from - to
        } else if to > from {
            from + self.cfg.max_orb - to
        } else {
            0
        }
    }

    fn dist_south(&self, from: usize, to: usize) -> usize {
        if to > from {
            to - from
        } else if to < from {
            self.cfg.max_orb - from + to
        } else {
            0
        }
    }

    fn dist_west(&self, from: usize, to: usize) -> usize {
        if to < from {
            from - to
        } else if to > from {
            from + self.cfg.max_sat - to
        } else {
            0
        }
    }

    fn dist_east(&self, from: usize, to: usize) -> usize {
        if to > from {
            to - from
        } else if to < from {
            self.cfg.max_sat - from + to
        } else {
            0
        }
    }

    fn vertical(&self, from: usize, to: usize) -> Option<Direction> {
        let n = self.dist_north(from, to);
        let s = self.dist_south(from, to);
        match n.cmp(&s) {
            std::cmp::Ordering::Less => Some(Direction::North),
            std::cmp::Ordering::Greater => Some(Direction::South),
            std::cmp::Ordering::Equal => None,
        }
    }

    fn horizontal(&self, from: usize, to: usize) -> Option<Direction> {
        let w = self.dist_west(from, to);
        let e = self.dist_east(from, to);
        match w.cmp(&e) {
            std::cmp::Ordering::Less => Some(Direction::West),
            std::cmp::Ordering::Greater => Some(Direction::East),
            std::cmp::Ordering::Equal => None,
        }
    }

    fn orb_dist(&self, sat: usize) -> f64 {
        let r = self.cfg.radius + self.cfg.alt;
        let angle = 2.0 * PI * (sat as f64 / self.cfg.max_sat as f64);
        let base = r * (2.0 * (1.0 - (2.0 * PI / self.cfg.max_orb as f64).cos())).sqrt();
        base * (angle.cos().powi(2) * (1.0 + angle.sin().powi(2))).sqrt()
    }

    pub fn next_hop(&self, from: SatAddr, to: SatAddr) -> Option<Direction> {
        if from == to {
            return None;
        }

        let vert = self.vertical(from.orb, to.orb);
        let horiz = self.horizontal(from.sat, to.sat);

        if self.cfg.optimize {
            if let Some(h) = horiz {
                let my = self.orb_dist(from.sat);
                let neighbor = self.orb_dist(self.step(from, h).sat);

                let at_crossover = self.orb_dist(self.step(from, Direction::West).sat) > my
                    && self.orb_dist(self.step(from, Direction::East).sat) > my;

                if self.cfg.crossover && at_crossover {
                    return Some(h);
                }
                if my > neighbor {
                    return Some(h);
                }
            }
        }

        vert.or(horiz)
    }
}
