use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use std::sync::Arc;

/// What the composition root hands every module's `build_<module>_module`. Building it never fails: an unknown
/// home only surfaces from the data directory paths a command asks for.
pub(crate) struct SharedDependencies {
    pub(crate) data_directory: DataDirectory,
    /// Shared across threads: the completion engine runs on its own.
    pub(crate) clock: Arc<dyn Clock + Send + Sync>,
}
