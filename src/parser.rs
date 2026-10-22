//! Spec loading (resolution, diff-versioned merge) and the argv state machine (SPEC §6.3–6.4).
//! OWNER: parser agent. `ParseResult` is consumed by the suggest agent — extend, don't rename.
use crate::js::{JsError, Js};
use crate::settings::Settings;
use crate::shell_parser::CommandInfo;
use crate::spec::{Arg, Directives, Opt, SpecLocation, Subcommand};
use crate::specs::SpecIndex;
use crate::types::Shell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnKind {
    Subcommand,
    Option,
    OptionArg,
    SubcommandArg,
    /// Token that could not be consumed (only possible for the final token).
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub kind: AnnKind,
    pub text: String,
    /// Sub-tokens of a composite token (`-abc`, `--opt=val`), else empty.
    pub parts: Vec<(AnnKind, String)>,
}

/// Outcome of parsing the current command (SPEC §6.4 `result(st)`).
#[derive(Clone, Debug)]
pub struct ParseResult {
    /// The subcommand level the cursor is in.
    pub completion: Rc<Subcommand>,
    /// Root..=current chain of subcommands (for the `help` template).
    pub path: Vec<Rc<Subcommand>>,
    pub current_arg: Option<Rc<Arg>>,
    pub search_term: String,
    pub suggest_subcommands: bool,
    pub suggest_options: bool,
    pub suggest_args: bool,
    /// Options already passed at this level.
    pub passed_options: Vec<Rc<Opt>>,
    /// Persistent options inherited at this level (parent wins on name clash).
    pub persistent: Vec<Rc<Opt>>,
    pub annotations: Vec<Annotation>,
    /// Index of the token that names the spec in use (`sudo git ch` → 1).
    pub command_index: usize,
    /// Token texts after alias / `parserDirectives.alias` substitution.
    pub tokens: Vec<String>,
    /// Raw text of the last token as typed.
    pub raw_last_token: String,
    pub directives: Directives,
    /// The composite option chain being typed (`-la`), if the last annotation is one ending in an option.
    pub option_chain: Option<String>,
    /// Single-token command: suggest first tokens (SPEC §6.9) instead of spec items.
    pub first_token: bool,
    /// Name of the root spec (`git`), for recency keys and caches.
    pub spec_name: String,
}

#[derive(Debug)]
pub enum ParseError {
    Aborted,
    /// No suggestions (unknown spec, unconsumable token, load failure). Logged by the caller.
    NoSuggestions(String),
}

impl From<JsError> for ParseError {
    fn from(e: JsError) -> Self {
        match e {
            JsError::Aborted => ParseError::Aborted,
            other => ParseError::NoSuggestions(other.to_string()),
        }
    }
}

/// Everything the parser needs from the session.
pub struct ParseCtx<'a> {
    pub cwd: &'a Path,
    pub settings: &'a Settings,
    pub env: &'a HashMap<String, String>,
    pub shell: Shell,
    pub index: &'a SpecIndex,
}

/// Spec loader + parse caches (engine thread).
pub struct Parser {
    // parser agent: spec cache by location, prefix-parse cache, version memo, generateSpec cache, …
    _private: (),
}

impl Parser {
    pub fn new() -> Parser {
        todo!("parser agent")
    }

    /// Drop caches (dev mode toggles, settings reload).
    pub fn clear_caches(&mut self) {
        todo!("parser agent")
    }

    /// Parse the command under the cursor against its spec (SPEC §6.4).
    pub fn parse(&mut self, cmd: &CommandInfo, ctx: &ParseCtx, js: &mut Js) -> Result<ParseResult, ParseError> {
        let _ = (cmd, ctx, js);
        todo!("parser agent")
    }

    /// Load (and cache) the spec at `loc`, applying diff versions (SPEC §6.3). `Ok(None)` when it does not exist
    /// or is disabled.
    pub fn load_spec(&mut self, loc: &SpecLocation, ctx: &ParseCtx, js: &mut Js) -> Result<Option<Rc<Subcommand>>, ParseError> {
        let _ = (loc, ctx, js);
        todo!("parser agent")
    }
}

/// Where the spec for `name` lives (SPEC §6.3 `spec_location`). `is_script`: `None` for the first token,
/// `Some(false)` for `isCommand` args, `Some(true)` for `isScript` args.
pub fn spec_location(name: &str, cwd: &Path, is_script: Option<bool>) -> SpecLocation {
    let _ = (name, cwd, is_script);
    todo!("parser agent")
}
