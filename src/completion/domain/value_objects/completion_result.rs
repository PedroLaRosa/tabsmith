use crate::completion::domain::value_objects::argument_hint::ArgumentHint;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::suggestion::Suggestion;

/// Engine → wrapper reply (SPEC §4.3).
#[derive(Clone, Debug)]
pub(crate) struct CompletionResult {
    pub(crate) req: u64,
    pub(crate) phase: CompletionPhase,
    pub(crate) error: bool,
    /// The buffer/cursor the reply was computed from (for insertion reconciliation, SPEC §7.3).
    pub(crate) buffer: String,
    pub(crate) cursor: usize,
    /// Token texts of the current command (after alias expansion).
    pub(crate) tokens: Vec<String>,
    pub(crate) command_text: String,
    pub(crate) command_index: usize,
    /// Identity of (current arg, current subcommand) — changes when a new argument starts.
    pub(crate) arg_key: u64,
    pub(crate) current_arg: Option<ArgumentHint>,
    pub(crate) search_term: String,
    pub(crate) triggered_generators: Vec<GeneratorKey>,
    pub(crate) items: Vec<Suggestion>,
    /// Some generator is still running.
    pub(crate) loading: bool,
    /// Local inline ghost text for this buffer (SPEC §6.11).
    pub(crate) ghost: Option<String>,
    /// Effective fuzzy mode for this reply.
    pub(crate) fuzzy: bool,
}

impl CompletionResult {
    /// Error reply: no suggestions, menu error-reset.
    pub(crate) fn error(req: u64, buffer: String, cursor: usize) -> CompletionResult {
        CompletionResult {
            req,
            phase: CompletionPhase::Parse,
            error: true,
            buffer,
            cursor,
            tokens: Vec::new(),
            command_text: String::new(),
            command_index: 0,
            arg_key: 0,
            current_arg: None,
            search_term: String::new(),
            triggered_generators: Vec::new(),
            items: Vec::new(),
            loading: false,
            ghost: None,
            fuzzy: false,
        }
    }
}
