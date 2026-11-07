use crate::modules::completion::application::completion_notice::CompletionNotice;
use crate::modules::completion::application::generator_runner::GeneratorRunner;
use crate::modules::completion::domain::entities::completion_session::CompletionSession;
use crate::modules::completion::domain::services::completion_result_builder::build_completion_result;
use crate::modules::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::modules::completion::domain::value_objects::completion_result::CompletionResult;
use crate::modules::history::domain::repositories::history_repository::HistoryRepository;
use std::rc::Rc;

/// Waits one slice on the running generators and replies again (phase `Generators`) when a slot of the current
/// argument changed while a request is served. The reply carries no ghost text (Q15); a newer request interrupting the
/// wait leaves no reply, and the runs, the cache and the tasks as they were.
pub(crate) struct ProgressGeneratorsUseCase {
    generators: Rc<GeneratorRunner>,
    history: Rc<dyn HistoryRepository>,
}

impl ProgressGeneratorsUseCase {
    pub(crate) fn new(generators: Rc<GeneratorRunner>, history: Rc<dyn HistoryRepository>) -> Self {
        ProgressGeneratorsUseCase { generators, history }
    }

    pub(crate) fn execute(
        &self,
        session: &mut CompletionSession,
        report: &mut dyn FnMut(CompletionNotice),
    ) -> Option<CompletionResult> {
        if self.generators.progress(session, report) == Some(true) && session.served().is_some() {
            return Some(build_completion_result(session, CompletionPhase::Generators, &self.history.recent()));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::application::test_fixtures::{request, CompletionDoubles};
    use crate::modules::settings::domain::services::settings_catalog::AUTOCOMPLETE_SCRIPT_TIMEOUT;
    use crate::modules::settings::domain::value_objects::setting_number::SettingNumber;
    use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use crate::modules::shared::application::ports::clock::Clock;
    use crate::modules::specs::application::ports::spec_runtime::InMemorySpecRuntime;
    use crate::modules::specs::domain::entities::subcommand::Subcommand;
    use crate::modules::specs::domain::repositories::spec_repository::InMemorySpecRepository;
    use crate::modules::specs::domain::value_objects::arg::Arg;
    use crate::modules::specs::domain::value_objects::generator::Generator;
    use crate::modules::specs::domain::value_objects::generator_cache_policy::GeneratorCachePolicy;
    use crate::modules::specs::domain::value_objects::generator_script::GeneratorScript;
    use crate::modules::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
    use crate::modules::specs::domain::value_objects::spec_index::SpecIndex;
    use crate::modules::specs::domain::value_objects::spec_value::SpecValue;
    use std::collections::BTreeMap;
    use std::time::Duration;

    /// The custom function of the first generator.
    const F: SpecFunctionRef = 1;
    /// The custom function of the second generator.
    const G: SpecFunctionRef = 2;

    /// The spec of a command `tool` whose first argument is `argument`.
    fn tool(argument: Arg) -> InMemorySpecRepository {
        let tool = Rc::new(Subcommand::default().with_names(vec!["tool".into()]).with_args(vec![Rc::new(argument)]));
        InMemorySpecRepository::with(&[("tool", &tool)])
    }

    fn custom(function: SpecFunctionRef) -> Generator {
        Generator::default().with_custom(Some(function))
    }

    /// A custom generator cached under the key `k` for `ttl_ms`, stale-while-revalidate past it.
    fn cached(function: SpecFunctionRef, ttl_ms: u64) -> Generator {
        let policy = GeneratorCachePolicy::default().with_ttl_ms(ttl_ms).with_cache_key(Some("k".into()));
        custom(function).with_cache(Some(policy))
    }

    /// Completes `tool ` with the cursor at its end as request `id`.
    fn complete(doubles: &CompletionDoubles, session: &mut CompletionSession, id: u64) -> CompletionResult {
        doubles
            .complete_command_line(SpecIndex::default())
            .execute(session, &request(id, "tool "), &mut |_| {})
            .unwrap()
    }

    fn texts(values: &[&str]) -> SpecValue {
        SpecValue::List(values.iter().map(|value| SpecValue::Text(value.to_string())).collect())
    }

    /// The names of the rows, sorted: ranking is not what these tests pin.
    fn names(result: &CompletionResult) -> Vec<String> {
        let mut names: Vec<String> = result.suggestions().iter().map(|row| row.first_name().to_string()).collect();
        names.sort();
        names
    }

    fn owned(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn should_reply_again_with_the_rows_of_a_finished_generator() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![custom(F)])),
            InMemorySpecRuntime::default().completing(F, Ok(texts(&["main", "dev"]))),
        );
        let mut session = doubles.session(Settings::default());
        doubles
            .complete_command_line(SpecIndex::default())
            .execute(&mut session, &request(1, "tool "), &mut |_| {});

        let result = doubles.progress_generators().execute(&mut session, &mut |_| {}).unwrap();

        assert_eq!(
            (result.phase(), names(&result), result.is_loading()),
            (CompletionPhase::Generators, owned(&["dev", "main"]), false)
        );
    }

    #[test]
    fn should_not_reply_while_no_generator_of_the_current_argument_changed() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![custom(F)])),
            InMemorySpecRuntime::default(),
        );
        let mut session = doubles.session(Settings::default());
        complete(&doubles, &mut session, 1);

        let result = doubles.progress_generators().execute(&mut session, &mut |_| {});

        assert_eq!((result.is_none(), session.has_running_generators()), (true, true));
    }

    #[test]
    fn should_reply_nothing_and_keep_the_runs_when_a_newer_request_interrupts() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![custom(F)])),
            InMemorySpecRuntime::default().completing(F, Ok(texts(&["main"]))).interrupting(),
        );
        let mut session = doubles.session(Settings::default());
        complete(&doubles, &mut session, 1);

        let result = doubles.progress_generators().execute(&mut session, &mut |_| {});

        assert_eq!((result.is_none(), session.runs().len()), (true, 1));
    }

    #[test]
    fn should_wait_for_the_debounce_before_running_a_generator() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_debounce(Some(100)).with_generators(vec![custom(F)])),
            InMemorySpecRuntime::default()
                .completing(F, Ok(texts(&["main", "dev"])))
                .finishing_sleeps(),
        );
        let progress = doubles.progress_generators();
        let mut session = doubles.session(Settings::default());
        complete(&doubles, &mut session, 1);

        let after_the_debounce = progress.execute(&mut session, &mut |_| {});
        let after_the_call = progress.execute(&mut session, &mut |_| {});

        assert_eq!(
            (after_the_debounce.is_none(), after_the_call.map(|result| names(&result))),
            (true, Some(owned(&["dev", "main"])))
        );
    }

    #[test]
    fn should_serve_a_fresh_cached_result_without_starting_a_task() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![cached(F, 60000)])),
            InMemorySpecRuntime::default().completing(F, Ok(texts(&["main", "dev"]))),
        );
        let mut session = doubles.session(Settings::default());
        complete(&doubles, &mut session, 1);
        doubles.progress_generators().execute(&mut session, &mut |_| {});
        session.reset_generator_slots();

        let result = complete(&doubles, &mut session, 2);

        assert_eq!(
            (result.phase(), names(&result), result.is_loading(), session.has_running_generators()),
            (CompletionPhase::Parse, owned(&["dev", "main"]), false, false)
        );
    }

    #[test]
    fn should_serve_a_stale_result_and_refresh_it_in_the_background() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![cached(F, 1000)])),
            InMemorySpecRuntime::default(),
        );
        let mut session = doubles.session(Settings::default());
        session.generator_cache_mut().store(",k", texts(&["old"]), doubles.clock.now_ms() - 1001);

        let result = complete(&doubles, &mut session, 1);

        assert_eq!(
            (result.phase(), names(&result), result.is_loading(), session.has_running_generators()),
            (CompletionPhase::Parse, owned(&["old"]), false, true)
        );
    }

    #[test]
    fn should_share_the_task_of_an_identical_fetch_in_flight() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![cached(F, 60000), cached(G, 60000)])),
            InMemorySpecRuntime::default().completing(F, Ok(texts(&["main"]))),
        );
        let mut session = doubles.session(Settings::default());
        complete(&doubles, &mut session, 1);

        let result = doubles.progress_generators().execute(&mut session, &mut |_| {}).unwrap();

        assert_eq!((result.is_loading(), session.has_running_generators()), (false, false));
    }

    #[test]
    fn should_keep_a_failed_background_refresh_silent() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![cached(F, 1000)])),
            InMemorySpecRuntime::default().completing(F, Err("boom".into())),
        );
        let mut session = doubles.session(Settings::default());
        session.generator_cache_mut().store(",k", texts(&["old"]), doubles.clock.now_ms() - 1001);
        complete(&doubles, &mut session, 1);
        let mut notices = Vec::new();

        let result = doubles.progress_generators().execute(&mut session, &mut |notice| notices.push(notice));

        assert_eq!(
            (result.is_none(), notices),
            (true, vec![CompletionNotice::GeneratorFailed("boom".into())])
        );
    }

    #[test]
    fn should_cache_the_value_of_a_superseded_run() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![cached(F, 60000)])),
            InMemorySpecRuntime::default().completing(F, Ok(texts(&["main"]))),
        );
        let mut session = doubles.session(Settings::default());
        complete(&doubles, &mut session, 1);
        session.reset_generator_slots();
        doubles.progress_generators().execute(&mut session, &mut |_| {});

        let result = complete(&doubles, &mut session, 2);

        assert_eq!(
            (result.phase(), names(&result), result.is_loading(), session.has_running_generators()),
            (CompletionPhase::Parse, owned(&["main"]), false, false)
        );
    }

    #[test]
    fn should_fail_an_expired_run_with_timed_out() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![custom(F)])),
            InMemorySpecRuntime::default(),
        );
        let mut session = doubles.session(Settings::default());
        complete(&doubles, &mut session, 1);
        doubles.clock.advance(Duration::from_secs(60));
        let mut notices = Vec::new();

        let result = doubles.progress_generators().execute(&mut session, &mut |notice| notices.push(notice));

        assert_eq!(
            (notices, result.map(|result| result.is_loading()), session.has_running_generators()),
            (vec![CompletionNotice::GeneratorFailed("timed out".into())], Some(false), false)
        );
    }

    #[test]
    fn should_report_every_failed_generator() {
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![custom(F), custom(G)])),
            InMemorySpecRuntime::default()
                .completing(F, Err("one".into()))
                .completing(G, Err("two".into())),
        );
        let mut session = doubles.session(Settings::default());
        complete(&doubles, &mut session, 1);
        let mut notices = Vec::new();

        let result = doubles.progress_generators().execute(&mut session, &mut |notice| notices.push(notice));

        assert_eq!(
            (notices, result.map(|result| result.is_loading())),
            (
                vec![CompletionNotice::GeneratorFailed("one".into()), CompletionNotice::GeneratorFailed("two".into())],
                Some(false)
            )
        );
    }

    /// The floor is set explicitly: its default depends on the platform.
    #[test]
    fn should_run_a_script_with_the_largest_timeout_of_floor_generator_and_request() {
        let input = SpecValue::Object(BTreeMap::from([
            ("command".to_string(), SpecValue::Text("x".into())),
            ("timeout".to_string(), SpecValue::Unsigned(7000)),
        ]));
        let script = Generator::default()
            .with_script(Some(GeneratorScript::Object(input)))
            .with_script_timeout(Some(4000));
        let doubles = CompletionDoubles::new(
            tool(Arg::default().with_generators(vec![script])),
            InMemorySpecRuntime::default(),
        );
        let floor = SettingValue::Number(SettingNumber::from(3000u64));
        let mut session = doubles.session(Settings::default().with(AUTOCOMPLETE_SCRIPT_TIMEOUT, floor));

        complete(&doubles, &mut session, 1);

        let timeouts: Vec<Option<u64>> =
            doubles.runtime.exec_requests().iter().map(|started| started.timeout()).collect();
        assert_eq!(timeouts, vec![Some(7000)]);
    }
}
