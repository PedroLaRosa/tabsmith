use crate::completion::domain::value_objects::argument_hint::ArgumentHint;
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::{CompletionResult, CompletionResultParts};
use crate::completion::domain::value_objects::filter_options::FilterOptions;
use crate::completion::domain::value_objects::insertion::{Insertion, InsertionParts};
use crate::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::completion::domain::value_objects::parsed_command_line::{ParsedCommandLine, ParsedCommandLineParts};
use crate::completion::domain::value_objects::served_request::{ServedRequest, ServedRequestParts};
use crate::completion::domain::value_objects::suggestion::{Suggestion, SuggestionParts};
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::settings::domain::value_objects::settings::Settings;
#[cfg(test)]
use crate::settings::infrastructure::adapters::setting_value_json::settings_from_json;
use crate::shared::domain::value_objects::shell::Shell;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::parser_directives::ParserDirectives;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::rc::Rc;

pub(crate) fn pr_for(comp: Subcommand, search: &str) -> ParsedCommandLineParts {
    let comp = Rc::new(comp);
    ParsedCommandLineParts {
        completion: comp.clone(),
        path: vec![comp],
        current_arg: None,
        search_term: search.into(),
        suggest_subcommands: true,
        suggest_options: true,
        suggest_args: true,
        passed_options: vec![],
        persistent: vec![],
        annotations: vec![],
        command_index: 0,
        tokens: vec![],
        raw_last_token: search.into(),
        directives: ParserDirectives::default(),
        option_chain: None,
        first_token: false,
        spec_name: "cmd".into(),
    }
}

/// A request served for `cmd ` (a spec without arguments, cursor at the end), for a session to hold.
pub(crate) fn served_request() -> ServedRequest {
    ServedRequest::new(ServedRequestParts {
        request_id: 1,
        buffer: "cmd ".into(),
        shell: Shell::Bash,
        history_mode: false,
        cursor: 4,
        parsed: ParsedCommandLine::new(pr_for(Subcommand::default(), "")),
        command_text: "cmd ".into(),
        argument_key: 0,
        fuzzy: false,
        queries: Vec::new(),
        triggered_generators: Vec::new(),
        first_token_candidates: Vec::new(),
        history_prefix: "cmd ".into(),
    })
}

pub(crate) fn opts() -> FilterOptions {
    FilterOptions::default()
}

pub(crate) fn sub(names: &[&str]) -> Rc<Subcommand> {
    Rc::new(Subcommand::default().with_names(names.iter().map(|s| s.to_string()).collect()))
}

pub(crate) fn opt(names: &[&str]) -> SpecOption {
    SpecOption::default().with_names(names.iter().map(|s| s.to_string()).collect())
}

pub(crate) fn arg(optional: bool) -> Rc<Arg> {
    Rc::new(Arg::default().with_name(Some("x".into())).with_is_optional(optional))
}

pub(crate) fn names(v: &[(SuggestionCandidate, bool)]) -> Vec<String> {
    v.iter()
        .map(|(s, _)| {
            format!(
                "{}{}",
                if s.kind() == SuggestionKind::AutoExecute {
                    "!"
                } else {
                    ""
                },
                s.first()
            )
        })
        .collect()
}

pub(crate) fn ctx<'a>(search: &'a str, raw: &'a str, shell: Shell) -> InsertionContext<'a> {
    InsertionContext::new(search, raw, shell).with_insert_space(true)
}

pub(crate) fn item(name: &str, kind: SuggestionKind, query: &str) -> Suggestion {
    let typed = name.starts_with(query) && !query.is_empty();
    Suggestion::new(SuggestionParts {
        names: vec![name.to_string()],
        display_name: None,
        description: Some(format!("about {name}")),
        kind,
        original_kind: None,
        args_hint: String::new(),
        highlight: (0..query.chars().count()).collect(),
        query: query.to_string(),
        insert: Insertion::new(InsertionParts {
            delete: if typed { 0 } else { query.chars().count() },
            text: if typed {
                name[query.len()..].to_string()
            } else {
                name.to_string()
            },
            cursor_back: 0,
            execute: false,
        }),
        generator: None,
        prefix_match: true,
        icon: None,
    })
}

/// A Parse reply for `buffer` (tokens split on spaces, a trailing space adds an empty token).
pub(crate) fn comp(buffer: &str, arg_key: u64, items: Vec<Suggestion>) -> CompletionResult {
    let tokens: Vec<String> = buffer.split(' ').map(String::from).collect();
    CompletionResult::new(CompletionResultParts {
        req: 1,
        phase: CompletionPhase::Parse,
        error: false,
        buffer: buffer.to_string(),
        cursor: buffer.len(),
        search_term: tokens.last().cloned().unwrap_or_default(),
        tokens,
        command_text: buffer.to_string(),
        command_index: 0,
        arg_key,
        current_arg: Some(ArgumentHint::new("path".into(), "A path".into())),
        triggered_generators: Vec::new(),
        items,
        loading: false,
        ghost: None,
    })
}

#[cfg(test)]
pub(crate) fn settings(json: &str) -> Settings {
    match serde_json::from_str(json).unwrap() {
        serde_json::Value::Object(m) => settings_from_json(m),
        _ => unreachable!(),
    }
}
