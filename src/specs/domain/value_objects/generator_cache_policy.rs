#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct GeneratorCachePolicy {
    ttl_ms: u64,
    /// `strategy: "max-age"` (else stale-while-revalidate).
    max_age: bool,
    cache_by_directory: bool,
    cache_key: Option<String>,
}

impl GeneratorCachePolicy {
    pub(crate) fn ttl_ms(&self) -> u64 {
        self.ttl_ms
    }

    pub(crate) fn max_age(&self) -> bool {
        self.max_age
    }

    pub(crate) fn cache_by_directory(&self) -> bool {
        self.cache_by_directory
    }

    pub(crate) fn cache_key(&self) -> &Option<String> {
        &self.cache_key
    }

    pub(crate) fn with_ttl_ms(mut self, value: u64) -> Self {
        self.ttl_ms = value;
        self
    }

    pub(crate) fn with_max_age(mut self, value: bool) -> Self {
        self.max_age = value;
        self
    }

    pub(crate) fn with_cache_by_directory(mut self, value: bool) -> Self {
        self.cache_by_directory = value;
        self
    }

    pub(crate) fn with_cache_key(mut self, value: Option<String>) -> Self {
        self.cache_key = value;
        self
    }
}
