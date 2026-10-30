use crate::ghost_text::application::suggest_local_ghost_text_use_case::SuggestLocalGhostTextUseCase;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use std::rc::Rc;

/// The ghost text use cases of the engine thread, over the history the history module records into.
pub(crate) struct GhostTextModule {
    pub(crate) suggest_local_ghost_text: SuggestLocalGhostTextUseCase,
}

pub(crate) fn build_ghost_text_module(history: Rc<dyn HistoryRepository>) -> GhostTextModule {
    GhostTextModule {
        suggest_local_ghost_text: SuggestLocalGhostTextUseCase::new(history),
    }
}
