use crate::network::isl::geo::LatLon;
use crate::network::isl::projection::Projection;
use crate::network::isl::shell::Shell;
use crate::network::isl::torus::Point;

/// Maps ground station IDs to geographic positions and
/// resolves the best gateway satellite via line-of-sight.
pub struct GatewayTable<const N: usize> {
    stations: heapless::Vec<(u8, LatLon), N>,
    min_elevation_deg: f32,
}

impl<const N: usize> GatewayTable<N> {
    /// Creates an empty gateway table.
    pub fn new(min_elevation_deg: f32) -> Self {
        Self {
            stations: heapless::Vec::new(),
            min_elevation_deg,
        }
    }

    /// Registers a ground station with its geographic position.
    pub fn add_station(&mut self, station: u8, position: LatLon) {
        self.stations.push((station, position)).ok();
    }

    /// Finds the gateway satellite for a ground station at
    /// the given time using line-of-sight calculation.
    ///
    /// Returns `None` if the station ID is unknown or no
    /// satellite has LOS.
    pub fn gateway(
        &self,
        shell: &Shell,
        station: u8,
        time_s: u32,
    ) -> Option<Point> {
        let (_, pos) = self
            .stations
            .iter()
            .find(|(id, _)| *id == station)?;
        let proj = Projection::new(*shell);
        proj.find_gateway(*pos, time_s as f32, self.min_elevation_deg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::isl::torus::Torus;

    #[test]
    fn gateway_for_known_station() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);

        let mut table = GatewayTable::<4>::new(5.0);
        table.add_station(0, LatLon::new(0.0, 0.0));

        let gw = table.gateway(&shell, 0, 0);
        assert!(gw.is_some(), "should find gateway at t=0");
    }

    #[test]
    fn gateway_for_unknown_station() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);

        let table = GatewayTable::<4>::new(5.0);
        let gw = table.gateway(&shell, 99, 0);
        assert!(gw.is_none());
    }
}
