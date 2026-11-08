use crate::modules::completion::domain::entities::completion_session::CompletionSession;
use crate::modules::completion::domain::services::history_suggestions;
use crate::modules::completion::domain::services::suggestion_assembler;
use crate::modules::completion::domain::services::suggestion_finalizer;
use crate::modules::completion::domain::services::suggestion_ranker;
use crate::modules::completion::domain::value_objects::argument_hint::ArgumentHint;
use crate::modules::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::modules::completion::domain::value_objects::completion_result::{CompletionResult, CompletionResultParts};
use crate::modules::completion::domain::value_objects::filter_options::FilterOptions;
use crate::modules::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
use crate::modules::history::domain::entities::history_entry::HistoryEntry;
use crate::modules::settings::domain::services::settings_catalog::{
    AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN, AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION,
    AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE, AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS,
    AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY, AUTOCOMPLETE_ONLY_SHOW_ON_TAB,
    AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS, AUTOCOMPLETE_SORT_METHOD, BETA_HISTORY_MODE,
};

/// Rank the current request's suggestions (SPEC §6.7.5) into a reply.
/// The reply carries no ghost text: whoever sends it adds the ghost text (Q15).
pub(crate) fn build_completion_result(
    session: &CompletionSession,
    phase: CompletionPhase,
    recent_history: &[HistoryEntry],
) -> CompletionResult {
    let served = session.served().expect("reply without a current request");
    let (pr, s) = (served.parsed(), session.settings());
    let mut list = if pr.first_token() {
        served.first_token_candidates().clone()
    } else {
        let views = session.slot_views(served.queries());
        suggestion_assembler::assemble(pr, &views)
    };
    let mode = s.string(BETA_HISTORY_MODE).unwrap_or_default();
    if served.history_mode() || mode == "show" || mode == "history_only" {
        let hist = history_suggestions::history_rows(served.history_prefix(), recent_history);
        list = history_suggestions::mix_history(list, hist, mode == "show" && !served.history_mode());
    }
    let fo = FilterOptions::default()
        .with_fuzzy(served.fuzzy())
        .with_auto_exec_hidden(
            s.bool(AUTOCOMPLETE_HIDE_AUTO_EXECUTE_SUGGESTION) || s.bool(AUTOCOMPLETE_ONLY_SHOW_ON_TAB),
        )
        .with_exec_after_space(s.bool(AUTOCOMPLETE_IMMEDIATELY_EXECUTE_AFTER_SPACE))
        .with_run_dangerous(s.bool(AUTOCOMPLETE_IMMEDIATELY_RUN_DANGEROUS_COMMANDS))
        .with_suggest_current_token(
            pr.current_arg()
                .as_ref()
                .and_then(|a| a.suggest_current_token())
                .unwrap_or_else(|| s.bool(AUTOCOMPLETE_ALWAYS_SUGGEST_CURRENT_TOKEN)),
        );
    let cmd = pr.tokens().get(pr.command_index()).map_or("", String::as_str); // D21
    let use_recency = s.string(AUTOCOMPLETE_SORT_METHOD) != Some("alphabetical");
    let map = Some(session.recency()).filter(|_| use_recency).and_then(|r| r.of_command(cmd));
    let ranked = suggestion_ranker::rank(list, pr.search_term(), map, &fo);
    let ic = InsertionContext::new(pr.search_term(), pr.raw_last_token(), served.shell())
        .with_fuzzy(served.fuzzy())
        .with_prefer_verbose(s.bool(AUTOCOMPLETE_PREFER_VERBOSE_SUGGESTIONS))
        .with_insert_space(s.bool(AUTOCOMPLETE_INSERT_SPACE_AUTOMATICALLY));
    let items: Vec<Suggestion> = ranked
        .into_iter()
        .map(|(x, p)| suggestion_finalizer::to_item(x, p, &ic))
        .collect();
    CompletionResult::new(CompletionResultParts {
        req: served.request_id(),
        phase,
        error: false,
        buffer: served.buffer().to_string(),
        cursor: served.cursor(),
        tokens: pr.tokens().clone(),
        command_text: served.command_text().to_string(),
        command_index: pr.command_index(),
        arg_key: served.argument_key(),
        current_arg: pr.current_arg().as_ref().map(|a| {
            ArgumentHint::new(
                a.name().clone().unwrap_or_default(),
                a.description().clone().unwrap_or_default(),
            )
        }),
        search_term: pr.search_term().to_string(),
        triggered_generators: if phase == CompletionPhase::Parse {
            served.triggered_generators().clone()
        } else {
            Vec::new()
        },
        items,
        loading: session.any_slot_loading(),
        ghost: None,
    })
}
