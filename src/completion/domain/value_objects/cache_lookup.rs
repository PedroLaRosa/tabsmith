use crate::specs::domain::value_objects::task_id::TaskId;

/// Cache decision for a generator run.
#[derive(Debug, PartialEq)]
pub(crate) enum CacheLookup<V> {
    /// Within TTL.
    Fresh(V),
    /// Past TTL, stale-while-revalidate: serve it; start a background refresh when `refresh`.
    Stale { value: V, refresh: bool },
    /// Nothing usable: fetch (blocking for the slot).
    Miss,
    /// Nothing usable, but an identical fetch is in flight: share its task.
    Shared(TaskId),
}
