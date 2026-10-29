use crate::completion::domain::value_objects::filter_options::FilterOptions;
use crate::completion::domain::value_objects::insertion_context::InsertionContext;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLineParts;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
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

pub(crate) fn opts() -> FilterOptions {
    FilterOptions {
        fuzzy: false,
        auto_exec_hidden: false,
        exec_after_space: false,
        run_dangerous: false,
        suggest_current_token: false,
    }
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
                if s.kind == SuggestionKind::AutoExecute {
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
    InsertionContext {
        search_term: search,
        raw,
        shell,
        fuzzy: false,
        prefer_verbose: false,
        insert_space: true,
    }
}
