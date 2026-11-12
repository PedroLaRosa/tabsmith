use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfile;

/// What `inline show-customizations` lists: the selected source and every configured profile, or the reason
/// it is invalid.
pub(crate) struct GhostTextSources {
    pub(crate) selected: String,
    pub(crate) profiles: Vec<(String, Result<LlmProfile, String>)>,
}
