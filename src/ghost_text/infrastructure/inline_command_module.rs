use crate::ghost_text::application::get_inline_status_use_case::GetInlineStatusUseCase;
use crate::ghost_text::application::list_ghost_text_sources_use_case::ListGhostTextSourcesUseCase;
use crate::ghost_text::application::select_ghost_text_source_use_case::SelectGhostTextSourceUseCase;
use crate::ghost_text::application::set_inline_enabled_use_case::SetInlineEnabledUseCase;
use crate::ghost_text::infrastructure::adapters::stdin_source_chooser::StdinSourceChooser;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use std::rc::Rc;

/// The use cases of the `inline` command, over the settings the composition root injects.
pub(crate) struct InlineCommandModule {
    pub(crate) set_inline_enabled: SetInlineEnabledUseCase,
    pub(crate) get_inline_status: GetInlineStatusUseCase,
    pub(crate) list_ghost_text_sources: ListGhostTextSourcesUseCase,
    pub(crate) select_ghost_text_source: SelectGhostTextSourceUseCase,
}

pub(crate) fn build_inline_command_module(
    settings: Rc<dyn SettingsRepository>,
) -> InlineCommandModule {
    InlineCommandModule {
        set_inline_enabled: SetInlineEnabledUseCase::new(settings.clone()),
        get_inline_status: GetInlineStatusUseCase::new(settings.clone()),
        list_ghost_text_sources: ListGhostTextSourcesUseCase::new(settings.clone()),
        select_ghost_text_source: SelectGhostTextSourceUseCase::new(
            settings,
            Rc::new(StdinSourceChooser),
        ),
    }
}
