use crate::modules::completion::application::command_line_parser::CommandLineParser;
use crate::modules::completion::application::completion_notice::CompletionNotice;
use crate::modules::completion::application::completion_request::CompletionRequest;
use crate::modules::completion::application::generator_runner::GeneratorRunner;
use crate::modules::completion::application::parse_context::ParseContext;
use crate::modules::completion::application::ports::executable_catalog::ExecutableCatalog;
use crate::modules::completion::domain::completion_error::CompletionError;
use crate::modules::completion::domain::entities::completion_session::CompletionSession;
use crate::modules::completion::domain::services::command_line_tokenizer;
use crate::modules::completion::domain::services::completion_request_policy::{
    env_var, history_prefix, is_mid_word, uses_fuzzy_search,
};
use crate::modules::completion::domain::services::completion_result_builder::build_completion_result;
use crate::modules::completion::domain::services::first_token_suggestions;
use crate::modules::completion::domain::services::generator_trigger_policy::{is_path_template, SYNC_CALL};
use crate::modules::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::modules::completion::domain::value_objects::completion_result::CompletionResult;
use crate::modules::completion::domain::value_objects::generator_key::argument_key;
use crate::modules::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::modules::completion::domain::value_objects::served_request::{ServedRequest, ServedRequestParts};
use crate::modules::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::modules::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::modules::history::domain::repositories::history_repository::HistoryRepository;
use crate::modules::settings::domain::services::settings_catalog::AUTOCOMPLETE_FIRST_TOKEN_COMPLETION;
use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::specs::application::ports::spec_runtime::SpecRuntime;
use crate::modules::specs::domain::spec_error::SpecError;
use crate::modules::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::modules::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::modules::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::modules::specs::domain::value_objects::spec_index::SpecIndex;
use crate::modules::specs::domain::value_objects::text_or_function::TextOrFunction;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Completes the command line under the cursor (SPEC §6): mid-word guard, the command under the cursor, its parse,
/// the query terms and the generators of the current argument, the first-token suggestions, then the ranked reply.
/// The reply carries no ghost text (Q15); a newer request interrupting the work leaves no reply (`None`).
pub(crate) struct CompleteCommandLineUseCase {
    parser: Rc<CommandLineParser>,
    runtime: Rc<dyn SpecRuntime>,
    generators: Rc<GeneratorRunner>,
    executables: Rc<dyn ExecutableCatalog>,
    history: Rc<dyn HistoryRepository>,
    clock: Arc<dyn Clock + Send + Sync>,
    index: SpecIndex,
}

impl CompleteCommandLineUseCase {
    pub(crate) fn new(
        parser: Rc<CommandLineParser>,
        runtime: Rc<dyn SpecRuntime>,
        generators: Rc<GeneratorRunner>,
        executables: Rc<dyn ExecutableCatalog>,
        history: Rc<dyn HistoryRepository>,
        clock: Arc<dyn Clock + Send + Sync>,
        index: SpecIndex,
    ) -> Self {
        CompleteCommandLineUseCase {
            parser,
            runtime,
            generators,
            executables,
            history,
            clock,
            index,
        }
    }

    /// Phase-Parse reply for `req`; `None` when a newer request aborted it (it is kept in the engine inbox).
    pub(crate) fn execute(
        &self,
        session: &mut CompletionSession,
        req: &CompletionRequest,
        report: &mut dyn FnMut(CompletionNotice),
    ) -> Option<CompletionResult> {
        let cursor = req.cursor.min(req.buffer.len());
        // SPEC §6.1.5 mid-word guard
        if is_mid_word(&req.buffer, cursor) {
            return Some(self.error(session, req, cursor));
        }
        let before = &req.buffer[..cursor];
        let aliases = session.alias_map(req.shell);
        let Some(cmd) = command_line_tokenizer::current_command(before, req.shell, &aliases) else {
            return Some(self.error(session, req, cursor));
        };
        let (cache, settings, env) = session.parse_inputs();
        let pctx = ParseContext {
            cwd: &req.cwd,
            settings,
            env,
            shell: req.shell,
            index: &self.index,
        };
        let pr = match self.parser.parse(
            cache,
            &cmd,
            &pctx,
            report,
        ) {
            Ok(pr) => pr,
            Err(CompletionError::Aborted) => return None,
            Err(CompletionError::NoSuggestions(e)) => {
                report(CompletionNotice::NoSuggestions(e));
                return Some(self.error(session, req, cursor));
            }
        };
        let arg = pr.current_arg().clone();
        let arg_key = argument_key(arg.as_ref(), pr.completion());
        session.enter_argument(arg_key);
        let strategy = arg
            .as_ref()
            .map_or(pr.completion().filter_strategy(), |a| a.filter_strategy());
        let fuzzy = uses_fuzzy_search(strategy, session.user_fuzzy());
        let env = session.env();
        let ctx = GeneratorContext::new(
            req.cwd.to_string_lossy().into_owned(),
            env.clone(),
            req.shell.name().into(),
            pr.search_term().to_string(),
            arg.as_ref().is_some_and(|a| a.is_dangerous()),
        );
        let (path, pathext) = (env_var(env, "PATH"), env_var(env, "PATHEXT"));
        let queries = self.queries(&pr, req.shell)?;
        let tokens = pr
            .tokens()
            .get(pr.command_index()..)
            .unwrap_or_default()
            .to_vec();
        let triggered = self.generators.trigger(session, &pr, &ctx, &tokens, report)?;
        let first = if pr.first_token()
            && session.settings().bool(AUTOCOMPLETE_FIRST_TOKEN_COMPLETION)
        {
            self.first_token_items(session, &aliases, path, pathext)
        } else {
            Vec::new()
        };
        let prefix = history_prefix(before, pr.raw_last_token());
        session.serve(ServedRequest::new(ServedRequestParts {
            request_id: req.id,
            buffer: req.buffer.clone(),
            shell: req.shell,
            history_mode: req.history_mode,
            cursor,
            parsed: pr,
            command_text: cmd.original_text().to_string(),
            argument_key: arg_key,
            fuzzy,
            queries,
            triggered_generators: triggered,
            first_token_candidates: first,
            history_prefix: prefix,
        }));
        Some(build_completion_result(session, CompletionPhase::Parse, &self.history.recent()))
    }

    /// Error reply (menu error-reset). Generator state is reset on errors too.
    fn error(&self, session: &mut CompletionSession, req: &CompletionRequest, cursor: usize) -> CompletionResult {
        session.reset_generator_slots();
        session.forget_served_request();
        CompletionResult::error(req.id, req.buffer.clone(), cursor)
    }

    /// Query derivation per generator of the current arg (function `getQueryTerm`s are called once here).
    fn queries(&self, pr: &ParsedCommandLine, shell: Shell) -> Option<Vec<SuggestionQuery>> {
        let Some(arg) = pr.current_arg() else {
            return Some(Vec::new());
        };
        let mut out = Vec::with_capacity(arg.generators().len());
        for g in arg.generators() {
            out.push(if is_path_template(g) {
                SuggestionQuery::Path {
                    backslash: shell == Shell::Pwsh,
                }
            } else {
                match g.get_query_term() {
                    None => SuggestionQuery::Term,
                    Some(TextOrFunction::Str(s)) => SuggestionQuery::Str(s.clone()),
                    Some(TextOrFunction::Fn(f)) => match self.runtime.call(
                        SpecFunctionCall::QueryTerm {
                            function: *f,
                            search_term: pr.search_term().to_string(),
                        },
                        None,
                        SYNC_CALL,
                    ) {
                        Ok(SpecFunctionResult::QueryTerm(Some(s))) => SuggestionQuery::Value(s),
                        Err(SpecError::Aborted) => return None,
                        Ok(
                            SpecFunctionResult::QueryTerm(None)
                            | SpecFunctionResult::Trigger(_)
                            | SpecFunctionResult::Alias(_)
                            | SpecFunctionResult::LoadSpec(_)
                            | SpecFunctionResult::GenerateSpecCacheKey(_)
                            | SpecFunctionResult::GenerateSpec(_)
                            | SpecFunctionResult::Script(_)
                            | SpecFunctionResult::Custom(_)
                            | SpecFunctionResult::PostProcess(_)
                            | SpecFunctionResult::FilterTemplateSuggestions(_),
                        )
                        | Err(
                            SpecError::Timeout
                            | SpecError::Invalid(_)
                            | SpecError::BundleUnavailable(_),
                        ) => SuggestionQuery::Value(pr.search_term().to_string()), // error → search term
                    },
                }
            });
        }
        Some(out)
    }

    fn first_token_items(
        &self,
        session: &mut CompletionSession,
        aliases: &HashMap<String, String>,
        path: Option<String>,
        pathext: Option<String>,
    ) -> Vec<SuggestionCandidate> {
        session.first_token_suggestions(&|| self.clock.now_instant(), || {
            let executables = self.executables.executables(path.as_deref(), pathext.as_deref());
            first_token_suggestions::first_token_items(
                self.index.completions(),
                aliases,
                &executables,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::application::ports::executable_catalog::FixedExecutableCatalog;
    use crate::modules::completion::application::test_fixtures::{request, CompletionDoubles};
    use crate::modules::completion::domain::value_objects::generator_key::GeneratorKey;
    use crate::modules::settings::domain::value_objects::setting_value::SettingValue;
    use crate::modules::settings::domain::value_objects::settings::Settings;
    use crate::modules::specs::application::ports::spec_runtime::InMemorySpecRuntime;
    use crate::modules::specs::domain::entities::subcommand::Subcommand;
    use crate::modules::specs::domain::repositories::spec_repository::InMemorySpecRepository;
    use crate::modules::specs::domain::value_objects::arg::Arg;
    use crate::modules::specs::domain::value_objects::generator::Generator;
    use std::collections::HashSet;
    use std::time::Duration;

    fn command(names: &[&str]) -> Subcommand {
        Subcommand::default().with_names(names.iter().map(|name| name.to_string()).collect())
    }

    fn git() -> Rc<Subcommand> {
        Rc::new(command(&["git"]).with_subcommands(vec![Rc::new(command(&["commit"])), Rc::new(command(&["status"]))]))
    }

    fn rows(result: &CompletionResult) -> Vec<String> {
        result.suggestions().iter().map(|row| row.first_name().to_string()).collect()
    }

    fn owned(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    fn first_token_completion() -> Settings {
        Settings::default().with(AUTOCOMPLETE_FIRST_TOKEN_COMPLETION, SettingValue::Bool(true))
    }

    fn served_fuzzy(session: &CompletionSession) -> bool {
        session.served().is_some_and(|served| served.fuzzy())
    }

    #[test]
    fn should_reply_with_the_subcommands_of_the_spec_for_the_typed_word() {
        let doubles = CompletionDoubles::new(InMemorySpecRepository::with(&[("git", &git())]), InMemorySpecRuntime::default());
        let mut session = doubles.session(Settings::default());

        let result = doubles
            .complete_command_line(SpecIndex::default())
            .execute(&mut session, &request(1, "git co"), &mut |_| {})
            .unwrap();

        assert_eq!((result.phase(), result.is_error(), rows(&result)), (CompletionPhase::Parse, false, owned(&["commit"])));
    }

    #[test]
    fn should_reply_an_error_for_a_cursor_inside_a_word() {
        let doubles = CompletionDoubles::new(InMemorySpecRepository::with(&[("git", &git())]), InMemorySpecRuntime::default());
        let use_case = doubles.complete_command_line(SpecIndex::default());
        let mut session = doubles.session(Settings::default());
        use_case.execute(&mut session, &request(1, "git co"), &mut |_| {});
        let inside_a_word = CompletionRequest {
            cursor: 5,
            ..request(2, "git commit")
        };

        let result = use_case.execute(&mut session, &inside_a_word, &mut |_| {}).unwrap();

        assert_eq!(
            (result.is_error(), result.phase(), result.cursor(), session.served().is_none()),
            (true, CompletionPhase::Parse, 5, true)
        );
    }

    #[test]
    fn should_reply_an_error_and_report_no_suggestions_for_a_command_without_a_spec() {
        let doubles = CompletionDoubles::new(InMemorySpecRepository::default(), InMemorySpecRuntime::default());
        let mut session = doubles.session(Settings::default());
        let mut notices = Vec::new();

        let result = doubles
            .complete_command_line(SpecIndex::default())
            .execute(&mut session, &request(1, "missing --flag"), &mut |notice| notices.push(notice))
            .unwrap();

        assert_eq!(
            (result.is_error(), notices),
            (true, vec![CompletionNotice::NoSuggestions("failed loading spec for missing".into())])
        );
    }

    #[test]
    fn should_reply_nothing_when_a_newer_request_interrupts_the_parse() {
        let generated = Rc::new(command(&["g"]).with_generate_spec(Some(1)));
        let doubles = CompletionDoubles::new(
            InMemorySpecRepository::with(&[("git", &git()), ("g", &generated)]),
            InMemorySpecRuntime::default().answering(1, |_| Err(SpecError::Aborted)),
        );
        let use_case = doubles.complete_command_line(SpecIndex::default());
        let mut session = doubles.session(Settings::default());
        use_case.execute(&mut session, &request(1, "git co"), &mut |_| {});
        let mut notices = Vec::new();

        let result = use_case.execute(&mut session, &request(2, "g x"), &mut |notice| notices.push(notice));

        assert_eq!(
            (result.is_none(), notices, session.served().map(|served| served.request_id())),
            (true, vec![], Some(1))
        );
    }

    #[test]
    fn should_start_the_generators_of_a_new_argument_and_report_them_loading() {
        let argument = Rc::new(Arg::default().with_generators(vec![Generator::default().with_custom(Some(1))]));
        let tool = Rc::new(command(&["tool"]).with_args(vec![argument.clone()]));
        let doubles = CompletionDoubles::new(InMemorySpecRepository::with(&[("tool", &tool)]), InMemorySpecRuntime::default());
        let mut session = doubles.session(Settings::default());

        let result = doubles
            .complete_command_line(SpecIndex::default())
            .execute(&mut session, &request(1, "tool "), &mut |_| {})
            .unwrap();

        assert_eq!(
            (result.is_loading(), result.triggered_generators(), session.has_running_generators()),
            (true, &[GeneratorKey::for_arg(&argument, 0)][..], true)
        );
    }

    #[test]
    fn should_keep_the_toggled_fuzzy_search_until_the_argument_changes() {
        let tool = Rc::new(command(&["tool"]).with_args(vec![Rc::new(Arg::default()), Rc::new(Arg::default())]));
        let doubles = CompletionDoubles::new(InMemorySpecRepository::with(&[("tool", &tool)]), InMemorySpecRuntime::default());
        let use_case = doubles.complete_command_line(SpecIndex::default());
        let mut session = doubles.session(Settings::default());
        use_case.execute(&mut session, &request(1, "tool "), &mut |_| {});
        session.toggle_fuzzy_search();
        use_case.execute(&mut session, &request(2, "tool x"), &mut |_| {});
        let typing = served_fuzzy(&session);
        use_case.execute(&mut session, &request(3, "tool xy"), &mut |_| {});
        let still_typing = served_fuzzy(&session);

        use_case.execute(&mut session, &request(4, "tool xy "), &mut |_| {});

        assert_eq!((typing, still_typing, served_fuzzy(&session)), (true, true, false));
    }

    #[test]
    fn should_reply_first_token_suggestions_for_a_single_word() {
        let doubles = CompletionDoubles {
            executables: Rc::new(FixedExecutableCatalog::new(vec!["ma".into(), "mk".into()])),
            ..CompletionDoubles::new(InMemorySpecRepository::default(), InMemorySpecRuntime::default())
        };
        let index = SpecIndex::new(vec!["mk".into(), "mz/sub".into()], HashSet::new());
        let mut session = doubles.session(first_token_completion());
        session.apply_session_context(HashMap::new(), Some("alias mka='make all'".into()));

        let result = doubles
            .complete_command_line(index)
            .execute(&mut session, &request(1, "m"), &mut |_| {})
            .unwrap();

        // the alias ranks first by its priority; the spec name comes before the executables, each name once
        assert_eq!(rows(&result), ["mka", "mk", "ma"]);
    }

    #[test]
    fn should_scan_the_path_only_on_a_first_token_cache_miss() {
        let doubles = CompletionDoubles::new(InMemorySpecRepository::default(), InMemorySpecRuntime::default());
        let use_case = doubles.complete_command_line(SpecIndex::default());
        let mut session = doubles.session(first_token_completion());
        use_case.execute(&mut session, &request(1, "m"), &mut |_| {});
        use_case.execute(&mut session, &request(2, "mk"), &mut |_| {});
        let scans_within_ten_seconds = doubles.executables.scans();
        doubles.clock.advance(Duration::from_secs(10));

        use_case.execute(&mut session, &request(3, "mk"), &mut |_| {});

        assert_eq!((scans_within_ten_seconds, doubles.executables.scans()), (1, 2));
    }

    #[test]
    fn should_rank_recently_inserted_suggestions_first() {
        let git = Rc::new(command(&["git"]).with_subcommands(vec![Rc::new(command(&["commit"])), Rc::new(command(&["checkout"]))]));
        let doubles = CompletionDoubles::new(InMemorySpecRepository::with(&[("git", &git)]), InMemorySpecRuntime::default());
        let use_case = doubles.complete_command_line(SpecIndex::default());
        let mut session = doubles.session(Settings::default());
        let before = rows(&use_case.execute(&mut session, &request(1, "git "), &mut |_| {}).unwrap());
        session.record_insertion("git".into(), "commit".into(), doubles.clock.now_ms());

        let result = use_case.execute(&mut session, &request(2, "git "), &mut |_| {}).unwrap();

        assert_eq!((before, rows(&result)), (owned(&["checkout", "commit"]), owned(&["commit", "checkout"])));
    }
}
