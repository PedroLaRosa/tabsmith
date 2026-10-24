use std::time::Instant;

/// What time it is. Wall-clock time is for values kept or compared across runs (generator cache ages, recency
/// timestamps in `state.json`, history times, backup names); the monotonic `Instant` is for timeouts and debounces.
pub(crate) trait Clock {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;

    /// Whole seconds since the Unix epoch: the milliseconds divided by 1000, truncated.
    fn now_secs(&self) -> u64 {
        self.now_ms() / 1000
    }

    fn now_instant(&self) -> Instant;
}

/// Test double: the wall clock reads the milliseconds it was built with and the `Instant` is taken once, when it is
/// built, so no time passes for the code under test.
#[cfg(test)]
pub(crate) struct FixedClock {
    now_ms: u64,
    now_instant: Instant,
}

#[cfg(test)]
impl FixedClock {
    pub(crate) fn new(now_ms: u64) -> Self {
        FixedClock {
            now_ms,
            now_instant: Instant::now(),
        }
    }
}

#[cfg(test)]
impl Clock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }

    fn now_instant(&self) -> Instant {
        self.now_instant
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_the_wall_clock_milliseconds_it_was_fixed_at() {
        let clock = FixedClock::new(1_700_000_123_456);

        let now = clock.now_ms();

        assert_eq!(now, 1_700_000_123_456);
    }

    #[test]
    fn should_report_whole_wall_clock_seconds_dropping_the_milliseconds() {
        let clock = FixedClock::new(1_700_000_123_999);

        let now = clock.now_secs();

        assert_eq!(now, 1_700_000_123);
    }

    #[test]
    fn should_keep_its_monotonic_instant_still_while_real_time_passes() {
        let clock = FixedClock::new(0);
        let first = clock.now_instant();

        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = clock.now_instant();

        assert_eq!(first, second);
    }
}
