use std::time::Instant;
#[cfg(test)]
use std::sync::Mutex;
#[cfg(test)]
use std::time::Duration;

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

/// Test double: the wall clock reads the milliseconds it was built with and the `Instant` is taken when it is built and
/// moves only when advanced, so no time passes for the code under test unless the test moves it.
#[cfg(test)]
pub(crate) struct FixedClock {
    now_ms: u64,
    now_instant: Mutex<Instant>,
}

#[cfg(test)]
impl FixedClock {
    pub(crate) fn new(now_ms: u64) -> Self {
        FixedClock {
            now_ms,
            now_instant: Mutex::new(Instant::now()),
        }
    }

    /// Moves the `Instant` forward by `by`; the wall clock stays where it was fixed.
    pub(crate) fn advance(&self, by: Duration) {
        *self.now_instant.lock().unwrap() += by;
    }
}

#[cfg(test)]
impl Clock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }

    fn now_instant(&self) -> Instant {
        *self.now_instant.lock().unwrap()
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

    #[test]
    fn should_move_its_instant_forward_when_advanced() {
        let clock = FixedClock::new(0);
        let before = clock.now_instant();
        let debounce = std::time::Duration::from_millis(250);

        clock.advance(debounce);

        assert_eq!(clock.now_instant(), before + debounce);
    }
}
