use crate::completion::domain::entities::generator_cache::GeneratorCache;
use crate::completion::domain::entities::generator_run::GeneratorRun;
use crate::completion::domain::entities::generator_slot::GeneratorSlot;
use crate::completion::domain::entities::parse_prefix_cache::ParsePrefixCache;
use crate::completion::domain::entities::suggestion_recency::SuggestionRecency;
use crate::completion::domain::value_objects::alias_table::{parse_aliases, AliasTable};
use crate::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::completion::domain::value_objects::generator_outcome::GeneratorOutcome;
use crate::completion::domain::value_objects::generator_slot_view::GeneratorSlotView;
use crate::completion::domain::value_objects::served_request::ServedRequest;
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_FUZZY_SEARCH;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::domain::value_objects::shell::Shell;
use crate::specs::domain::value_objects::arg::Arg;
use crate::specs::domain::value_objects::spec_value::SpecValue;
use crate::specs::domain::value_objects::task_id::TaskId;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

const FIRST_TOKEN_TTL: Duration = Duration::from_secs(10);

/// What the completion engine keeps between requests: the settings and the shell's env and aliases, the recency,
/// the parse caches, the generators of the current argument and the latest served request.
pub(crate) struct CompletionSession {
    settings: Settings,
    wrapper_env: HashMap<String, String>,
    session_env: HashMap<String, String>,
    session_alias: Option<String>,
    aliases: Option<(Shell, AliasTable)>,
    recency: SuggestionRecency,
    user_fuzzy: bool,
    last_arg_key: u64,
    parse_prefix_cache: ParsePrefixCache,
    slot_arg: Option<Rc<Arg>>,
    slots: Vec<GeneratorSlot>,
    prev_search: String,
    next_gen: u64,
    runs: Vec<GeneratorRun>,
    cache: GeneratorCache<SpecValue>,
    served: Option<ServedRequest>,
    first_token_cache: Option<(Instant, Vec<SuggestionCandidate>)>,
}

impl CompletionSession {
    pub(crate) fn new(settings: Settings, wrapper_env: HashMap<String, String>, recency: SuggestionRecency) -> Self {
        CompletionSession {
            user_fuzzy: settings.bool(AUTOCOMPLETE_FUZZY_SEARCH),
            settings,
            wrapper_env,
            session_env: HashMap::new(),
            session_alias: None,
            aliases: None,
            recency,
            last_arg_key: 0,
            parse_prefix_cache: ParsePrefixCache::default(),
            slot_arg: None,
            slots: Vec::new(),
            prev_search: String::new(),
            next_gen: 0,
            runs: Vec::new(),
            cache: GeneratorCache::default(),
            served: None,
            first_token_cache: None,
        }
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub(crate) fn replace_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// The env of the shell's latest prompt; an empty one means the shell reported none: the wrapper's env.
    pub(crate) fn env(&self) -> &HashMap<String, String> {
        reported_env(&self.session_env, &self.wrapper_env)
    }

    /// The aliases the shell reported, parsed once per shell.
    pub(crate) fn alias_map(&mut self, shell: Shell) -> AliasTable {
        if !matches!(&self.aliases, Some((s, _)) if *s == shell) {
            let map = match self.session_alias.as_deref() {
                Some(text) if !text.is_empty() => parse_aliases(text, shell),
                _ => HashMap::new(),
            };
            self.aliases = Some((shell, map));
        }
        self.aliases
            .as_ref()
            .map(|(_, m)| m.clone())
            .unwrap_or_default()
    }

    pub(crate) fn apply_session_context(&mut self, env: HashMap<String, String>, alias: Option<String>) {
        self.session_env = env;
        self.session_alias = alias;
        self.aliases = None;
        self.first_token_cache = None; // aliases / PATH from this prompt
    }

    pub(crate) fn toggle_fuzzy_search(&mut self) {
        self.user_fuzzy = !self.user_fuzzy;
    }

    /// The request's argument is `argument_key`: a new one resets the fuzzy search to the setting.
    pub(crate) fn enter_argument(&mut self, argument_key: u64) {
        if argument_key != self.last_arg_key {
            self.user_fuzzy = self.settings.bool(AUTOCOMPLETE_FUZZY_SEARCH);
            self.last_arg_key = argument_key;
        }
    }

    pub(crate) fn user_fuzzy(&self) -> bool {
        self.user_fuzzy
    }

    /// The parse's cache, settings and env, lent together.
    pub(crate) fn parse_inputs(&mut self) -> (&mut ParsePrefixCache, &Settings, &HashMap<String, String>) {
        let env = reported_env(&self.session_env, &self.wrapper_env);
        (&mut self.parse_prefix_cache, &self.settings, env)
    }

    pub(crate) fn parse_prefix_cache(&mut self) -> &mut ParsePrefixCache {
        &mut self.parse_prefix_cache
    }

    pub(crate) fn serve(&mut self, served: ServedRequest) {
        self.served = Some(served);
    }

    pub(crate) fn served(&self) -> Option<&ServedRequest> {
        self.served.as_ref()
    }

    pub(crate) fn forget_served_request(&mut self) {
        self.served = None;
    }

    /// The first-token rows `list` gave less than ten seconds ago (by `now`), else what it gives now.
    pub(crate) fn first_token_suggestions(
        &mut self,
        now: &dyn Fn() -> Instant,
        list: impl FnOnce() -> Vec<SuggestionCandidate>,
    ) -> Vec<SuggestionCandidate> {
        if let Some((at, items)) = &self.first_token_cache {
            if now().duration_since(*at) < FIRST_TOKEN_TTL {
                return items.clone();
            }
        }
        let items = list();
        self.first_token_cache = Some((now(), items.clone()));
        items
    }

    pub(crate) fn forget_first_token_suggestions(&mut self) {
        self.first_token_cache = None;
    }

    pub(crate) fn recency(&self) -> &SuggestionRecency {
        &self.recency
    }

    /// Records the insertion of `name` for `command`; `true` when the recency changed (and is due a save).
    pub(crate) fn record_insertion(&mut self, command: String, name: String, at_ms: u64) -> bool {
        self.recency.record(command, name, at_ms)
    }

    /// Forgets the generators of the current argument and the search term they fired for.
    pub(crate) fn reset_generator_slots(&mut self) {
        self.slots.clear();
        self.slot_arg = None;
        self.prev_search.clear();
    }

    /// No current argument: no generator slots.
    pub(crate) fn clear_generator_slots(&mut self) {
        self.slots.clear();
        self.slot_arg = None;
    }

    /// A new current argument: one empty slot per generator key.
    pub(crate) fn assign_generator_slots(&mut self, argument: Rc<Arg>, keys: Vec<GeneratorKey>) {
        self.slots = keys.into_iter().map(GeneratorSlot::new).collect();
        self.slot_arg = Some(argument);
    }

    pub(crate) fn slot_argument(&self) -> Option<&Rc<Arg>> {
        self.slot_arg.as_ref()
    }

    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn previous_search_term(&self) -> &str {
        &self.prev_search
    }

    pub(crate) fn remember_search_term(&mut self, search_term: String) {
        self.prev_search = search_term;
    }

    /// Fires the generator of slot `index` as the next generation: that generation and the slot's key.
    pub(crate) fn fire_slot(&mut self, index: usize) -> (u64, GeneratorKey) {
        self.next_gen += 1;
        let slot = &mut self.slots[index];
        slot.fire(self.next_gen);
        (self.next_gen, slot.key())
    }

    pub(crate) fn is_current(&self, run: &GeneratorRun) -> bool {
        self.slots
            .get(run.slot())
            .is_some_and(|s| s.generation() == run.generation())
    }

    /// Record an outcome; returns whether a current slot changed.
    pub(crate) fn settle(&mut self, run: GeneratorRun, outcome: GeneratorOutcome) -> bool {
        let items = match outcome {
            GeneratorOutcome::Pending => {
                self.runs.push(run);
                return false;
            }
            GeneratorOutcome::Done(items) => items,
            GeneratorOutcome::Fail(_) => {
                if run.is_background() {
                    return false;
                }
                Vec::new() // any error → [] (never cached)
            }
        };
        self.slots
            .get_mut(run.slot())
            .is_some_and(|s| s.finish(run.generation(), items))
    }

    pub(crate) fn push_run(&mut self, run: GeneratorRun) {
        self.runs.push(run);
    }

    pub(crate) fn runs(&self) -> &[GeneratorRun] {
        &self.runs
    }

    pub(crate) fn take_runs(&mut self) -> Vec<GeneratorRun> {
        std::mem::take(&mut self.runs)
    }

    /// Keeps `runs` after the runs pushed since `take_runs`.
    pub(crate) fn keep_runs(&mut self, runs: Vec<GeneratorRun>) {
        self.runs.extend(runs);
    }

    pub(crate) fn has_run_on(&self, task: TaskId) -> bool {
        self.runs.iter().any(|r| r.task() == task)
    }

    pub(crate) fn has_running_generators(&self) -> bool {
        !self.runs.is_empty()
    }

    /// Each slot with the query of its generator (the shorter of the two decides how many).
    pub(crate) fn slot_views(&self, queries: &[SuggestionQuery]) -> Vec<GeneratorSlotView<'_>> {
        self.slots
            .iter()
            .zip(queries)
            .map(|(sl, q)| GeneratorSlotView::new(sl.key(), q.clone(), sl.is_loading(), sl.result()))
            .collect()
    }

    pub(crate) fn any_slot_loading(&self) -> bool {
        self.slots.iter().any(|s| s.is_loading())
    }

    pub(crate) fn generator_cache_mut(&mut self) -> &mut GeneratorCache<SpecValue> {
        &mut self.cache
    }
}

/// GEN-GT38: an empty session env means the shell reported none, so the wrapper's env stands in. A free function
/// over the two fields so `parse_inputs` can lend it beside the mutably borrowed parse cache.
fn reported_env<'a>(
    session_env: &'a HashMap<String, String>,
    wrapper_env: &'a HashMap<String, String>,
) -> &'a HashMap<String, String> {
    if session_env.is_empty() {
        wrapper_env
    } else {
        session_env
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use std::cell::Cell;

    fn session(settings: Settings) -> CompletionSession {
        CompletionSession::new(settings, HashMap::new(), SuggestionRecency::default())
    }

    fn names(rows: &[SuggestionCandidate]) -> Vec<&str> {
        rows.iter().map(|row| row.names()[0].as_str()).collect()
    }

    #[test]
    fn should_keep_first_token_suggestions_for_ten_seconds() {
        let base = Instant::now();
        let offset = Cell::new(Duration::ZERO);
        let now = || base + offset.get();
        let lists = Cell::new(0);
        let list = |name: &str| {
            lists.set(lists.get() + 1);
            vec![SuggestionCandidate::new(name, SuggestionKind::Special)]
        };
        let mut session = session(Settings::default());
        session.first_token_suggestions(&now, || list("git"));

        offset.set(Duration::from_millis(9_999));
        let kept = session.first_token_suggestions(&now, || list("ls"));
        offset.set(Duration::from_secs(10));
        let listed = session.first_token_suggestions(&now, || list("ls"));

        assert_eq!((names(&kept), names(&listed), lists.get()), (vec!["git"], vec!["ls"], 2));
    }

    #[test]
    fn should_reset_the_fuzzy_search_to_the_setting_for_a_new_argument() {
        let mut session = session(Settings::default().with(AUTOCOMPLETE_FUZZY_SEARCH, SettingValue::Bool(true)));
        session.enter_argument(1);
        session.toggle_fuzzy_search();
        session.enter_argument(1);
        let toggled = session.user_fuzzy();

        session.enter_argument(2);

        assert_eq!((toggled, session.user_fuzzy()), (false, true));
    }
}
