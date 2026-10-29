use crate::completion::application::command_line_parser::CommandLineParser;
use crate::completion::application::completion_notice::CompletionNotice;
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::application::generator_runner::GeneratorRunner;
use crate::completion::application::parse_context::ParseContext;
use crate::completion::application::ports::executable_catalog::ExecutableCatalog;
use crate::completion::domain::completion_error::CompletionError;
use crate::completion::domain::entities::completion_session::CompletionSession;
use crate::completion::domain::services::command_line_tokenizer;
use crate::completion::domain::services::completion_request_policy::{
    env_var, history_prefix, is_mid_word, uses_fuzzy_search,
};
use crate::completion::domain::services::completion_result_builder::build_completion_result;
use crate::completion::domain::services::first_token_suggestions;
use crate::completion::domain::services::generator_trigger_policy::{is_path_template, SYNC_CALL};
use crate::completion::domain::value_objects::completion_phase::CompletionPhase;
use crate::completion::domain::value_objects::completion_result::CompletionResult;
use crate::completion::domain::value_objects::generator_key::argument_key;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::completion::domain::value_objects::served_request::{ServedRequest, ServedRequestParts};
use crate::completion::domain::value_objects::suggestion_candidate::SuggestionCandidate;
use crate::completion::domain::value_objects::suggestion_query::SuggestionQuery;
use crate::history::domain::repositories::history_repository::HistoryRepository;
use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_FIRST_TOKEN_COMPLETION;
use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::value_objects::shell::Shell;
use crate::specs::application::ports::spec_runtime::SpecRuntime;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::specs::domain::value_objects::spec_index::SpecIndex;
use crate::specs::domain::value_objects::text_or_function::TextOrFunction;
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
    use crate::completion::application::test_fixtures::{request, CompletionDoubles};
    use crate::specs::application::ports::spec_runtime::InMemorySpecRuntime;
    use crate::specs::domain::entities::subcommand::Subcommand;
    use crate::specs::domain::repositories::spec_repository::InMemorySpecRepository;
    use crate::settings::domain::value_objects::settings::Settings;

    fn command(names: &[&str]) -> Subcommand {
        Subcommand::default().with_names(names.iter().map(|name| name.to_string()).collect())
    }

    fn git() -> Rc<Subcommand> {
        Rc::new(command(&["git"]).with_subcommands(vec![Rc::new(command(&["commit"])), Rc::new(command(&["status"]))]))
    }

    fn rows(result: &CompletionResult) -> Vec<&str> {
        result.suggestions().iter().map(|row| row.first_name()).collect()
    }

    #[test]
    fn should_reply_with_the_subcommands_of_the_spec_for_the_typed_word() {
        let doubles = CompletionDoubles::new(InMemorySpecRepository::with(&[("git", &git())]), InMemorySpecRuntime::default());
        let mut session = doubles.session(Settings::default());

        let result = doubles
            .complete_command_line(SpecIndex::default())
            .execute(&mut session, &request(1, "git co"), &mut |_| {})
            .unwrap();

        assert_eq!((result.phase(), result.is_error(), rows(&result)), (CompletionPhase::Parse, false, vec!["commit"]));
    }
}
