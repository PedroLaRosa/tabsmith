#[cfg(test)]
use std::cell::Cell;

/// When tabsmith last checked for an update in the background.
pub(crate) trait UpdateCheckRepository {
    /// Seconds since the Unix epoch of the last check; `None` before the first.
    fn last_check_secs(&self) -> Option<u64>;
    fn record_check(&self, now_secs: u64);
}

/// The last check kept in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryUpdateCheckRepository {
    last_check_secs: Cell<Option<u64>>,
}

#[cfg(test)]
impl UpdateCheckRepository for InMemoryUpdateCheckRepository {
    fn last_check_secs(&self) -> Option<u64> {
        self.last_check_secs.get()
    }

    fn record_check(&self, now_secs: u64) {
        self.last_check_secs.set(Some(now_secs));
    }
}
