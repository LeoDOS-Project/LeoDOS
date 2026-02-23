pub trait RtoPolicy {
    fn rto_ticks(&self, now_secs: u32) -> u32;
}

pub struct FixedRto {
    rto_ticks: u32,
}

impl FixedRto {
    pub fn new(rto_ticks: u32) -> Self {
        Self { rto_ticks }
    }
}

impl RtoPolicy for FixedRto {
    fn rto_ticks(&self, _now_secs: u32) -> u32 {
        self.rto_ticks
    }
}

#[derive(Debug, Clone)]
pub struct ContactWindow {
    pub station_id: u8,
    pub start_secs: u32,
    pub end_secs: u32,
}

pub struct ContactSchedule<const N: usize> {
    windows: heapless::Vec<ContactWindow, N>,
}

impl<const N: usize> ContactSchedule<N> {
    pub fn new() -> Self {
        Self {
            windows: heapless::Vec::new(),
        }
    }

    pub fn add_window(&mut self, window: ContactWindow) -> Result<(), ContactWindow> {
        let pos = self
            .windows
            .iter()
            .position(|w| w.start_secs > window.start_secs)
            .unwrap_or(self.windows.len());

        self.windows.insert(pos, window).map_err(|e| e)
    }

    pub fn in_window(&self, now_secs: u32) -> bool {
        self.windows
            .iter()
            .any(|w| now_secs >= w.start_secs && now_secs < w.end_secs)
    }

    pub fn next_window(&self, now_secs: u32) -> Option<&ContactWindow> {
        self.windows.iter().find(|w| w.start_secs > now_secs)
    }
}

pub struct OrbitAwareRto<const N: usize> {
    isl_rto_ticks: u32,
    margin_ticks: u32,
    schedule: ContactSchedule<N>,
}

impl<const N: usize> OrbitAwareRto<N> {
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
