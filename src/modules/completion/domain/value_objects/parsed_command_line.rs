use crate::modules::completion::domain::value_objects::token_annotation::TokenAnnotation;
use crate::modules::specs::domain::entities::subcommand::Subcommand;
use crate::modules::specs::domain::value_objects::arg::Arg;
use crate::modules::specs::domain::value_objects::parser_directives::ParserDirectives;
use crate::modules::specs::domain::value_objects::spec_option::SpecOption;
use std::rc::Rc;

/// Outcome of parsing the current command (SPEC §6.4 `result(st)`).
#[derive(Clone, Debug)]
pub(crate) struct ParsedCommandLine {
    /// The subcommand level the cursor is in.
    completion: Rc<Subcommand>,
    /// Root..=current chain of subcommands (for the `help` template).
    path: Vec<Rc<Subcommand>>,
    current_arg: Option<Rc<Arg>>,
    search_term: String,
    suggest_subcommands: bool,
    suggest_options: bool,
    suggest_args: bool,
    /// Options already passed at this level.
    passed_options: Vec<Rc<SpecOption>>,
    /// Persistent options inherited at this level, in lookup order (parent first: parent wins on name clash).
    persistent: Vec<Rc<SpecOption>>,
    annotations: Vec<TokenAnnotation>,
    /// Index of the token that names the spec in use (`sudo git ch` → 1).
    command_index: usize,
    /// Token texts after alias / `parserDirectives.alias` substitution.
    tokens: Vec<String>,
    /// Raw text of the last token as typed; for an option value inside the token (`--opt=val`, `-oval`) only
    /// the value part.
    raw_last_token: String,
    directives: ParserDirectives,
    /// The composite option chain being typed (`-la`), if the last annotation is one ending in an option.
    option_chain: Option<String>,
    /// Single-token command: suggest first tokens (SPEC §6.9) instead of spec items.
    first_token: bool,
    /// Name of the root spec (`git`). No production code reads it; the parser tests assert it.
    #[cfg_attr(not(test), allow(dead_code, reason = "the parser tests assert which spec a parse resolved to"))]
    spec_name: String,
}

/// The fields of a ParsedCommandLine, by name.
pub(crate) struct ParsedCommandLineParts {
    pub(crate) completion: Rc<Subcommand>,
    pub(crate) path: Vec<Rc<Subcommand>>,
    pub(crate) current_arg: Option<Rc<Arg>>,
    pub(crate) search_term: String,
    pub(crate) suggest_subcommands: bool,
    pub(crate) suggest_options: bool,
    pub(crate) suggest_args: bool,
    pub(crate) passed_options: Vec<Rc<SpecOption>>,
    pub(crate) persistent: Vec<Rc<SpecOption>>,
    pub(crate) annotations: Vec<TokenAnnotation>,
    pub(crate) command_index: usize,
    pub(crate) tokens: Vec<String>,
    pub(crate) raw_last_token: String,
    pub(crate) directives: ParserDirectives,
    pub(crate) option_chain: Option<String>,
    pub(crate) first_token: bool,
    pub(crate) spec_name: String,
}

impl ParsedCommandLine {
    pub(crate) fn new(parts: ParsedCommandLineParts) -> Self {
        let ParsedCommandLineParts {
            completion,
            path,
            current_arg,
            search_term,
            suggest_subcommands,
            suggest_options,
            suggest_args,
            passed_options,
            persistent,
            annotations,
            command_index,
            tokens,
            raw_last_token,
            directives,
            option_chain,
            first_token,
            spec_name,
        } = parts;
        ParsedCommandLine {
            completion,
            path,
            current_arg,
            search_term,
            suggest_subcommands,
            suggest_options,
            suggest_args,
            passed_options,
            persistent,
            annotations,
            command_index,
            tokens,
            raw_last_token,
            directives,
            option_chain,
            first_token,
            spec_name,
        }
    }

    pub(crate) fn completion(&self) -> &Rc<Subcommand> {
        &self.completion
    }

    pub(crate) fn path(&self) -> &Vec<Rc<Subcommand>> {
        &self.path
    }

    pub(crate) fn current_arg(&self) -> &Option<Rc<Arg>> {
        &self.current_arg
    }

    pub(crate) fn search_term(&self) -> &str {
        &self.search_term
    }

    pub(crate) fn suggest_subcommands(&self) -> bool {
        self.suggest_subcommands
    }

    pub(crate) fn suggest_options(&self) -> bool {
        self.suggest_options
    }

    pub(crate) fn suggest_args(&self) -> bool {
        self.suggest_args
    }

    pub(crate) fn passed_options(&self) -> &Vec<Rc<SpecOption>> {
        &self.passed_options
    }

    pub(crate) fn persistent(&self) -> &Vec<Rc<SpecOption>> {
        &self.persistent
    }

    pub(crate) fn annotations(&self) -> &Vec<TokenAnnotation> {
        &self.annotations
    }

    pub(crate) fn command_index(&self) -> usize {
        self.command_index
    }

    pub(crate) fn tokens(&self) -> &Vec<String> {
        &self.tokens
    }

    pub(crate) fn raw_last_token(&self) -> &str {
        &self.raw_last_token
    }

    pub(crate) fn directives(&self) -> &ParserDirectives {
        &self.directives
    }

    pub(crate) fn option_chain(&self) -> &Option<String> {
        &self.option_chain
    }

    pub(crate) fn first_token(&self) -> bool {
        self.first_token
    }

    #[cfg(test)]
    pub(crate) fn spec_name(&self) -> &str {
        &self.spec_name
    }
}
