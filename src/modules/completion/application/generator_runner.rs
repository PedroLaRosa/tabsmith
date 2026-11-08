use crate::modules::completion::application::completion_notice::CompletionNotice;
use crate::modules::completion::application::ports::directory_lister::DirectoryLister;
use crate::modules::completion::domain::entities::completion_session::CompletionSession;
use crate::modules::completion::domain::entities::generator_run::GeneratorRun;
use crate::modules::completion::domain::services::generator_trigger_policy::{
    cache_key, dir_changed, effective_debounce, is_path_template, run_budget, script_timeout, triggered,
    SYNC_CALL,
};
use crate::modules::completion::domain::services::path_template;
use crate::modules::completion::domain::value_objects::cache_lookup::CacheLookup;
use crate::modules::completion::domain::value_objects::generator_command::GeneratorCommand;
use crate::modules::completion::domain::value_objects::generator_key::GeneratorKey;
use crate::modules::completion::domain::value_objects::generator_outcome::GeneratorOutcome;
use crate::modules::completion::domain::value_objects::generator_step::GeneratorStep;
use crate::modules::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::modules::shared::application::ports::clock::Clock;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::specs::application::ports::interrupted::Interrupted;
use crate::modules::specs::application::ports::spec_runtime::SpecRuntime;
use crate::modules::specs::domain::spec_error::SpecError;
use crate::modules::specs::domain::value_objects::custom_generator_result::CustomGeneratorResult;
use crate::modules::specs::domain::value_objects::generator::Generator;
use crate::modules::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::modules::specs::domain::value_objects::generator_script::GeneratorScript;
use crate::modules::specs::domain::value_objects::generator_trigger::GeneratorTrigger;
use crate::modules::specs::domain::value_objects::script_resolution::ScriptResolution;
use crate::modules::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::modules::specs::domain::value_objects::spec_function_kind::SpecFunctionKind;
use crate::modules::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::modules::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
use crate::modules::specs::domain::value_objects::spec_value::SpecValue;
use crate::modules::specs::domain::value_objects::task_id::TaskId;
use crate::modules::specs::domain::value_objects::task_outcome::TaskOutcome;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// Pump slice while generators run: deferred messages and finished runs are handled at least this often.
const SLICE: Duration = Duration::from_millis(50);

/// Starts, feeds and settles generator runs through the spec runtime, the directory lister and the clock, for both
/// generator use cases (they hold it by `Rc`).
pub(crate) struct GeneratorRunner {
    runtime: Rc<dyn SpecRuntime>,
    directories: Rc<dyn DirectoryLister>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl GeneratorRunner {
    pub(crate) fn new(
        runtime: Rc<dyn SpecRuntime>,
        directories: Rc<dyn DirectoryLister>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        GeneratorRunner {
            runtime,
            directories,
            clock,
        }
    }

    /// SPEC §6.6.1: decide triggers (no mutation until every JS trigger answered), then start runs.
    pub(crate) fn trigger(
        &self,
        session: &mut CompletionSession,
        pr: &ParsedCommandLine,
        ctx: &GeneratorContext,
        tokens: &[String],
        report: &mut dyn FnMut(CompletionNotice),
    ) -> Option<Vec<GeneratorKey>> {
        let arg = pr.current_arg().clone();
        let same = match (&arg, session.slot_argument()) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            _ => false,
        };
        let (new, old) = (pr.search_term(), session.previous_search_term());
        let gens: &[Generator] = arg.as_deref().map_or(&[], |a| a.generators());
        let mut fire = Vec::with_capacity(gens.len());
        for (i, g) in gens.iter().enumerate() {
            fire.push(if !same || i >= session.slot_count() {
                true
            } else if is_path_template(g) {
                dir_changed(new, old, ctx.shell() == Shell::Pwsh.name())
            } else {
                match g.trigger() {
                    None => arg.as_ref().is_some_and(|a| a.debounce().is_some()),
                    Some(GeneratorTrigger::Fn(f)) => match self.runtime.call(
                        SpecFunctionCall::Trigger {
                            function: *f,
                            new_token: new.to_string(),
                            old_token: old.to_string(),
                        },
                        None,
                        SYNC_CALL,
                    ) {
                        Ok(SpecFunctionResult::Trigger(fired)) => fired,
                        Err(SpecError::Aborted) => return None,
                        Ok(
                            SpecFunctionResult::QueryTerm(_)
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
                        ) => true, // a throwing trigger counts as a trigger
                    },
                    Some(t) => triggered(t, new, old).unwrap_or(true),
                }
            });
        }
        session.remember_search_term(pr.search_term().to_string());
        let Some(arg) = arg.as_ref() else {
            session.clear_generator_slots();
            return Some(Vec::new());
        };
        if !same {
            // ponytail: results are not carried over to a different arg (they could be kept while loading)
            let keys = (0..gens.len())
                .map(|i| GeneratorKey::for_arg(arg, i))
                .collect();
            session.assign_generator_slots(arg.clone(), keys);
        }
        let help = if gens.iter().any(|g| g.template().iter().any(|t| t == "help")) {
            path_template::help_items(pr)
        } else {
            Vec::new()
        };
        let floor = script_timeout(session.settings());
        let mut triggered_keys = Vec::new();
        for (i, fire) in fire.into_iter().enumerate() {
            if !fire {
                continue;
            }
            let (generation, key) = session.fire_slot(i);
            triggered_keys.push(key);
            let debounce = effective_debounce(arg.debounce());
            // ponytail: one deadline for the whole run (custom generators may exec several times)
            let budget = run_budget(debounce, floor, gens[i].script_timeout());
            let mut run = GeneratorRun::new(
                i,
                generation,
                arg.clone(),
                ctx.clone(),
                tokens.to_vec(),
                help.clone(),
                self.clock.now_instant() + Duration::from_millis(budget),
            );
            let outcome = match debounce {
                Some(ms) => self.start(&mut run, GeneratorCommand::StartSleep(ms), GeneratorStep::Debounce),
                None => self.begin(session, &mut run),
            };
            self.settle(session, run, outcome, report);
        }
        Some(triggered_keys)
    }

    /// Wait (one slice) on every running generator task: `Some(changed)`, whether a current slot changed, or `None`
    /// when a newer request interrupted the wait (runs, cache and tasks untouched).
    pub(crate) fn progress(
        &self,
        session: &mut CompletionSession,
        report: &mut dyn FnMut(CompletionNotice),
    ) -> Option<bool> {
        let mut tasks: Vec<TaskId> = session.runs().iter().map(|r| r.task()).collect();
        tasks.sort_unstable();
        tasks.dedup();
        let deadline = session
            .runs()
            .iter()
            .map(|r| r.deadline())
            .min()
            .map_or(self.clock.now_instant() + SLICE, |d| d.min(self.clock.now_instant() + SLICE));
        let results = match self.runtime.wait(&tasks, deadline) {
            Ok(r) => r,
            Err(Interrupted) => return None, // a newer request is queued
        };
        let mut changed = false;
        let mut runs = session.take_runs();
        for (task, res) in tasks.iter().zip(results) {
            let Some(res) = res else { continue };
            let (mine, rest): (Vec<GeneratorRun>, Vec<GeneratorRun>) =
                runs.into_iter().partition(|r| r.task() == *task);
            runs = rest;
            // the owner (stores into the cache) goes first so sharers see a consistent cache
            let (owners, sharers): (Vec<GeneratorRun>, Vec<GeneratorRun>) =
                mine.into_iter().partition(|r| r.store_key().is_some());
            for mut run in owners.into_iter().chain(sharers) {
                let o = self.advance(session, &mut run, res.clone());
                changed |= self.settle(session, run, o, report);
            }
            self.runtime.forget(&[*task]);
        }
        let now = self.clock.now_instant();
        let (expired, alive): (Vec<GeneratorRun>, Vec<GeneratorRun>) =
            runs.into_iter().partition(|r| now >= r.deadline());
        session.keep_runs(alive);
        for mut run in expired {
            if let Some(k) = run.take_store() {
                session.generator_cache_mut().fail(&k);
            }
            if !session.has_run_on(run.task()) {
                self.runtime.forget(&[run.task()]);
            }
            changed |= self.settle(session, run, GeneratorOutcome::Fail("timed out".into()), report);
        }
        Some(changed)
    }

    /// Record an outcome; returns whether a current slot changed. Every failure is reported first.
    fn settle(
        &self,
        session: &mut CompletionSession,
        run: GeneratorRun,
        outcome: GeneratorOutcome,
        report: &mut dyn FnMut(CompletionNotice),
    ) -> bool {
        if let GeneratorOutcome::Fail(e) = &outcome {
            report(CompletionNotice::GeneratorFailed(e.clone()));
        }
        session.settle(run, outcome)
    }

    /// Run a transition's command: settle with its outcome, or start its task and wait on it in `step`.
    fn start(&self, run: &mut GeneratorRun, command: GeneratorCommand, step: GeneratorStep) -> GeneratorOutcome {
        let task = match command {
            GeneratorCommand::Settle(outcome) => return outcome,
            GeneratorCommand::StartCall(call, context) => self.runtime.start(call, Some(&context)),
            GeneratorCommand::StartExec(request, context) => self.runtime.start_exec(request, &context),
            GeneratorCommand::StartSleep(ms) => self.runtime.start_sleep(ms),
        };
        run.wait_on(task, step);
        GeneratorOutcome::Pending
    }

    /// First step after the (optional) debounce: template, script or custom.
    fn begin(&self, session: &mut CompletionSession, run: &mut GeneratorRun) -> GeneratorOutcome {
        let arg = run.arg().clone();
        let g = &arg.generators()[run.index()];
        if !g.template().is_empty() {
            let mut items = Vec::new();
            let files = g.template().iter().any(|t| t == "filepaths");
            if is_path_template(g) && !run.context().cwd().is_empty() {
                let dir = path_template::inserted_directory(
                    run.context().cwd(),
                    run.context().search_term(),
                    run.context().env(),
                    run.context().shell() == Shell::Pwsh.name(),
                );
                items = self.directories.list(&dir, files, run.context().is_dangerous());
            }
            let command = run.template_command(g, items);
            return self.start(run, command, GeneratorStep::Filter);
        }
        if run.context().cwd().is_empty() {
            return GeneratorOutcome::Done(Vec::new()); // no context for script/custom generators
        }
        if let Some(script) = g.script() {
            return match script {
                GeneratorScript::Fn(f) => {
                    let task = self.runtime.start(
                        SpecFunctionCall::Script {
                            function: *f,
                            tokens: run.tokens().to_vec(),
                        },
                        Some(run.context()),
                    );
                    run.wait_on(task, GeneratorStep::Script);
                    GeneratorOutcome::Pending
                }
                GeneratorScript::Argv(a) => {
                    let argv = SpecValue::List(a.iter().cloned().map(SpecValue::Text).collect());
                    self.script_input(session, run, g, argv)
                }
                GeneratorScript::Shell(s) => self.script_input(session, run, g, SpecValue::Text(s.clone())),
                GeneratorScript::Object(o) => self.script_input(session, run, g, o.clone()),
            };
        }
        let Some(custom) = g.custom() else {
            return GeneratorOutcome::Done(Vec::new());
        };
        let key = g.cache().as_ref().map(|c| {
            cache_key(
                c,
                run.context().cwd(),
                c.cache_key().clone().unwrap_or_else(|| run.tokens().join(" ")),
            )
        });
        let call = SpecFunctionCall::Custom {
            function: custom,
            tokens: run.tokens().to_vec(),
        };
        let ctx = run.context().clone();
        match self.fetch(session, run, g, key, GeneratorStep::Custom, move |js| {
            js.start(call, Some(&ctx))
        }) {
            Ok(v) => self.custom_result(run, g, v),
            Err(o) => o,
        }
    }

    /// Cached fetch (D10): a value now (`Ok`), or a started/shared task (`Err(Pending)`).
    fn fetch(
        &self,
        session: &mut CompletionSession,
        run: &mut GeneratorRun,
        g: &Generator,
        key: Option<String>,
        step: GeneratorStep,
        start: impl FnOnce(&dyn SpecRuntime) -> TaskId,
    ) -> Result<SpecValue, GeneratorOutcome> {
        let (Some(cfg), Some(key)) = (g.cache(), key) else {
            run.wait_on(start(self.runtime.as_ref()), step);
            return Err(GeneratorOutcome::Pending);
        };
        match session.generator_cache_mut().lookup(&key, cfg, self.clock.now_ms()) {
            CacheLookup::Fresh(v) => Ok(v),
            CacheLookup::Stale { value, refresh } => {
                if refresh {
                    let mut bg = run.background_copy();
                    bg.wait_on(start(self.runtime.as_ref()), step);
                    session.generator_cache_mut().start(&key, bg.task());
                    bg.store_result_as(key);
                    session.push_run(bg);
                }
                Ok(value)
            }
            CacheLookup::Shared(t) => {
                run.wait_on(t, step);
                Err(GeneratorOutcome::Pending)
            }
            CacheLookup::Miss => {
                run.wait_on(start(self.runtime.as_ref()), step);
                session.generator_cache_mut().start(&key, run.task());
                run.store_result_as(key);
                Err(GeneratorOutcome::Pending)
            }
        }
    }

    /// SPEC §6.6.3: script value → exec input → (cached) exec.
    fn script_input(&self, session: &mut CompletionSession, run: &mut GeneratorRun, g: &Generator, v: SpecValue) -> GeneratorOutcome {
        let resolution = match self.runtime.read(&v, SpecFunctionKind::Script, run.context()) {
            SpecFunctionResult::Script(resolution) => resolution,
            SpecFunctionResult::QueryTerm(_)
            | SpecFunctionResult::Trigger(_)
            | SpecFunctionResult::Alias(_)
            | SpecFunctionResult::LoadSpec(_)
            | SpecFunctionResult::GenerateSpecCacheKey(_)
            | SpecFunctionResult::GenerateSpec(_)
            | SpecFunctionResult::Custom(_)
            | SpecFunctionResult::PostProcess(_)
            | SpecFunctionResult::FilterTemplateSuggestions(_) => return GeneratorOutcome::Done(Vec::new()), // falsy → []
        };
        let key = match &resolution {
            ScriptResolution::Ready { cache_key_source, .. } => {
                let cache_key_source = cache_key_source.clone();
                g.cache()
                    .as_ref()
                    .map(|c| cache_key(c, run.context().cwd(), c.cache_key().clone().unwrap_or(cache_key_source)))
            }
            ScriptResolution::Skip | ScriptResolution::Invalid(_) => None,
        };
        match run.script_command(g, resolution, script_timeout(session.settings())) {
            GeneratorCommand::StartExec(request, ctx) => {
                match self.fetch(session, run, g, key, GeneratorStep::Exec, move |js| {
                    js.start_exec(request, &ctx)
                }) {
                    Ok(v) => self.exec_output(run, g, &v),
                    Err(o) => o,
                }
            }
            command @ (GeneratorCommand::Settle(_) | GeneratorCommand::StartCall(..) | GeneratorCommand::StartSleep(_)) => {
                self.start(run, command, GeneratorStep::Exec)
            }
        }
    }

    /// `splitOn` takes precedence over `postProcess`; neither → [].
    fn exec_output(&self, run: &mut GeneratorRun, g: &Generator, v: &SpecValue) -> GeneratorOutcome {
        let stdout = self.runtime.read_exec_stdout(v);
        let command = run.exec_command(g, &stdout);
        self.start(run, command, GeneratorStep::PostProcess)
    }

    /// Custom result; `filterTemplateSuggestions` applies when the first item has a `context`.
    fn custom_result(&self, run: &mut GeneratorRun, g: &Generator, v: SpecValue) -> GeneratorOutcome {
        let custom = match self.runtime.read(&v, SpecFunctionKind::Custom, run.context()) {
            SpecFunctionResult::Custom(custom) => custom,
            SpecFunctionResult::QueryTerm(_)
            | SpecFunctionResult::Trigger(_)
            | SpecFunctionResult::Alias(_)
            | SpecFunctionResult::LoadSpec(_)
            | SpecFunctionResult::GenerateSpecCacheKey(_)
            | SpecFunctionResult::GenerateSpec(_)
            | SpecFunctionResult::Script(_)
            | SpecFunctionResult::PostProcess(_)
            | SpecFunctionResult::FilterTemplateSuggestions(_) => CustomGeneratorResult::new(false, Vec::new()),
        };
        let command = run.custom_command(g, v, custom);
        self.start(run, command, GeneratorStep::Filter)
    }

    /// A task of `run` finished.
    fn advance(&self, session: &mut CompletionSession, run: &mut GeneratorRun, res: TaskOutcome) -> GeneratorOutcome {
        let v = match res {
            Ok(v) => v,
            Err(e) => {
                if let Some(k) = run.take_store() {
                    session.generator_cache_mut().fail(&k);
                }
                return GeneratorOutcome::Fail(e);
            }
        };
        if let Some(k) = run.take_store() {
            session.generator_cache_mut().store(&k, v.clone(), self.clock.now_ms());
        }
        if !session.is_current(run) {
            return GeneratorOutcome::Done(Vec::new()); // superseded (e.g. after a debounce): abandon
        }
        let arg = run.arg().clone();
        let g = &arg.generators()[run.index()];
        match run.step() {
            GeneratorStep::Debounce => self.begin(session, run),
            GeneratorStep::Script => self.script_input(session, run, g, v),
            GeneratorStep::Exec => self.exec_output(run, g, &v),
            GeneratorStep::Custom => self.custom_result(run, g, v),
            GeneratorStep::PostProcess => GeneratorOutcome::Done(suggestion_rows(self.runtime.read(
                &v,
                SpecFunctionKind::PostProcess,
                run.context(),
            ))),
            GeneratorStep::Filter => GeneratorOutcome::Done(suggestion_rows(self.runtime.read(
                &v,
                SpecFunctionKind::FilterTemplateSuggestions,
                run.context(),
            ))),
        }
    }
}

/// The rows of a read post-process or filter result; the kinds a generator never reads this way give none.
fn suggestion_rows(result: SpecFunctionResult) -> Vec<SpecSuggestion> {
    match result {
        SpecFunctionResult::PostProcess(rows) | SpecFunctionResult::FilterTemplateSuggestions(rows) => rows,
        SpecFunctionResult::QueryTerm(_)
        | SpecFunctionResult::Trigger(_)
        | SpecFunctionResult::Alias(_)
        | SpecFunctionResult::LoadSpec(_)
        | SpecFunctionResult::GenerateSpecCacheKey(_)
        | SpecFunctionResult::GenerateSpec(_)
        | SpecFunctionResult::Script(_)
        | SpecFunctionResult::Custom(_) => Vec::new(),
    }
}
