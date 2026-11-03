use crate::completion::domain::value_objects::argument_hint::ArgumentHint;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::suggestion::Suggestion;

/// Engine → wrapper reply (SPEC §4.3).
#[derive(Clone, Debug)]
pub(crate) struct CompletionResult {
    req: u64,
    phase: CompletionPhase,
    error: bool,
    /// The buffer/cursor the reply was computed from (for insertion reconciliation, SPEC §7.3).
    buffer: String,
    cursor: usize,
    /// Token texts of the current command (after alias expansion).
    tokens: Vec<String>,
    command_text: String,
    command_index: usize,
    /// Identity of (current arg, current subcommand) — changes when a new argument starts.
    arg_key: u64,
    current_arg: Option<ArgumentHint>,
    search_term: String,
    triggered_generators: Vec<GeneratorKey>,
    items: Vec<Suggestion>,
    /// Some generator is still running.
    loading: bool,
    /// Local inline ghost text for this buffer (SPEC §6.11).
    ghost: Option<String>,
}

/// What a `CompletionResult` is built from: named fields, so two counts or flags cannot be swapped.
pub(crate) struct CompletionResultParts {
    pub(crate) req: u64,
    pub(crate) phase: CompletionPhase,
    pub(crate) error: bool,
    pub(crate) buffer: String,
    pub(crate) cursor: usize,
    pub(crate) tokens: Vec<String>,
    pub(crate) command_text: String,
    pub(crate) command_index: usize,
    pub(crate) arg_key: u64,
    pub(crate) current_arg: Option<ArgumentHint>,
    pub(crate) search_term: String,
    pub(crate) triggered_generators: Vec<GeneratorKey>,
    pub(crate) items: Vec<Suggestion>,
    pub(crate) loading: bool,
    pub(crate) ghost: Option<String>,
}

impl CompletionResult {
    pub(crate) fn new(parts: CompletionResultParts) -> Self {
        let CompletionResultParts {
            req,
            phase,
            error,
            buffer,
            cursor,
            tokens,
            command_text,
            command_index,
            arg_key,
            current_arg,
            search_term,
            triggered_generators,
            items,
            loading,
            ghost,
        } = parts;
        CompletionResult {
            req,
            phase,
            error,
            buffer,
            cursor,
            tokens,
            command_text,
            command_index,
            arg_key,
            current_arg,
            search_term,
            triggered_generators,
            items,
            loading,
            ghost,
        }
    }

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
        }
    }

    pub(crate) fn request_id(&self) -> u64 {
        self.req
    }

    pub(crate) fn phase(&self) -> CompletionPhase {
        self.phase
    }

    pub(crate) fn is_error(&self) -> bool {
        self.error
    }

    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(crate) fn tokens(&self) -> &[String] {
        &self.tokens
    }

    pub(crate) fn command_text(&self) -> &str {
        &self.command_text
    }

    pub(crate) fn command_index(&self) -> usize {
        self.command_index
    }

    pub(crate) fn arg_key(&self) -> u64 {
        self.arg_key
    }

    pub(crate) fn current_arg(&self) -> Option<&ArgumentHint> {
        self.current_arg.as_ref()
    }

    pub(crate) fn search_term(&self) -> &str {
        &self.search_term
    }

    pub(crate) fn triggered_generators(&self) -> &[GeneratorKey] {
        &self.triggered_generators
    }

    pub(crate) fn suggestions(&self) -> &[Suggestion] {
        &self.items
    }

    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(crate) fn ghost(&self) -> Option<&str> {
        self.ghost.as_deref()
    }

    /// Moves the suggestions out and keeps the rest of the reply: the menu reads it after its items left.
    pub(crate) fn take_suggestions(&mut self) -> Vec<Suggestion> {
        std::mem::take(&mut self.items)
    }

    pub(crate) fn with_ghost(mut self, ghost: Option<String>) -> Self {
        self.ghost = ghost;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_request_id(mut self, req: u64) -> Self {
        self.req = req;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_phase(mut self, phase: CompletionPhase) -> Self {
        self.phase = phase;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_triggered_generators(mut self, triggered_generators: Vec<GeneratorKey>) -> Self {
        self.triggered_generators = triggered_generators;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_arg_key(mut self, arg_key: u64) -> Self {
        self.arg_key = arg_key;
        self
    }
}
