/// Policy for computing retransmission timeout values.
pub trait RtoPolicy {
    /// Compute the retransmission timeout in ticks for the current time.
    fn rto_ticks(&self, now_secs: u32) -> u32;
}

/// Fixed retransmission timeout that ignores orbital dynamics.
pub struct FixedRto {
    /// Constant timeout value in ticks.
    rto_ticks: u32,
}

impl FixedRto {
    /// Create a new fixed RTO policy with the given timeout in ticks.
    pub fn new(rto_ticks: u32) -> Self {
        Self { rto_ticks }
    }
}

impl RtoPolicy for FixedRto {
    fn rto_ticks(&self, _now_secs: u32) -> u32 {
        self.rto_ticks
    }
}

/// A ground station contact window defined by start and end times.
#[derive(Debug, Clone)]
pub struct ContactWindow {
    /// Ground station identifier.
    pub station_id: u8,
    /// Window start time in mission-elapsed seconds.
    pub start_secs: u32,
    /// Window end time in mission-elapsed seconds (exclusive).
    pub end_secs: u32,
}

/// Ordered schedule of ground station contact windows.
pub struct ContactSchedule<const N: usize> {
    /// Chronologically ordered list of contact windows.
    windows: heapless::Vec<ContactWindow, N>,
}

impl<const N: usize> ContactSchedule<N> {
    /// Create an empty contact schedule.
    pub fn new() -> Self {
        Self {
            windows: heapless::Vec::new(),
        }
    }

    /// Insert a contact window in chronological order.
    pub fn add_window(&mut self, window: ContactWindow) -> Result<(), ContactWindow> {
        let pos = self
            .windows
            .iter()
            .position(|w| w.start_secs > window.start_secs)
            .unwrap_or(self.windows.len());

        self.windows.insert(pos, window).map_err(|e| e)
    }

    /// Check if the given time falls within any contact window.
    pub fn in_window(&self, now_secs: u32) -> bool {
        self.windows
            .iter()
            .any(|w| now_secs >= w.start_secs && now_secs < w.end_secs)
    }

    /// Return the next contact window starting after the given time.
    pub fn next_window(&self, now_secs: u32) -> Option<&ContactWindow> {
        self.windows.iter().find(|w| w.start_secs > now_secs)
    }
}

/// RTO policy that adapts timeout based on orbital contact windows.
pub struct OrbitAwareRto<const N: usize> {
    /// RTO used during active ISL contact windows.
    isl_rto_ticks: u32,
    /// Extra margin added to the wait-for-window timeout.
    margin_ticks: u32,
    /// Ground station contact schedule.
    schedule: ContactSchedule<N>,
}

impl<const N: usize> OrbitAwareRto<N> {
    /// Create an orbit-aware RTO with ISL timeout, margin, and contact schedule.
    pub fn new(isl_rto_ticks: u32, margin_ticks: u32, schedule: ContactSchedule<N>) -> Self {
        Self {
            isl_rto_ticks,
            margin_ticks,
            schedule,
        }
    }
}

impl<const N: usize> RtoPolicy for OrbitAwareRto<N> {
    fn rto_ticks(&self, now_secs: u32) -> u32 {
        if self.schedule.in_window(now_secs) {
            return self.isl_rto_ticks;
        }

        match self.schedule.next_window(now_secs) {
            Some(window) => {
                let secs_until = window.start_secs - now_secs;
                secs_until * 1000 + self.margin_ticks
            }
            None => self.isl_rto_ticks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_rto() {
        let policy = FixedRto::new(500);
        assert_eq!(policy.rto_ticks(0), 500);
        assert_eq!(policy.rto_ticks(1000), 500);
        assert_eq!(policy.rto_ticks(u32::MAX), 500);
    }

    #[test]
    fn test_orbit_aware_in_window() {
        let mut schedule = ContactSchedule::<4>::new();
        schedule
            .add_window(ContactWindow {
                station_id: 1,
                start_secs: 100,
                end_secs: 200,
            })
            .unwrap();

        let policy = OrbitAwareRto::new(50, 500, schedule);

        assert_eq!(policy.rto_ticks(100), 50);
        assert_eq!(policy.rto_ticks(150), 50);
        assert_eq!(policy.rto_ticks(199), 50);
    }

    #[test]
    fn test_orbit_aware_between_windows() {
        let mut schedule = ContactSchedule::<4>::new();
        schedule
            .add_window(ContactWindow {
                station_id: 1,
                start_secs: 100,
                end_secs: 200,
            })
            .unwrap();
        schedule
            .add_window(ContactWindow {
                station_id: 2,
                start_secs: 500,
                end_secs: 600,
            })
            .unwrap();

        let policy = OrbitAwareRto::new(50, 500, schedule);

        assert_eq!(policy.rto_ticks(0), 100 * 1000 + 500);
        assert_eq!(policy.rto_ticks(250), 250 * 1000 + 500);
    }

    #[test]
    fn test_orbit_aware_no_future_windows() {
        let mut schedule = ContactSchedule::<4>::new();
        schedule
            .add_window(ContactWindow {
                station_id: 1,
                start_secs: 100,
                end_secs: 200,
            })
            .unwrap();

        let policy = OrbitAwareRto::new(50, 500, schedule);

        assert_eq!(policy.rto_ticks(300), 50);
    }

    #[test]
    fn test_contact_schedule_queries() {
        let mut schedule = ContactSchedule::<4>::new();
        schedule
            .add_window(ContactWindow {
                station_id: 1,
                start_secs: 100,
                end_secs: 200,
            })
            .unwrap();
        schedule
            .add_window(ContactWindow {
                station_id: 2,
                start_secs: 300,
                end_secs: 400,
            })
            .unwrap();

        assert!(!schedule.in_window(50));
        assert!(schedule.in_window(100));
        assert!(schedule.in_window(150));
        assert!(!schedule.in_window(200));
        assert!(!schedule.in_window(250));
        assert!(schedule.in_window(300));
        assert!(schedule.in_window(350));
        assert!(!schedule.in_window(400));

        assert_eq!(schedule.next_window(0).unwrap().station_id, 1);
        assert_eq!(schedule.next_window(150).unwrap().station_id, 2);
        assert!(schedule.next_window(350).is_none());
    }
}
