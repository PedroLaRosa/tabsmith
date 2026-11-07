use crate::modules::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::modules::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::modules::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::modules::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::modules::shared::domain::value_objects::shell::Shell;

/// The latest served request (for phase-Generators replies).
pub(crate) struct ServedRequest {
    request_id: u64,
    buffer: String,
    shell: Shell,
    history_mode: bool,
    cursor: usize,
    parsed: ParsedCommandLine,
    command_text: String,
    argument_key: u64,
    fuzzy: bool,
    queries: Vec<SuggestionQuery>,
    triggered_generators: Vec<GeneratorKey>,
    first_token_candidates: Vec<SuggestionCandidate>,
    /// Buffer up to the start of the last token (history rows).
    history_prefix: String,
}

/// What a `ServedRequest` is built from: named fields, so two keys or flags cannot be swapped.
pub(crate) struct ServedRequestParts {
    pub(crate) request_id: u64,
    pub(crate) buffer: String,
    pub(crate) shell: Shell,
    pub(crate) history_mode: bool,
    /// The request's cursor, clamped to the buffer.
    pub(crate) cursor: usize,
    pub(crate) parsed: ParsedCommandLine,
    pub(crate) command_text: String,
    pub(crate) argument_key: u64,
    pub(crate) fuzzy: bool,
    pub(crate) queries: Vec<SuggestionQuery>,
    pub(crate) triggered_generators: Vec<GeneratorKey>,
    pub(crate) first_token_candidates: Vec<SuggestionCandidate>,
    pub(crate) history_prefix: String,
}

impl ServedRequest {
    pub(crate) fn new(parts: ServedRequestParts) -> Self {
        let ServedRequestParts {
            request_id,
            buffer,
            shell,
            history_mode,
            cursor,
            parsed,
            command_text,
            argument_key,
            fuzzy,
            queries,
            triggered_generators,
            first_token_candidates,
            history_prefix,
        } = parts;
        ServedRequest {
            request_id,
            buffer,
            shell,
            history_mode,
            cursor,
            parsed,
            command_text,
            argument_key,
            fuzzy,
            queries,
            triggered_generators,
            first_token_candidates,
            history_prefix,
        }
    }

    pub(crate) fn request_id(&self) -> u64 {
        self.request_id
    }

    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    pub(crate) fn shell(&self) -> Shell {
        self.shell
    }

    pub(crate) fn history_mode(&self) -> bool {
        self.history_mode
    }

    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    pub(crate) fn parsed(&self) -> &ParsedCommandLine {
        &self.parsed
    }

    pub(crate) fn command_text(&self) -> &str {
        &self.command_text
    }

    pub(crate) fn argument_key(&self) -> u64 {
        self.argument_key
    }

    pub(crate) fn fuzzy(&self) -> bool {
        self.fuzzy
    }

    pub(crate) fn queries(&self) -> &Vec<SuggestionQuery> {
        &self.queries
    }

    pub(crate) fn triggered_generators(&self) -> &Vec<GeneratorKey> {
        &self.triggered_generators
    }

    pub(crate) fn first_token_candidates(&self) -> &Vec<SuggestionCandidate> {
        &self.first_token_candidates
    }

    pub(crate) fn history_prefix(&self) -> &str {
        &self.history_prefix
    }
}
