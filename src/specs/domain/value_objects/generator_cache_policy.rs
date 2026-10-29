#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GeneratorCachePolicy {
    pub(crate) ttl_ms: u64,
    /// `strategy: "max-age"` (else stale-while-revalidate).
    pub(crate) max_age: bool,
    pub(crate) cache_by_directory: bool,
    pub(crate) cache_key: Option<String>,
}
