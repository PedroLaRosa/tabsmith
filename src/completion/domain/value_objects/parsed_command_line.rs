use crate::completion::domain::value_objects::token_annotation::TokenAnnotation;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::parser_directives::ParserDirectives;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use std::rc::Rc;

/// Outcome of parsing the current command (SPEC §6.4 `result(st)`).
#[derive(Clone, Debug)]
pub(crate) struct ParsedCommandLine {
    /// The subcommand level the cursor is in.
    pub(crate) completion: Rc<Subcommand>,
    /// Root..=current chain of subcommands (for the `help` template).
    pub(crate) path: Vec<Rc<Subcommand>>,
    pub(crate) current_arg: Option<Rc<Arg>>,
    pub(crate) search_term: String,
    pub(crate) suggest_subcommands: bool,
    pub(crate) suggest_options: bool,
    pub(crate) suggest_args: bool,
    /// Options already passed at this level.
    pub(crate) passed_options: Vec<Rc<SpecOption>>,
    /// Persistent options inherited at this level, in lookup order (parent first: parent wins on name clash).
    pub(crate) persistent: Vec<Rc<SpecOption>>,
    pub(crate) annotations: Vec<TokenAnnotation>,
    /// Index of the token that names the spec in use (`sudo git ch` → 1).
    pub(crate) command_index: usize,
    /// Token texts after alias / `parserDirectives.alias` substitution.
    pub(crate) tokens: Vec<String>,
    /// Raw text of the last token as typed; for an option value inside the token (`--opt=val`, `-oval`) only
    /// the value part.
    pub(crate) raw_last_token: String,
    pub(crate) directives: ParserDirectives,
    /// The composite option chain being typed (`-la`), if the last annotation is one ending in an option.
    pub(crate) option_chain: Option<String>,
    /// Single-token command: suggest first tokens (SPEC §6.9) instead of spec items.
    pub(crate) first_token: bool,
    /// Name of the root spec (`git`), for recency keys and caches.
    pub(crate) spec_name: String,
}
