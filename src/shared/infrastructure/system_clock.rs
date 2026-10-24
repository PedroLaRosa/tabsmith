use crate::shared::application::ports::clock::Clock;
use std::time::Instant;

/// The real clock: `SystemTime` for wall-clock time.
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn now_instant(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    fn millis_since_unix_epoch() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
    }

    #[test]
    fn should_report_the_milliseconds_elapsed_since_the_unix_epoch() {
        let before = millis_since_unix_epoch();

        let now = SystemClock.now_ms();

        let after = millis_since_unix_epoch();
        assert!(before <= now && now <= after, "{before} <= {now} <= {after}");
    }

    #[test]
    fn should_report_the_monotonic_instant_of_the_moment_it_is_asked() {
        let before = Instant::now();

        let now = SystemClock.now_instant();

        let after = Instant::now();
        assert!(before <= now && now <= after, "{before:?} <= {now:?} <= {after:?}");
    }
}
