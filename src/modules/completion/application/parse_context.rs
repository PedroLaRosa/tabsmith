use crate::modules::settings::domain::value_objects::settings::Settings;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::specs::domain::value_objects::spec_index::SpecIndex;
use std::collections::HashMap;
use std::path::Path;

/// Everything the parser needs from the session.
pub(crate) struct ParseContext<'a> {
    pub(crate) cwd: &'a Path,
    pub(crate) settings: &'a Settings,
    pub(crate) env: &'a HashMap<String, String>,
    pub(crate) shell: Shell,
    pub(crate) index: &'a SpecIndex,
}
