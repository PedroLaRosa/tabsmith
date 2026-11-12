use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::shared::domain::value_objects::data_directory::DataDirectory;
use crate::modules::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::modules::shared::infrastructure::system_clock::SystemClock;
use std::sync::Arc;

/// What the composition root hands every module's `build_<module>_module`. Building it never fails: an unknown
/// home only surfaces from the data directory paths a command asks for.
pub(crate) struct SharedDependencies {
    pub(crate) data_directory: DataDirectory,
    /// Shared across threads: the completion engine runs on its own.
    pub(crate) clock: Arc<dyn Clock + Send + Sync>,
}

impl SharedDependencies {
    /// What every module is built from. Never fails: an unknown home only fails the paths a command asks for.
    pub(crate) fn from_env() -> Self {
        SharedDependencies {
            data_directory: data_directory_from_env(),
            clock: Arc::new(SystemClock),
        }
    }
}
