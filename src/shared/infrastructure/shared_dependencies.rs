use crate::shared::domain::value_objects::data_directory::DataDirectory;

/// What the composition root hands every module's `build_<module>_module`. Building it never fails: an unknown
/// home only surfaces from the data directory paths a command asks for.
pub(crate) struct SharedDependencies {
    pub(crate) data_directory: DataDirectory,
}
