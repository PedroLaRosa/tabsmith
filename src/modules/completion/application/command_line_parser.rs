use crate::modules::settings::domain::services::settings_catalog::AUTOCOMPLETE_DEVELOPER_MODE;
use crate::modules::settings::domain::value_objects::settings::Settings;
use crate::modules::completion::application::completion_notice::CompletionNotice;
use crate::modules::completion::application::parse_context::ParseContext;
use crate::modules::completion::domain::completion_error::CompletionError;
use crate::modules::completion::domain::entities::parse_prefix_cache::ParsePrefixCache;
use crate::modules::completion::domain::services::argv_parser::{result, update, update_final, ArgvState};
use crate::modules::completion::domain::services::command_line_tokenizer::substitute_token;
use crate::modules::completion::domain::value_objects::annotation_kind::AnnotationKind;
use crate::modules::completion::domain::value_objects::command_line::CommandLine;
use crate::modules::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::modules::specs::application::ports::spec_runtime::SpecRuntime;
use crate::modules::specs::domain::entities::subcommand::Subcommand;
use crate::modules::specs::domain::repositories::spec_repository::SpecRepository;
use crate::modules::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::modules::specs::domain::value_objects::spec_function_call::SpecFunctionCall;
use crate::modules::specs::domain::value_objects::spec_function_result::SpecFunctionResult;
use crate::modules::specs::domain::value_objects::spec_load_directive::SpecLoadDirective;
use crate::modules::specs::domain::services::spec_locator::spec_location;
use crate::modules::specs::domain::value_objects::spec_location::SpecLocation;
use crate::modules::specs::domain::value_objects::spec_lookup::SpecLookup;
use crate::modules::specs::domain::value_objects::text_or_function::TextOrFunction;
use crate::modules::shared::domain::value_objects::shell::Shell;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

#[cfg(test)]
mod tests;

/// Deadline for one spec callback (SPEC §6.3).
const TIMEOUT: Duration = Duration::from_secs(5);

fn no(msg: impl Into<String>) -> CompletionError {
    CompletionError::NoSuggestions(msg.into())
}

/// Parses the command under the cursor over the spec repository and the spec runtime (SPEC §6.3–6.4).
pub(crate) struct CommandLineParser {
    specs: Rc<dyn SpecRepository>,
    runtime: Rc<dyn SpecRuntime>,
}

impl CommandLineParser {
    pub(crate) fn new(specs: Rc<dyn SpecRepository>, runtime: Rc<dyn SpecRuntime>) -> Self {
        CommandLineParser { specs, runtime }
    }

    /// Drop caches (dev mode toggles, settings reload). The CLI-version memo survives, and so
    /// do spec identities (`by_json`, the first-token spec): unchanged specs are not a "new argument".
    pub(crate) fn clear_caches(&self, cache: &mut ParsePrefixCache) {
        self.specs.forget_loaded_specs();
        cache.clear_keeping_first();
    }

    /// Parse the command under the cursor against its spec (SPEC §6.4). A failed generateSpec is reported at once.
    pub(crate) fn parse(
        &self,
        cache: &mut ParsePrefixCache,
        cmd: &CommandLine,
        ctx: &ParseContext,
        report: &mut dyn FnMut(CompletionNotice),
    ) -> Result<ParsedCommandLine, CompletionError> {
        let dev = dev_mode(ctx.settings);
        let generator = generator_context(ctx);
        let lookup = SpecLookup::new(ctx.settings, ctx.index, &generator);
        let mut pass = ParsePass {
            cache,
            specs: &*self.specs,
            runtime: &*self.runtime,
            lookup,
            generator: &generator,
            shell: ctx.shell,
            report,
        };
        pass.parse(cmd, ctx.cwd, dev)
    }
}

fn dev_mode(settings: &Settings) -> bool {
    settings.bool(AUTOCOMPLETE_DEVELOPER_MODE)
}

fn generator_context(ctx: &ParseContext) -> GeneratorContext {
    GeneratorContext::new(
        ctx.cwd.to_string_lossy().into_owned(),
        ctx.env.clone(),
        ctx.shell.name().into(),
        String::new(),
        false,
    )
}

/// One parse: the caller's cache, the spec ports, the parse's spec lookup and generator context, the request's shell
/// (alias re-tokenizing) and where notices go.
struct ParsePass<'a> {
    cache: &'a mut ParsePrefixCache,
    specs: &'a dyn SpecRepository,
    runtime: &'a dyn SpecRuntime,
    lookup: SpecLookup<'a>,
    generator: &'a GeneratorContext,
    shell: Shell,
    report: &'a mut dyn FnMut(CompletionNotice),
}

/// Guards loadSpec cycles in user specs (they would recurse forever).
const MAX_DEPTH: usize = 64;

impl ParsePass<'_> {
    fn load(&self, location: &SpecLocation) -> Result<Option<Rc<Subcommand>>, CompletionError> {
        self.specs.find(location, &self.lookup).map_err(CompletionError::from)
    }

    /// Call a spec function; its `executeShellCommand` is bound to the session's exec.
    fn call(&self, call: SpecFunctionCall) -> Result<SpecFunctionResult, CompletionError> {
        Ok(SpecRuntime::call(self.runtime, call, Some(self.generator), TIMEOUT)?)
    }

    fn substitute(&self, cmd: &CommandLine, index: usize, value: &str) -> Option<CommandLine> {
        substitute_token(cmd, index, value, self.shell)
    }

    fn parse(
        &mut self,
        cmd: &CommandLine,
        cwd: &Path,
        dev: bool,
    ) -> Result<ParsedCommandLine, CompletionError> {
        let last = cmd.tokens().last().ok_or_else(|| no("no tokens"))?;
        let texts: Vec<String> = cmd.tokens().iter().map(|t| t.text().to_string()).collect();
        if cmd.tokens().len() == 1 {
            if !last.text().contains('/') {
                let st = ArgvState::initial(self.cache.first().clone(), last.text(), "", texts);
                return Ok(result(st, last.text(), last.raw(), true));
            }
            let name = if last.text() == "bin/console" {
                "php/bin-console"
            } else {
                "dotslash"
            };
            let spec = self
                .load(&SpecLocation::Global { name: name.into() })?
                .ok_or_else(|| no(format!("no {name} spec")))?;
            return Ok(result(
                ArgvState::initial(spec, last.text(), name, texts),
                last.text(),
                last.raw(),
                false,
            ));
        }
        let loc = spec_location(&texts[0], cwd, None);
        let st = self.parse_prefix(cmd, vec![loc], 0, cwd, dev, 0)?;
        let st = update_final(st, last.text());
        Ok(result(st, last.text(), last.raw(), false))
    }

    /// Parse `cmd.tokens()[start..len-1]` from the first location that loads (SPEC §6.4 `parse_prefix`).
    #[allow(clippy::too_many_arguments)]
    fn parse_prefix(
        &mut self,
        cmd: &CommandLine,
        locations: Vec<SpecLocation>,
        start: usize,
        cwd: &Path,
        dev: bool,
        depth: usize,
    ) -> Result<ArgvState, CompletionError> {
        if depth > MAX_DEPTH {
            return Err(no("loadSpec recursion too deep"));
        }
        let texts: Vec<String> = cmd.tokens()[start..].iter().map(|t| t.text().to_string()).collect();
        let key = |loc: &SpecLocation| {
            format!(
                "{},{},{}",
                texts[..texts.len() - 1].join(" "),
                loc.cache_key(),
                cwd.display()
            )
        };
        if !dev {
            if let Some(st) = locations.iter().find_map(|l| self.cache.prefix_state(&key(l))) {
                return Ok(st.clone());
            }
        }
        let mut found = None;
        for loc in &locations {
            if let Some(spec) = self.load(loc)? {
                found = Some((spec, loc));
                break;
            }
        }
        let (spec, loc) =
            found.ok_or_else(|| no(format!("failed loading spec for {}", texts[0])))?;
        let (SpecLocation::Global { name } | SpecLocation::Local { name, .. }) = loc;
        let cache_key = key(loc);
        let mut st = ArgvState::initial(spec, &texts[0], name, texts.clone());
        let mut cur = cmd.clone();
        let root_ls = st.completion().load_spec().clone();
        if let Some(nested) = self.apply_load_spec(
            &mut st, root_ls, 0, None, &cur, start, cwd, dev, depth,
        )? {
            return Ok(nested);
        }
        let mut substituted: HashSet<String> = HashSet::new();
        let mut i = 1;
        while i < cur.tokens().len() - start {
            if st.completion().generate_spec().is_some() {
                let tokens: Vec<String> =
                    cur.tokens()[start..].iter().map(|t| t.text().to_string()).collect();
                st = self.generate_spec(st, &tokens, dev)?;
            }
            if i == cur.tokens().len() - start - 1 {
                break; // the final token never loads specs
            }
            let token = cur.tokens()[start + i].text().to_string();
            let last_arg = st.current_arg().cloned();
            let last_kind = if st.prefer_opt_arg() {
                AnnotationKind::OptionArg
            } else {
                AnnotationKind::SubcommandArg
            };
            let last_state = st.clone();
            st = update(&st, &token, false)
                .ok_or_else(|| no(format!("cannot consume {token:?}")))?;
            let last_type = st.annotations().last().map_or(AnnotationKind::None, |a| a.kind());
            let mut alias_err = None;
            if let Some(alias) = last_arg
                .as_ref()
                .and_then(|a| a.alias_directive().clone())
                .filter(|_| last_type == last_kind && !substituted.contains(&token))
            {
                match self.substitute_alias(&cur, start + i, &token, &alias) {
                    Ok(next) => {
                        substituted.insert(token);
                        cur = next;
                        st = last_state;
                        continue; // re-parse this position with the expanded words
                    }
                    Err(e) if substituted.is_empty() || matches!(e, CompletionError::Aborted) => {
                        return Err(e)
                    }
                    Err(e) => alias_err = Some(e),
                }
            }
            let mut ls = if last_type == AnnotationKind::Subcommand {
                st.completion().load_spec().clone()
            } else {
                None
            };
            if let Some(a) = last_arg.filter(|_| last_type == last_kind) {
                if a.load_spec().is_some() {
                    ls = a.load_spec().clone();
                } else if a.is_command() || a.is_script() {
                    ls = Some(SpecLoadDirective::Locations(vec![spec_location(
                        &token,
                        cwd,
                        Some(a.is_script()),
                    )]));
                } else if let Some(prefix) = a.is_module() {
                    ls = Some(SpecLoadDirective::Locations(vec![SpecLocation::Global {
                        name: format!("{prefix}{token}"),
                    }]));
                }
            }
            if let Some(nested) = self.apply_load_spec(
                &mut st,
                ls,
                i,
                Some(&token),
                &cur,
                start,
                cwd,
                dev,
                depth,
            )? {
                return Ok(nested);
            }
            if let Some(e) = alias_err {
                return Err(e);
            }
            substituted.clear();
            i += 1;
        }
        st = st.with_tokens(cur.tokens()[start..].iter().map(|t| t.text().to_string()).collect());
        if !dev {
            self.cache.remember_prefix_state(cache_key, st.clone());
        }
        Ok(st)
    }

    /// `parserDirectives.alias` (git aliases): the token's expansion re-tokenized into the command.
    fn substitute_alias(
        &mut self,
        cmd: &CommandLine,
        index: usize,
        token: &str,
        alias: &TextOrFunction,
    ) -> Result<CommandLine, CompletionError> {
        let value = match alias {
            TextOrFunction::Str(s) => s.clone(),
            TextOrFunction::Fn(f) => {
                let SpecFunctionResult::Alias(Some(value)) =
                    self.call(SpecFunctionCall::Alias {
                        function: *f,
                        token: token.to_string(),
                    })?
                else {
                    return Err(no("alias did not return a string"));
                };
                value
            }
        };
        // `git config --get` output ends with "\n"; a newline would not split words in the re-tokenized command.
        self.substitute(cmd, index, value.trim_end())
            .ok_or_else(|| no(format!("invalid alias for {token:?}")))
    }

    /// `Some(state)` when a location loadSpec took over parsing (SPEC §6.4); inline subcommands replace the level.
    #[allow(clippy::too_many_arguments)]
    fn apply_load_spec(
        &mut self,
        st: &mut ArgvState,
        ls: Option<SpecLoadDirective>,
        index: usize,
        token: Option<&str>,
        cmd: &CommandLine,
        start: usize,
        cwd: &Path,
        dev: bool,
        depth: usize,
    ) -> Result<Option<ArgvState>, CompletionError> {
        let ls = match (ls, token) {
            (Some(SpecLoadDirective::Fn(f)), Some(t)) => {
                match self.call(SpecFunctionCall::LoadSpec {
                    function: f,
                    token: t.to_string(),
                })? {
                    SpecFunctionResult::LoadSpec(directive) => directive,
                    _ => None,
                }
            }
            (Some(SpecLoadDirective::Fn(_)), None) => None, // function loadSpecs are skipped at the root
            (ls, _) => ls,
        };
        match ls {
            Some(SpecLoadDirective::Locations(l)) => {
                let nested =
                    self.parse_prefix(cmd, l, start + index, cwd, dev, depth + 1)?;
                let prefix = cmd.tokens()[start..start + index]
                    .iter()
                    .map(|t| t.text().to_string());
                Ok(Some(nested.with_command_prefix(index, prefix)))
            }
            Some(SpecLoadDirective::Inline(sub)) => {
                st.apply_inline_spec(sub);
                Ok(None)
            }
            Some(SpecLoadDirective::Fn(_)) | None => Ok(None),
        }
    }

    /// Merge `generateSpec(tokens, exec)` into the current level (SPEC §6.4); errors (except an abort) leave the
    /// state unchanged.
    fn generate_spec(
        &mut self,
        mut st: ArgvState,
        tokens: &[String],
        dev: bool,
    ) -> Result<ArgvState, CompletionError> {
        let Some(f) = st.completion().generate_spec() else {
            return Ok(st);
        };
        let cache_key = match st.completion().generate_spec_cache_key() {
            Some(TextOrFunction::Str(k)) => Some(k.clone()),
            Some(TextOrFunction::Fn(kf)) => {
                match self.call(SpecFunctionCall::GenerateSpecCacheKey {
                    function: *kf,
                    tokens: tokens.to_vec(),
                }) {
                    Err(CompletionError::Aborted) => return Err(CompletionError::Aborted),
                    Ok(SpecFunctionResult::GenerateSpecCacheKey(k)) => k,
                    Ok(_) | Err(CompletionError::NoSuggestions(_)) => None,
                }
            }
            None => None,
        }
        .filter(|_| !dev)
        .map(|k| format!("{}:{k}", tokens[0]));
        let generator = match cache_key.as_ref().and_then(|k| self.cache.generated_spec(k)) {
            Some(g) => g.clone(),
            None => {
                let generated = self
                    .call(SpecFunctionCall::GenerateSpec {
                        function: f,
                        tokens: tokens.to_vec(),
                    })
                    .and_then(|r| match r {
                        SpecFunctionResult::GenerateSpec(s) => s.map(|spec| *spec).map_err(no),
                        _ => Err(no("spec is not an object")),
                    });
                match generated {
                    Ok(g) => {
                        let g = Rc::new(g);
                        if let Some(k) = cache_key {
                            self.cache.remember_generated_spec(k, g.clone());
                        }
                        g
                    }
                    Err(CompletionError::Aborted) => return Err(CompletionError::Aborted),
                    Err(e) => {
                        (self.report)(CompletionNotice::GenerateSpecFailed(format!("generateSpec ({}): {e:?}", tokens[0]))); // spec name only: logs never hold typed text
                        return Ok(st);
                    }
                }
            }
        };
        st.merge_generated_spec(&generator);
        Ok(st)
    }
}
