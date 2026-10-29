use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::spec_option::SpecOption;
use crate::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
use std::rc::Rc;

/// A suggestion row inside the pipeline (a spec's `SpecSuggestion` + internal fields), before it becomes a `Suggestion`.
#[derive(Clone, Debug)]
pub(crate) struct SuggestionCandidate {
    pub(crate) names: Vec<String>,
    pub(crate) display_name: Option<String>,
    pub(crate) insert_value: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) kind: SuggestionKind,
    pub(crate) original_kind: Option<SuggestionKind>,
    /// 0 = unset (`0` and unset are treated alike).
    pub(crate) priority: f64,
    pub(crate) hidden: bool,
    pub(crate) is_dangerous: bool,
    /// Args of the subcommand/option (auto-execute rule, args hint, dedupe).
    pub(crate) args: Vec<Rc<Arg>>,
    pub(crate) should_add_space: bool,
    pub(crate) separator: Option<String>,
    pub(crate) query: SuggestionQuery,
    pub(crate) generator: Option<GeneratorKey>,
    pub(crate) template_type: Option<String>,
    /// Spec `icon` (emoji or `fig://icon?type=`), resolved by the renderer.
    pub(crate) icon: Option<String>,
}

impl SuggestionCandidate {
    pub(crate) fn new(name: impl Into<String>, kind: SuggestionKind) -> SuggestionCandidate {
        SuggestionCandidate {
            names: vec![name.into()],
            display_name: None,
            insert_value: None,
            description: None,
            kind,
            original_kind: None,
            priority: 0.0,
            hidden: false,
            is_dangerous: false,
            args: Vec::new(),
            should_add_space: false,
            separator: None,
            query: SuggestionQuery::Term,
            generator: None,
            template_type: None,
            icon: None,
        }
    }

    pub(crate) fn first(&self) -> &str {
        self.names.first().map(String::as_str).unwrap_or("")
    }

    pub(crate) fn from_sub(s: &Subcommand) -> SuggestionCandidate {
        let mandatory = s.args().first().is_some_and(|a| !a.is_optional());
        SuggestionCandidate {
            names: s.names().clone(),
            display_name: s.display_name().clone(),
            insert_value: s.insert_value().clone(),
            description: s.description().clone(),
            icon: s.icon().clone(),
            priority: s.priority().unwrap_or(0.0),
            hidden: s.hidden(),
            is_dangerous: s.is_dangerous(),
            args: s.args().clone(),
            // SPEC §6.7.1 should_add_space
            should_add_space: mandatory
                || s.requires_subcommand().unwrap_or(!s.subcommands().is_empty()),
            ..SuggestionCandidate::new("", SuggestionKind::Subcommand)
        }
    }

    pub(crate) fn from_opt(o: &SpecOption, separators: &[String]) -> SuggestionCandidate {
        let separator = if o.args().first().is_some_and(|a| a.is_optional()) {
            None
        } else {
            match o.requires_separator() {
                Some(s) if !s.is_empty() => Some(s.clone()),
                Some(_) => Some(separators.first().cloned().unwrap_or_else(|| "=".into())),
                None => o.requires_equals().then(|| "=".to_string()),
            }
        };
        SuggestionCandidate {
            names: o.names().clone(),
            display_name: o.display_name().clone(),
            insert_value: o.insert_value().clone(),
            description: o.description().clone(),
            icon: o.icon().clone(),
            priority: o.priority().unwrap_or(0.0),
            hidden: o.hidden(),
            is_dangerous: o.is_dangerous(),
            args: o.args().clone(),
            should_add_space: o.args().first().is_some_and(|a| !a.is_optional()),
            separator,
            ..SuggestionCandidate::new("", SuggestionKind::Option)
        }
    }

    /// Static/additional/generator suggestion (`type` defaults to `arg`).
    pub(crate) fn from_suggestion(
        s: &SpecSuggestion,
        dangerous: bool,
        generator: Option<GeneratorKey>,
        query: SuggestionQuery,
    ) -> SuggestionCandidate {
        SuggestionCandidate {
            names: s.names().clone(),
            display_name: s.display_name().clone(),
            insert_value: s.insert_value().clone(),
            description: s.description().clone(),
            priority: s.priority().unwrap_or(0.0),
            hidden: s.hidden(),
            is_dangerous: s.is_dangerous() || dangerous,
            query,
            generator,
            template_type: s.template_type().clone(),
            icon: s.icon().clone(),
            ..SuggestionCandidate::new("", s.kind().unwrap_or(SuggestionKind::Arg))
        }
    }
}
