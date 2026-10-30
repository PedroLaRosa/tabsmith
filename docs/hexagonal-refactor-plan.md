# tabsmith → DDD + hexagonal architecture: refactor plan

Status: Phases 0, 1, 2, 3, 4, 5, 6, 7 and 8 are done (gate green at `da2f950`). Phases 0-7 ran on branch
`refactor/hexagonal-architecture`, Phase 8 on `refactor/hexagonal-dropdown-terminal-session`. Phase 9 is next. Working
notes, tools, the per-file source map and the Phase 3, 4, 5, 6, 7 and 8 research are in
[`hexagonal-refactor/`](hexagonal-refactor/README.md). Source inventory taken on 2026-10-24 from `main` @ `4f26859`.

## Context

tabsmith is one crate: 21 flat files in `src/`, about 26k lines, about 330 tests. The files are grouped by technical
role (`pty.rs`, `render.rs`, `engine.rs`, `types.rs`, …), which causes these problems:

- Business rules, I/O, threads and terminal protocol are mixed together. `pty::Session` has 44 fields and 22
  responsibilities; `Engine` has 22 fields.
- There are no ports. The only trait in the crate is the private `parser::Host`.
- Paths come from env vars read wherever they are needed (`settings::home()`).
- There are dependency cycles: `cli` ↔ `integrations`, and `suggest` → `integrations::quote`.
- Rules and parsers are duplicated: theme parsing, setting defaults, launch rules, three atomic-write helpers, two
  versions of the generator-context JSON.

Goal: reorganise into **10 business modules (vertical slices)**, each with `domain/`, `application/` and
`infrastructure/`, following `.claude/skills/hexagonal-architecture` adapted to Rust.

- Use a ubiquitous language that every type name follows.
- Behaviour must not change. Every step keeps `cargo test` green and leaves user-visible output unchanged (`init`
  snapshots, `settings list --all`, the dropdown).

Decisions taken with the user:
1. **The domain uses std only**, plus `unicode-width` for display width. serde, serde_json and every I/O or framework
   crate live in infrastructure only.
2. **Module map.** 10 modules plus a `bootstrap/` composition root.
3. **One application use case per event** in both real-time loops (the PTY session and the completion engine). The
   loops themselves become thin infrastructure drivers.

---

## 1. Rust adaptation of the skill (conventions for every module)

| Skill (TypeScript) | Rust in this repo |
|---|---|
| `interface` | `trait` |
| class with private constructor + `create()` | struct with private fields + `pub(crate) fn new(..) -> Result<Self, <Module>Error>` (or `-> Self` when the value cannot be invalid) |
| `Maybe<T>` | `Option<T>` |
| throw `DomainError` | `Result<T, <Module>Error>`. One `enum` per module in `domain/<module>_error.rs`, with hand-written `Display` and `std::error::Error` (no new crate) |
| exhaustive switch | `match` with no `_` arm on domain enums |
| `toPrimitives()` | only on entities that are persisted or exposed through a DTO (`HistoryEntry`, `SessionContext`, `Settings`, `Integration`): `to_primitives()` returns a plain struct. Other types expose read-only accessors (YAGNI). |
| generic `Id` VO | identities here are natural keys, so use specific VOs: `SessionId`, `SpecLocation`, `GeneratorKey`, (shell, path) for `Integration`. No generated ids exist. |
| one file per class, PascalCase | **one file per type, snake_case file name** (`edit_buffer.rs` holds `EditBuffer`). Small private helper enums stay in their owner's file. |
| folder = module | directory modules with `mod.rs`, which only declares children (no glob re-exports) |
| Repository interface + InMemory in same file | trait + `InMemory<Name>Repository` in the same file. The InMemory version is `#[cfg(test)]` unless production uses it. |
| External port in `application/ports/` | trait in `application/ports/<port>.rs`, with its test double in the same file under `#[cfg(test)]` (`FixedClock`, `ScriptedLlmClient`, `RecordingCompletionEngine`, …) |
| constructor DI | use cases are structs holding `Rc<dyn Port>` (same thread) or `Arc<dyn Port + Send + Sync>` (crosses threads). No generics on use cases. |
| state loaded through repositories | **in-process aggregates** (`TerminalSession`, `CompletionSession`, `Menu`) are owned by the driver and passed to `execute` as `&mut`. Repositories exist only for persisted state. |
| `register<Module>Module(app, deps)` | `<module>/infrastructure/<module>_module.rs` contains `build_<module>_module(&SharedDependencies) -> <Module>Module`, a struct of built use cases. CLI handlers are `run_<command>(args, &<Module>Module) -> anyhow::Result<i32>`. |
| `http/` controllers | `infrastructure/cli/` holds the clap `Args`/`Subcommand` types and handlers (driving adapters). `terminal_session/infrastructure/pty/` holds the PTY event loop (also a driving adapter). |
| `shared/infrastructure/server` + `main` | `src/main.rs` (thin) + `src/bootstrap/`: clap root and dispatch, engine server, and cross-module bridges. `shared/` stays a kernel and never imports a feature module. |
| tests folder | **tests stay in the same file** (`#[cfg(test)] mod tests` at the bottom). A test module over about 500 lines goes to the sibling `<file>/tests.rs` (precedent: `src/parser/tests.rs`). `tests/fixtures/` and `tests/snapshots/` stay as data. No `tests/*.rs`. |

Other rules:
- **Errors.** `anyhow` is used only in `infrastructure/cli`, `infrastructure/pty` and `bootstrap`.
- **Logging.** Only infrastructure logs. Domain and application return errors or warnings, and the adapter or driver
  logs them.
- **Output.** No `println!`/`eprintln!` in domain or application. Output goes through presenters in `infrastructure/cli`
  or through ports.
- **Visibility.** Everything is `pub(crate)` at most. Domain struct fields are private.
- **Cross-module imports.** A module may import another module's domain types, repository traits, port traits and
  DTOs (DTOs are the contracts of ports and use cases, for example `CompletionRequest` in the `CompletionEngine`
  port). It must never import another module's use cases or infrastructure. When one event needs several modules, the driver
  in `bootstrap/` calls each module's use case in turn; a use case never calls another use case.
- **Dependencies.** No new crates (AGENTS.md). Versions in `Cargo.toml` stay as they are.
- **Naming.** Ports have no suffix (`Clock`, `LlmClient`); adapters get a technology prefix (`QuickJsSpecRuntime`,
  `JsonlHistoryRepository`); use cases get the `UseCase` suffix; DTOs are named after what they carry (no `Dto`
  suffix needed when unambiguous).
- **DTOs.** DTOs hold primitives. One exception: setting values are inherently tree-shaped, so the setting DTOs reuse
  the behaviour-free `SettingValue` enum.

### Canonical shape (the skill's TypeScript examples, in Rust)

```rust
// src/settings/domain/repositories/settings_repository.rs
pub(crate) trait SettingsRepository {
    fn load(&self) -> Result<Settings, SettingsError>;
    fn save(&self, settings: &Settings) -> Result<(), SettingsError>;
}

#[cfg(test)]
pub(crate) struct InMemorySettingsRepository { stored: std::cell::RefCell<Settings> }
#[cfg(test)]
impl SettingsRepository for InMemorySettingsRepository { /* clone in / clone out */ }

// src/settings/application/set_setting_use_case.rs
pub(crate) struct SetSettingUseCase { settings: Rc<dyn SettingsRepository> }

impl SetSettingUseCase {
    pub(crate) fn new(settings: Rc<dyn SettingsRepository>) -> Self { Self { settings } }

    pub(crate) fn execute(&self, key: &str, value: SettingValue) -> Result<(), SettingsError> {
        let key = SettingKey::new(key)?;
        let updated = self.settings.load()?.with(key, value);
        self.settings.save(&updated)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn should_store_the_value_under_the_given_key() {
        // Arrange: InMemorySettingsRepository, use case
        // Act: execute("autocomplete.height", SettingValue::Number(10.0))
        // Assert: repository.load()?.get("autocomplete.height")
    }
}

// src/settings/infrastructure/settings_module.rs
pub(crate) struct SettingsModule { pub(crate) set_setting: SetSettingUseCase, /* … */ }
pub(crate) fn build_settings_module(deps: &SharedDependencies) -> SettingsModule { /* adapters → use cases */ }

// src/settings/infrastructure/cli/settings_command.rs
#[derive(clap::Args)] pub(crate) struct SettingsArgs { /* moved from cli.rs */ }
pub(crate) fn run_settings_command(args: SettingsArgs, module: &SettingsModule) -> anyhow::Result<i32> { /* parse → use case → render */ }
```

### Architecture fitness test (one runnable check that enforces all of the above)

`src/architecture_tests.rs` (`#[cfg(test)]`) walks `src/` with `std::fs` and fails on:
1. a file in `*/domain/` that uses another layer (`::application`, `::infrastructure`), `crate::bootstrap`, a legacy
   flat module (`crate::types`, `crate::js`, `crate::engine`, ..), an external crate other than
   `std`/`core`/`alloc`/`unicode_width`, or `println!`/`eprintln!`/`logger::`;
2. a file in `*/application/` that uses `::infrastructure`, `crate::bootstrap`, a legacy flat module, or external crates
   (same allow-list);
3. a module other than `bootstrap` that references another module's use case (a `*_use_case` file or `*UseCase` type
   segment anywhere under it, so a module root or legacy shim re-exporting it under its own name is caught too) or its
   `::infrastructure`; a use case re-exported under another name, an `as` rename, a `super::` path and an adapter
   re-exported by a legacy file or a legacy module root still pass;
4. `shared/` that references a feature module;
5. a registered module with a `src/<module>.rs`, or a file in `src/<module>/` other than `mod.rs` and the files inside
   its `domain/`, `application/` and `infrastructure/` folders (rules 1-2 read the layer from the folder name, so a file
   beside the layers or in a misspelled or extra folder would escape them);
6. anything but attributes and child declarations (`mod x;`, `pub(crate) mod x;`) in a registered module's `mod.rs`:
   a re-export, a `type` alias or a wrapper fn would let another module reach a use case by a path that rule 3
   cannot read as one;
7. anything in `src/main.rs` but attributes, child declarations and `fn main`, and a `use` item anywhere in it: every
   module sees the crate root's private items as `crate::<name>` (`crate::logger`, `crate::Cli`), a path that rules
   1-3 cannot read as the shared infrastructure or bootstrap, so `main` names what it calls by full path;
8. a `#[path]` attribute, an `include!`, or a `#[macro_export]` or `#[macro_use]` macro in a registered module or in
   `src/main.rs`: each compiles code the rules never read into a module (a file outside it, or a macro body the
   domain calls as `crate::name!` or `name!`, a path that names no logger or infrastructure).

It starts with the module list empty, and each phase adds the module it finishes.

---

## 2. Module map and dependency direction

```
src/
├── main.rs              thin: Cli::parse, logger init, bootstrap::run, exit code
├── bootstrap/           composition root: clap root + dispatch, engine server, cross-module bridges
├── shared/              kernel: Shell, ShellQuoting, DataDirectory, Clock + infra utilities
├── settings/            settings.json, catalog, get/set/delete/list/open
├── history/             history.jsonl, record executed commands
├── shell_integration/   init scripts, dotfiles, launch decision, setup/uninstall, status
├── specs/               Fig spec model, bundle, spec loading, QuickJS runtime, process exec
├── completion/          tokenizer, argv parser, generators, suggestions, ranking, insertion
├── ghost_text/          inline ghost text: history/spec sources, LLM, cache
├── dropdown/            menu state machine, keymap, themes, layout, icons
├── terminal_session/    PTY wrapper, OSC 697 markers, edit buffer, overlay drawing, session context
└── diagnostics/         doctor, diagnostic report, debug logs
```

Allowed module dependencies. The graph is acyclic; the architecture test checks the layer rules.

```
settings, history                  → shared
shell_integration                  → shared, settings
specs                              → shared, settings
completion                         → shared, settings, specs, history
ghost_text                         → shared, settings, history, specs, completion
dropdown                           → shared, settings, specs, completion
terminal_session                   → shared, settings, history, completion, ghost_text, dropdown
diagnostics                        → shared, settings, shell_integration, specs, ghost_text, terminal_session
bootstrap                          → everything (composition root only)
```

Cross-module **bridges**. Each port is declared by the module that uses it; `bootstrap/bridges.rs` implements it by
calling the other module's use case or adapter.

| Port (declared in) | Implemented in bootstrap by |
|---|---|
| `shell_integration::SpecBundleInstaller` | `specs::InstallSpecBundleUseCase` |
| `diagnostics::CheckFixer` | shell_integration install/reinstall use cases, settings repository reset, `specs::InstallSpecBundleUseCase`, data-dir creation |
| `terminal_session::CompletionEngine` | `bootstrap::ChannelCompletionEngine` (sends `EngineMessage` to the engine server thread) |
| `terminal_session::GeneratorProcessReaper` | `specs` `ProcessExecutor::kill_live_children` |
| `specs::RuntimeEventSource` | `bootstrap::EngineInbox` (`engine_inbox.rs`, the engine server's single channel; keeps today's abort-on-newer-request semantics) |
| theme and settings repositories used by `terminal_session` | `dropdown::FileThemeRepository`, `settings::JsonFileSettingsRepository` (injected as trait objects) |

---

## 3. Ubiquitous language (glossary)

See [`ubiquitous-language.md`](ubiquitous-language.md): every entity and value object per module, the type it
replaces, and the qualified names for words with more than one meaning (lock, prompt, ghost, history, session,
suggestion, completion, cache). Every type named in the sections below must match it.

---

## 4. Target layout per module

`←` means "moved/extracted from". Only files with content are created; there are no empty folders.

### shared
```
shared/domain/value_objects/{shell.rs ← types::Shell, data_directory.rs ← settings path fns + joins}
shared/domain/services/shell_quoting.rs ← integrations::{quote, pwsh_sq}
shared/application/ports/clock.rs        trait Clock + #[cfg(test)] FixedClock
shared/infrastructure/
  shared_dependencies.rs                  struct SharedDependencies { data_directory, clock }
  system_clock.rs
  data_directory_from_env.rs              ← TABSMITH_HOME / TABSMITH_SETTINGS_PATH / HOME / USERPROFILE (settings.rs:8-35)
  error_chain.rs                          display_chain: an error and each source() as anyhow's {:#} (Phase 4 review)
  json_document_store.rs                  ← settings::Store (flat dotted-key JSON file; corrupted detection)
  atomic_file.rs                          unifies Store::save, integrations::write_private, integrations::write
                                          (temp+rename, 0600/0700, keeps symlinks, purge guard)
  logger.rs                               ← logger.rs (global OnceLock stays)
  output_format.rs                        ← cli::OutputFormat  (breaks the cli ↔ integrations cycle)
  terminal_style.rs                       ← cli::{stdout_tty, sgr, bold}, integrations::{paint, epaint}
  os/{executable_lookup.rs ← integrations::which (+ js::find_exe dedupe),
      process_runner.rs ← output_with_timeout,
      process_info.rs ← is_root, is_console, exe_path, parent_exe, windows_parent_exe}
  testing/temp_dir.rs (#[cfg(test)])      ← specs::testutil::TmpDir; replaces the 5 ad-hoc temp-dir helpers
```

### settings
```
domain/value_objects/{setting_key, setting_value, setting_number, setting_key_pattern ← wildcard_match,
                      setting_definition ← CatalogEntry, settings ← Settings}.rs
domain/services/settings_catalog.rs       ← CATALOG + key constants + default(key)
domain/repositories/settings_repository.rs  trait {load, save, last_modified} + InMemory
domain/settings_error.rs
application/{get_setting, set_setting, delete_settings, list_settings, list_setting_definitions,
             open_settings_file}_use_case.rs
application/ports/settings_editor.rs
infrastructure/settings_module.rs
infrastructure/load_or_defaults.rs        the legacy pty.rs/doctor.rs lenient load: defaults + the old warning
                                          (no caller once Phases 9-10 move it into their own infrastructure)
infrastructure/adapters/{json_file_settings_repository.rs, setting_value_json.rs (SettingValue ↔ serde_json,
                         "JSON else string" parsing), system_settings_editor.rs ← cli::open_settings}
infrastructure/cli/settings_command.rs    ← cli::{SettingsArgs, SettingsCmd, settings, render_get, render_list,
                                           render_list_all, removal_message}
(cli::matching_keys is inlined into DeleteSettingsUseCase::execute)
```

### history
```
domain/entities/history_entry.rs          ← types::HistoryEntry (no serde)
domain/repositories/history_repository.rs trait {append, recent} + InMemory (← History::from_entries)
domain/history_error.rs
application/record_command_use_case.rs    (blank commands are skipped)
infrastructure/history_module.rs
infrastructure/adapters/jsonl_history_repository.rs ← inline::History (load, append, compact 10k→5k, 0600,
                                                       purge guard) + HistoryRecord (serde)
```

### shell_integration
```
domain/entities/integration.rs            ← Integration (+ describe, file_name, rc, script_path, source_line)
domain/value_objects/{integration_phase, integration_target, integration_status, dotfile, integration_script,
                      init_script, launch_facts, launch_decision, restart_reminder}.rs
domain/services/{integration_catalog ← integrations() §5.5 table (existence facts passed in),
                 launch_policy ← launch_decision, init_script_renderer ← init_output minus I/O,
                 manual_install_instructions ← manual_text}.rs
domain/repositories/{dotfile_repository, backup_repository, restart_reminder_repository}.rs (+ InMemory)
domain/shell_integration_error.rs
application/{install_integrations, uninstall_integrations, reinstall_integrations, get_integration_status,
             render_init_script, decide_launch, set_up_tabsmith, uninstall_tabsmith}_use_case.rs
application/integration_status.rs (DTO)
application/ports/{user_confirmation ← confirm, shell_environment (which, parent shell, current exe, pwsh $PROFILE,
                   is_root, stdin tty, env snapshot), spec_bundle_installer, data_directory_purger}.rs
infrastructure/shell_integration_module.rs
infrastructure/adapters/{file_dotfile_repository, file_backup_repository, state_restart_reminder_repository,
                         stdin_user_confirmation ← confirm/answer, os_shell_environment,
                         embedded_shell_scripts ← include_str!("../shell/*") + real-shell tests in its tests.rs}.rs
infrastructure/cli/{init_command ← InitArgs/ShellArg, integrations_command ← IntegrationsCmd/status,
                    setup_command, uninstall_command, should_launch_command, get_shell_command}.rs
```

### specs
```
domain/entities/subcommand.rs             ← spec::Subcommand (+ reindex, merged_with_generated)
domain/value_objects/{spec_option, arg, generator, spec_suggestion, suggestion_kind, spec_location,
                      spec_function_ref, text_or_function, generator_trigger, generator_cache_policy,
                      generator_script, repeatability, filter_strategy, parser_directives, spec_load_directive,
                      spec_index, semantic_version, spec_value, spec_lookup, generator_context,
                      exec_request (+ with_timeout(Option<u64>), Phase 6.4), process_output, task_id, task_outcome,
                      spec_function_call, spec_function_kind, spec_function_result, script_resolution,
                      custom_generator_result, template_suggestions}.rs
domain/services/{spec_locator ← parser::spec_location + specs::resolve_in name rules and lookup order
                 (is_file predicate injected; the bin/console rule, but not "dotslash": the single-token dotslash
                 rule stays with the parser, then ParsePrefixCache),
                 spec_version_selector ← apply_versions key pick (every diff ≤ target, cumulative, semver order),
                 bundle_retention_policy ← specs cleanup staleness rules}.rs
domain/repositories/spec_repository.rs    trait {find(&SpecLocation, &SpecLookup) -> Result<Option<Rc<Subcommand>>,
                                          SpecError>; forget_loaded_specs()} + #[cfg(test)] InMemorySpecRepository
                                          (disableForCommands: SpecLookup::disables, called first by every find,
                                          deviation 3)
domain/spec_error.rs                      (Aborted, Timeout, Invalid, BundleUnavailable)
application/install_spec_bundle_use_case.rs delegates to SpecBundle::install (the ensure_extracted branching stays in
                                          embedded_spec_bundle, deviation 26)
application/ports/spec_runtime.rs         typed task API: start(SpecFunctionCall, Option<&GeneratorContext>) -> TaskId,
                                          start_exec(ExecRequest, &GeneratorContext), start_sleep, wait(tasks, deadline)
                                          -> Result<Vec<Option<TaskOutcome>>, Interrupted>, forget, call,
                                          read(&SpecValue, SpecFunctionKind, &GeneratorContext) -> SpecFunctionResult,
                                          read_exec_stdout(&SpecValue) -> String (Phase 6.4; no resolve_script, a
                                          script value is read as SpecFunctionKind::Script), apply_settings;
                                          + #[cfg(test)] InMemorySpecRuntime (scripted outcomes from Phase 6.4)
application/ports/{spec_bundle (+ #[cfg(test)] RecordingSpecBundle),
                   runtime_event_source (+ #[cfg(test)] ChannelRuntimeEventSource),
                   runtime_event, runtime_event_sink, runtime_event_poll, interrupted}.rs
infrastructure/specs_module.rs            SpecsModule { install_spec_bundle } + build_specs_module
infrastructure/adapters/
  fig_spec_mapper.rs        ← spec::{normalise, subcommand, option, arg, generator, …, suggestion_from_json,
                               load_spec_result, fn_id}, ItemType::from_fig, suggest::{fig_type, template_json},
                               engine::{truthy, script_exec_input, json_items, post_process_items} (JSON halves;
                               script_string became SpecSuggestion::from_script_output), generator_context_json,
                               SpecValue ↔ JSON, function_result (a JSON result read by kind); tests in
                               fig_spec_mapper/tests.rs (the 8 spec tests among them)
  fig_version_diff.rs       ← parser::{apply_versions, merge_*, shallow_merge, make_array, names, truthy}
  quickjs_spec_runtime.rs   ← js::Js + task registry; call_arguments (a typed call's JS arguments); JSON ↔ typed
                               results via fig_spec_mapper. The production generatorContext JSON builder is
                               fig_spec_mapper::generator_context_json; the test-only one stays in the runtime tests
                               (deviations 4 and 25). Tests in quickjs_spec_runtime/tests.rs
  quickjs_module_wrapper.rs ← wrap_module, PRELUDE
  quickjs_spec_repository.rs ← parser::Loader (spec cache, Rc identity by JSON, diff-versioned specs, CLI version
                               probe), spec_file, cli_version
  process_executor.rs       ← js::{exec, build_command, key_eq, find_exe, wait_output, git_bash, clean, LIVE,
                               kill_live_children, job}
  native_ls.rs              ← js::native_ls
  exec_input.rs             ← js::ExecInput (the serde record, into ExecRequest) + the exec result JSON
  embedded_spec_bundle.rs   ← specs::{ARCHIVE, version, ensure_extracted, extract_to, touch, cleanup, unpack,
                               SpecIndex::load_from, resolve (fs part), dev_folder, user_dir}
```

### completion
```
domain/entities/{completion_session ← Engine state (settings, wrapper and session env, aliases memo, recency, fuzzy
                 toggle, parse-prefix cache, generator slots, runs and cache, served request, first-token cache),
                 generator_slot ← Slot, generator_run ← Run + the pure step transitions (template list, script, exec
                 stdout, custom result) returning GeneratorCommand {StartCall, StartExec, StartSleep, Settle},
                 generator_cache ← GenCache (generic GeneratorCache<V>), parse_prefix_cache ← ParseCaches,
                 suggestion_recency ← Recency (the newest-wins merge stays in the recency adapter)}.rs
domain/value_objects/{token, command_line, alias_table, annotation_kind, token_annotation, parsed_command_line,
                      suggestion_candidate, suggestion_query, filter_options, insertion_context, generator_slot_view,
                      insertion, suggestion, argument_hint, completion_phase, completion_result, generator_key,
                      cache_lookup, generator_step, generator_outcome, generator_command, served_request}.rs
domain/services/
  command_line_tokenizer.rs ← shell_parser.rs (whole, git mv; AST stays private; 16 tests in tests.rs sibling)
  argv_parser.rs            ← parser's argv state machine (ArgvState + private ArgState, update…result)
  suggestion_assembler.rs   ← assemble, static_options, short_name, option_chain (+ private cmp_first)
  history_suggestions.rs    ← history_rows, mix_history
  fuzzy_matcher.rs          ← fuzzysort
  suggestion_filter.rs      ← filter, partial, prefix_score, auto_exec, cmp_locale
  suggestion_ranker.rs      ← update_priorities, dedupe, rank (+ private same_row)
  insertion_composer.rs     ← insertion, name_match, is_var_ref, unquote, escape_* (not merged into ShellQuoting)
  suggestion_finalizer.rs   ← to_item, highlight, args_hint
  generator_trigger_policy.rs ← triggered, dir_changed, cache_key, is_path_template, script_timeout (f64), SYNC_CALL,
                              effective_debounce, run_budget
  path_template.rs          ← shell_expand, inserted_directory, help_items
  first_token_suggestions.rs ← first_token_items minus the PATH scan (takes the executables), is_root_spec
  completion_request_policy.rs ← compute's pure parts: is_mid_word, uses_fuzzy_search, history_prefix, env_var
                              (the argument key is generator_key::argument_key)
  completion_result_builder.rs ← reply minus the ghost: build_completion_result
domain/repositories/recency_repository.rs trait {load, save, flush} + #[cfg(test)] InMemoryRecencyRepository
domain/completion_error.rs                ← ParseError (Aborted, NoSuggestions); Debug only, no Display
domain/test_fixtures.rs (#[cfg(test)])    ← suggest's test helpers (pr_for, opts, sub, opt, arg, names, ctx); Phase 8
                                          adds menu::tests::{comp, item}
application/command_line_parser.rs        application service ← Parser/ParseCaches::{parse, parse_prefix,
                                          substitute_alias, apply_load_spec, generate_spec} as a private ParsePass over
                                          SpecRepository + SpecRuntime; reports a failed generateSpec as a
                                          CompletionNotice (parser/tests.rs → command_line_parser/tests.rs, git mv)
application/{parse_context, completion_notice (DTO), completion_request (DTO)}.rs
application/generator_runner.rs           application service: starts, feeds and settles generator runs and executes
                                          their GeneratorCommands; Rc-shared by the two generator use cases
application/{complete_command_line, progress_generators, reset_generators, record_suggestion_insertion,
             apply_completion_settings, apply_session_context, toggle_fuzzy_search}_use_case.rs
application/test_fixtures.rs (#[cfg(test)]) the use-case harness over the InMemory doubles
application/ports/{directory_lister ← list_dir (+ InMemoryDirectoryLister), executable_catalog ← executables (PATH
                   scan) (+ FixedExecutableCatalog)}.rs
infrastructure/completion_module.rs       CompletionModule {the seven use cases, recency} + build_completion_module
infrastructure/adapters/{fs_directory_lister ← list_dir, path_executable_catalog ← the PATH scan + exe_name,
                         state_file_recency_repository ← load_recency/save_recency + throttled saver thread
                         + the newest-wins merge}.rs
```

### ghost_text
```
domain/entities/ghost_text_cache.rs       ← GhostCache
domain/value_objects/{ghost_text ← pty next_word/ghost_rest (ghost_key stays in pty.rs until Phase 9),
                      ghost_text_source ← Source + BUILTIN_SOURCES,
                      llm_profile ← Profile + parse_profile (from SettingValue), llm_provider ← Provider,
                      llm_prompt (carries SYSTEM)}.rs
domain/services/{ghost_text_source_resolver ← profiles/source/cli::selected_source, history_ghost_text,
                 spec_ghost_text, llm_prompt_builder ← llm_history/is_private/usable/history_prompt/limits,
                 llm_completion_cleaner ← clean, cd_target_validator ← the pure half of validate/split_words/
                 expand}.rs
domain/ghost_text_error.rs
application/{suggest_local_ghost_text ← engine::ghost, request_llm_ghost_text ← engine::llm + llm_complete pipeline,
             clear_ghost_text_cache, set_inline_enabled, get_inline_status, select_ghost_text_source,
             list_ghost_text_sources}_use_case.rs
application/{ghost_text_sources (DTO), llm_ghost_text_request (DTO), llm_ghost_text_answer (Immediate | Deferred),
             llm_ghost_text_job ← the worker half of llm_complete + the probe-backed validate}.rs
application/ports/{llm_client.rs (+ ScriptedLlmClient), directory_probe.rs (+ InMemoryDirectoryProbe),
                   source_chooser.rs (+ FixedSourceChooser) ← choose_source}
infrastructure/{ghost_text_module, inline_command_module}.rs
infrastructure/adapters/{http_llm_client ← build_request/parse_response + ureq (both providers),
                         fs_directory_probe, stdin_source_chooser ← choose_source}.rs
infrastructure/cli/inline_command.rs      ← InlineCmd, inline, render_sources, PROFILE_EXAMPLE
```

### dropdown
```
domain/entities/menu.rs                   ← menu::Menu (Instant passed in, never read inside; max_rows stays here;
                                            the tests are in menu/tests.rs)
domain/value_objects/{menu_visibility, menu_effect, key, key_code, modifiers, key_action, key_binding, keymap,
                      common_prefix_outcome, theme (ThemeParts), rgb (a tuple alias), theme_choice, theme_definition,
                      theme_author, theme_palette (FigPaletteParts, Base16PaletteParts), menu_layout (MenuLayoutParts),
                      menu_side, icon_slot, icon_id (holds ICON_NAMES, placeholders, color), graphics_support}.rs
domain/services/{common_prefix, keystroke_parser ← parse_keystroke/normalize (two free functions),
                 menu_layout_calculator ← render::layout, menu_describer ← render::describe,
                 color_parser ← parse_color/xterm256,
                 theme_resolver ← Theme::from_settings rules (resolve_theme: dark/light/system luma/custom; takes the
                 ThemeRepository trait object),
                 icon_resolver ← icons::{slot, spec_image, type_image, ascii_slot, wide_emoji},
                 text_fitting ← render::{clean, cw, width, fit, wrap}}.rs
domain/repositories/theme_repository.rs   trait {folder, find, list, find_palette} + InMemoryThemeRepository
domain/dropdown_error.rs
application/{select_theme, list_themes, show_theme}_use_case.rs
application/{theme_selection, current_theme}.rs (DTOs)
infrastructure/dropdown_module.rs
infrastructure/adapters/{file_theme_repository,
                         theme_json_mapper ← cli::{parse_theme, ThemeFile, ThemeAuthor} (the strict entry) and
                                             render::Theme::from_json (the lenient entry): two entry points, not one parser,
                         theme_file_error ← ThemeFileError, the strict entry's error}.rs
infrastructure/cli/theme_command.rs       ← ThemeArgs, theme, theme_banner, BUILTIN_THEMES (the names live in ThemeChoice)
```

### terminal_session
```
domain/entities/{terminal_session (aggregate), marker_splitter ← Splitter, marker_tracker ← Tracker state +
                 on_marker/end_prompt/new_cmd/pre_exec/flush_deferred (cursor column in, ShellEvents +
                 ScreenCommands out), insertion_lock, echo_hold, session_context ← SessionCtx}.rs
domain/value_objects/{session_id, key_event (built in Phase 8), shell_marker, output_piece, shell_event, edit_buffer, shell_suggestion_color,
                      screen_command, screen_state, key_route, wrapped_shell_command, terminal_background}.rs
domain/services/{insertion_reconciler ← insertion_bytes/expected_buffer/unechoed_after, key_router ← on_keys
                 1046-1110, render_guard ← suppressed/drawable, session_deadlines ← next_deadline}.rs
domain/repositories/session_context_repository.rs trait {save, find, remove, prune_older_than} + InMemory
domain/terminal_session_error.rs
application/  one use case per event (table below) + report_session_context_use_case.rs (`_ pre-cmd`)
application/ports/{terminal_screen.rs (pass_through bytes → stdout bytes, apply ScreenCommand, cursor_column,
                   edit_buffer, state, resize, draw_overlay, take_clear, full_repaint),
                   terminal_writer.rs, wrapped_shell.rs (write input, resize),
                   completion_engine.rs (+ RecordingCompletionEngine), generator_process_reaper.rs}
infrastructure/terminal_session_module.rs
infrastructure/pty/{session_runner ← run/start/run/step/guard/emit (event loop, panic pass-through, batching,
                    credits, ONE stdout write per batch), session_message ← types::Msg, pty_reader ← pump,
                    terminal_probe ← PROBE/parse_probe/parse_cpr/parse_osc11, fallback_shell ← fallback,
                    platform_unix, platform_windows ← sys}.rs
infrastructure/adapters/{alacritty_terminal_screen (implements TerminalScreen; owns Term = types::Model),
                         output_filter ← OutFilter, edit_buffer_extractor ← extract/strip_pwsh_prompts/cell colours,
                         cell_tagger ← tag/mark_line_start,
                         overlay_renderer ← render::Renderer + frame/restore code (built in Phase 8: OverlayRenderer;
                         FrameInput stays in its file until 9.4),
                         kitty_graphics ← icons::{transmit, place, base64, tmux_graphics, detect} (built in Phase 8:
                         KittyImage and KittyPixels share the file, plus CLEAR_PLACEMENTS and detect),
                         icon_assets ← PNGS/TEMPLATE_RGBA (built in Phase 8: also tinted and kitty_image),
                         stdin_key_decoder ← keys::Decoder/decode* (built in Phase 8),
                         stdout_terminal_writer, pty_wrapped_shell,
                         file_session_context_repository ← SessionCtx::path/load, pre_cmd writer, prune_run_files,
                         + SessionContextRecord (serde)}.rs
infrastructure/cli/{pty_command, pre_cmd_command, debug_shell_command ← cli::debug_shell/debug_shell_files,
                    key_tester_command ← cli::key_tester/key_line}.rs
```

### diagnostics
```
domain/entities/doctor_run.rs             ← doctor::Run policy (soft→warn unless strict, abort on unfixed, --all)
domain/value_objects/{check, check_group, check_severity, check_outcome, check_fix, system_facts, tool_version,
                      diagnostic_report}.rs
domain/services/{doctor_check_catalog ← dotfile/setup/env/system checks evaluated over SystemFacts
                 (session_kind, inline_outcome, is_local, short_path; launch rules reuse
                 shell_integration::LaunchPolicy instead of duplicating them), username_scrubber}.rs
application/{run_doctor, collect_diagnostic_report, show_recent_logs}_use_case.rs
application/ports/{system_facts_probe, doctor_reporter, check_fixer, log_reader}.rs
infrastructure/diagnostics_module.rs
infrastructure/adapters/{os_system_facts_probe, terminal_doctor_reporter ← print_outcome/Spinner/SIGINT cursor
                         restore, file_log_reader}.rs
infrastructure/cli/{doctor_command, diagnostic_command ← diagnostic/diagnostics/to_toml, debug_logs_command}.rs
```

### bootstrap (composition root)
```
main.rs                    Cli::parse → logger init → bootstrap::run → exit code
bootstrap/cli.rs           ← main.rs Cli/Command/InternalCmd + dispatch to module cli handlers; cli::completion,
                             version, cli::print_error
bootstrap/engine_server.rs ← engine.rs by git mv: spawn (builds the inbox, the runtime and the modules on the engine
                             thread), EngineServer {run, the dispatch table below, send_reply, log_notice}, owns the
                             inbox instance
bootstrap/engine_message.rs ← types::EngineMsg, renamed EngineMessage
bootstrap/engine_inbox.rs  EngineInbox, the engine thread's one channel; implements specs::RuntimeEventSource
bootstrap/channel_completion_engine.rs  implements terminal_session::CompletionEngine over Sender<EngineMessage>
                             + start_completion_engine, the starter the pty receives
bootstrap/bridges.rs       cross-module port implementations (section 2)
```

### Event → use case tables

PTY session (`terminal_session/infrastructure/pty/session_runner.rs`). The driver only decodes, routes and writes.
| Event (today's `Msg`) | Use case |
|---|---|
| startup | `PrepareSessionLaunchUseCase` (argv, env, title) |
| `Stdin(bytes)`: keys decoded and late probe replies stripped by the driver | `HandleKeystrokesUseCase` |
| `Pty(batch)` | `ProcessShellOutputUseCase` (markers → events → refresh → request → frame; one write) |
| `Resize(c, r)` | `ResizeTerminalUseCase` |
| `Completion(c)` | `ApplyCompletionResultUseCase` |
| `Ghost{req, text}` | `ApplyLlmGhostTextUseCase` |
| `ChildExit(code)` / `PtyEof` | `HandleShellExitUseCase` |
| timeout | `HandleTimerTickUseCase` (resize repaint, redraw grace, held keys, lock expiry, LLM debounce, spinner, settings/session reload) |
| `Term` and loop end | `EndSessionUseCase` (engine shutdown, reap generator processes, remove session context) |
| `_ pre-cmd` (separate process) | `ReportSessionContextUseCase` |

Completion engine (`bootstrap/engine_server.rs`). The `ghost_text::` steps are `ghost_text`'s use cases, held by
`GhostTextModule`.
| Event (`EngineMessage`) | Use cases, in order |
|---|---|
| `Complete(request)` | `completion::CompleteCommandLine` → `ghost_text::SuggestLocalGhostText` → reply (error replies get the local ghost too) |
| idle with generator tasks pending | `completion::ProgressGenerators` → `ghost_text::SuggestLocalGhostText` → reply if changed |
| `CommandFinished(entry)` | `history::RecordCommand` → `ghost_text::ClearGhostTextCache` → `completion::ResetGenerators` |
| `CommandEnded` | `ghost_text::ClearGhostTextCache` → `completion::ResetGenerators` |
| `RecordInsert` | `completion::RecordSuggestionInsertion` |
| `Llm` | `ghost_text::RequestLlmGhostText` on the engine thread; a `Deferred` answer runs on an `llm` worker thread |
| `Settings` / `Session` / `ToggleFuzzy` | `completion::ApplyCompletionSettings` / `ApplySessionContext` / `ToggleFuzzySearch` |
| `Shutdown` | driver stops (a `Complete` drained with it gets no reply); the recency adapter flushes |
| `Runtime(event)` | not an event of the engine: `EngineInbox` hands it to `QuickJsSpecRuntime` through `RuntimeEventSource` (the old `ExecDone`; `Timer` was deleted in Phase 5) |

---

## 5. Phases

Every step follows the same loop:

1. Move code verbatim first, with its tests. Then rename to the glossary names. Then extract the pure logic.
2. While a phase is in progress, the old file may become a shim of `pub(crate) use` re-exports. The phase ends by
   deleting that shim; shims never cross phases. The one exception is `types.rs`, which shrinks phase by phase and is
   deleted in Phase 11.
3. New seams (repository traits, InMemory doubles, port fakes, new VOs with validation) are written test-first,
   following the xp-tdd cycle RED → GREEN → COMMIT → REFACTOR → COMMIT. Pure moves need no new test; the moved tests
   are the safety net. Existing test names are kept. New tests follow testing-standard (`should_…`, AAA, no mocks).
4. Before each commit: `cargo build --locked && cargo test --locked` (pass count ≥ baseline), `cargo clippy
   --all-targets --locked` (no new warnings), and the architecture test.
5. Commit as `refactor(<module>): <why>` (Conventional Commits, like the existing log), with the attribution trailer.
   `docs/git-convention.md`, which AGENTS.md points to, does not exist yet; follow the log style.

Implementation session protocol (AGENTS.md):
- At the start, load the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture`.
- Commit after each step.
- Work on branch `refactor/hexagonal-architecture`. Never commit on `main`.

### Phase 0: branch and baseline (done)
1. `git switch -c refactor/hexagonal-architecture`.
2. Commit this plan and `docs/ubiquitous-language.md` (both written by the planning session) on the branch, if they
   are not committed yet.
3. Record the baseline in the plan doc:
   - `cargo test --locked` result lines;
   - `cargo test --locked --release -- --ignored all_bundle_modules`;
   - the `cargo clippy --all-targets --locked` warning count.

Baseline recorded on 2026-10-24 at `ef807c1` (rustc 1.95.0, macOS):
- `cargo test --locked`: 284 tests, `283 passed; 0 failed; 1 ignored`.
- `cargo test --locked --release -- --ignored all_bundle_modules`: `1 passed; 0 failed`.
- `cargo clippy --all-targets --locked`: 3 warnings (2 × `useless use of format!`, 1 × `use of default to create a
  unit struct`).
- Output capture for the equivalence check: `--help` of every command, `init` for every shell and phase, `settings`
  get/set/delete/list (plain, json, json-pretty), `theme`, `inline`, `integrations status/install/uninstall` (with the
  written dotfiles and their modes), `completion`, `diagnostic`, `doctor --all`, run in an isolated `HOME`.

### Phase 1: composition root, fitness test, shared kernel (done)
1. Move `main.rs`'s clap root and dispatch, `cli::completion` and `cli::print_error` to `bootstrap/cli.rs`.
   `main.rs` becomes thin.
2. Add `src/architecture_tests.rs` with an empty module list.
3. Move these into shared:
   - `Shell` and `ShellQuoting`. This removes `suggest` → `integrations` and `cli` → `integrations::quote`.
   - `OutputFormat` and `terminal_style`. This removes the `cli` ↔ `integrations` cycle.
   - the os helpers, the logger, and `testing/temp_dir.rs`. The temp-dir helper replaces `specs::testutil::TmpDir`
     and the 5 ad-hoc helpers.
4. Add the `DataDirectory` VO + `data_directory_from_env` + `SharedDependencies`. Each later phase switches its own
   callers. The old `settings::home()` family went in Phase 2, once its callers had switched.
5. Add the `Clock` port + `SystemClock` + `FixedClock`. `types::now_ms/now_secs` stay as thin wrappers until Phase 11.
6. Add `JsonDocumentStore` and `AtomicFile`. Move these tests to `atomic_file.rs`: `save_keeps_a_symlinked_file`,
   `dotfile_write_is_atomic`, and the `write_private` behaviour.

Done when `shared` is registered in the architecture test and the build has no cycles.

Done at `52dcf85`: `shared` registered; the last cycle (legacy `cli.rs` → `bootstrap` for `settings --help`) went in
Phase 2.

### Phase 2: settings (done)
1. Write the VOs test-first: `SettingKey`, `SettingValue`. Move `SettingKeyPattern` (with the wildcard test) and
   `SettingDefinition` + `SettingsCatalog` (with the catalog test).
2. Build `Settings` with the same getter surface (`bool`, `string`, `i64`, `f64`, `str_list`, `with_prefix`, `get`),
   so callers switch mechanically. Defaults come from the catalog.
   - Add one test per key that call sites defaulted before, asserting the catalog default equals the old literal:
     `autocomplete.height` 7, `autocomplete.width` 40, `inline.enabled` true, `autocomplete.theme` "dark",
     `inline.source` "history+specs".
3. Add `SettingsRepository` + InMemory, `JsonFileSettingsRepository` and `setting_value_json`.
4. Write the six use cases with InMemory tests. Move `cli.rs`'s settings render/parse tests into
   `cli/settings_command.rs`.
5. Add `settings_module` and switch the dispatch arm. Switch every `crate::settings::*` import. Delete `settings.rs`.

Done at `c7b796e`: `settings` registered in the architecture test, `settings.rs` deleted, gate green (512 tests).
See the Phase 2 notes in section 9.

### Phase 3: history (done: [research](hexagonal-refactor/phase-3-history.md))
1. Make `HistoryEntry` an entity without serde, and give the adapter a `HistoryRecord` struct.
2. Add `HistoryRepository` + InMemory, and `JsonlHistoryRepository` with `history_file_round_trip_and_compaction`.
3. Write `RecordCommandUseCase` test-first; blank commands are skipped.

Done at `f485ea7`: `history` registered in the architecture test, `inline::History` and `types::HistoryEntry`
deleted, gate green (524 passed). See the Phase 3 notes in section 9.

### Phase 4: shell_integration (the first full CLI-only slice; also starts terminal_session's session context; done: [research](hexagonal-refactor/phase-4-shell-integration.md))
1. Domain:
   - Extract the `Dotfile` VO from `install` / `uninstall` / `is_installed`, test-first, using the existing expectations.
   - Move these services, with their tests: `IntegrationCatalog`, `LaunchPolicy` (with
     `integrations_should_launch_table`), `InitScriptRenderer` and `ManualInstallInstructions`.
2. Add the three repositories + InMemory + file adapters (on `AtomicFile`), and the ports.
   - `SpecBundleInstaller` is bridged to the legacy `specs::ensure_extracted` until Phase 5.
3. Write the eight use cases with InMemory tests. The temp-HOME tests stay green as adapter-level tests.
4. Add the CLI handlers, plus `embedded_shell_scripts.rs`, which receives the 22 real-shell tests and the snapshot
   tests in its `tests.rs`.
5. Move `_ pre-cmd` and run-file pruning into `terminal_session`:
   - `SessionContext`, `SessionId`, `SessionContextRepository` and `FileSessionContextRepository`;
   - `ReportSessionContextUseCase` and `cli/pre_cmd_command.rs`.
6. Delete `integrations.rs`.

Done at `ab7052b`: `shell_integration` registered in the architecture test, `integrations.rs` and `types::SessionCtx`
deleted, gate green (761 passed). terminal_session stays unregistered until Phase 9. See the Phase 4 notes in
section 9.

### Phase 5: specs (done: [research](hexagonal-refactor/phase-5-specs.md))
1. **Model.** Move the VOs and entities from `spec.rs` under glossary names. Build `FigSpecMapper` with the 8 spec
   tests, `from_fig` and `fig_type`. `GeneratorScript::Object(Value)` becomes a typed `ExecRequest`.
2. **Bundle and location.**
   - Domain: `SpecIndex`, `SpecLocator`, `SemanticVersion` + `SpecVersionSelector`, `BundleRetentionPolicy`.
   - Split `spec_location_table` and `resolve_order` into a pure domain part and an fs adapter part.
   - Add `EmbeddedSpecBundle`, the `SpecBundle` port and `InstallSpecBundleUseCase`. Switch the shell_integration
     bridge to it.
3. **Runtime (the riskiest step: do it alone).**
   - Add the typed `SpecRuntime` port + `InMemorySpecRuntime`, and the `RuntimeEventSource` port.
   - Build `QuickJsSpecRuntime` from `js.rs`. Keep the `{"$fn": n}` wire format, the task registry and the timeouts
     unchanged. JSON ↔ typed conversion goes through `fig_spec_mapper`.
   - Split out `ProcessExecutor` and `native_ls`.
   - Until Phase 6 the legacy engine implements `RuntimeEventSource`.
   - Guard tests: all 18 js tests, `wait_aborts_and_defers_on_complete`, and
     `cli_version_detection_survives_an_aborted_parse`.
4. **Repository.**
   - Add `SpecRepository` + InMemory.
   - Build `QuickJsSpecRepository` from `parser::Loader`, using `fig_version_diff` (with the `diff_merge` test).
   - The legacy `JsHost` delegates to the repository + runtime until Phase 6.
   - Keep `Rc` identity by JSON. Guard tests: `first_token_spec_identity_is_stable` and
     `spec_identity_survives_dev_reload_and_cache_clear`.
5. Delete `spec.rs`, `specs.rs` and `js.rs`.

Done at `d7079b9`: `specs` registered in the architecture test, `spec.rs`, `specs.rs` and `js.rs` deleted, gate green
(851 passed). The phase ran in 17 steps (`5.1a` … `5.5`); the typed `ExecRequest` of step 1 and the split of step 2
became deviations. See the Phase 5 notes in section 9.

### Phase 6: completion (done: [research](hexagonal-refactor/phase-6-completion.md))
1. Move the tokenizer and its VOs from `shell_parser.rs`, with its 16 tests.
2. Build the pipeline VOs and services from `suggest.rs`, splitting its 23 tests across the services.
   - `list_dir` and `executables` sit behind the `DirectoryLister` and `ExecutableCatalog` ports; their 3 fs tests
     move to the adapters.
3. Move `ArgvParser` (pure) and `ParsePrefixCache`.
   - Build `CommandLineParser` over `SpecRepository` + `SpecRuntime`; `trait Host` is deleted.
   - Move `parser/tests.rs` to `command_line_parser/tests.rs`, using `InMemorySpecRepository` + `InMemorySpecRuntime`.
   - `disableForCommands` stays in `SpecRepository::find` (`SpecLookup::disables`, deviation 3).
4. Build the generator entities. `GeneratorRun` returns commands; the use case executes them through `SpecRuntime`.
   - Also build `GeneratorTriggerPolicy`, `SuggestionRecency` + `RecencyRepository` + `StateFileRecencyRepository`
     (with the recency tests) and `CompletionSession`.
5. Write the seven use cases and the result VOs (`types.rs` shrinks).
   - Add **new** use-case tests for the request pipeline and the generator lifecycle, using the InMemory spec
     repository/runtime and history/recency. These are untested today.
6. Build `bootstrap/engine_server.rs`: the inbox implements `RuntimeEventSource`, and the dispatch table calls the
   legacy `inline.rs` until Phase 7.
   - Create the `terminal_session::CompletionEngine` port + `ChannelCompletionEngine`. The legacy `pty.rs` switches
     from the raw `Sender<EngineMsg>` to the port.
7. Delete `shell_parser.rs`, `suggest.rs`, `parser.rs`, `parser/tests.rs` and `engine.rs`.

Done at `f3fc30a`: `completion` registered in the architecture test; `shell_parser.rs`, `parser.rs`, `parser/tests.rs`
and `suggest.rs` gone, each with its last item, and `engine.rs` moved to `bootstrap/engine_server.rs` by `git mv`; gate
green (943 passed). The phase ran in 28 steps (`6.0a` … `6.7`) in six runs. See the Phase 6 notes in section 9.

### Phase 7: ghost_text (done: [research](hexagonal-refactor/phase-7-ghost-text.md))
1. Domain: `GhostText`, with the accept rules and the tests `ghost_keys_and_words` and
   `ghost_with_control_chars_is_never_shown_or_accepted` moved from `pty.rs`. Also `LlmProfile` (from `SettingValue`),
   `LlmPrompt`, `GhostTextCache`, and the five services with their 11 pure tests.
2. Ports and adapters: the OpenAI and Anthropic clients (with `request_bodies` and `response_parsing`), the directory
   probe and the stdin chooser.
3. Use cases.
   - `RequestLlmGhostTextUseCase` gets its first end-to-end test with `ScriptedLlmClient`; `llm_complete` has none today.
   - Move the `cli.rs` inline tests to `cli/inline_command.rs`.
   - The inline commands get the settings repository from bootstrap as a trait object (section 2). The legacy
     `cli.rs` builds a `JsonFileSettingsRepository` itself; importing it from a registered module breaks fitness rule 3.
4. Replace the three legacy ghost touchpoints of `engine_server.rs` (`local_ghost_text`, `request_llm_ghost_text`,
   `clear_ghost_text_cache`) with ghost_text's use cases. Delete `inline.rs`.

Done at `353faaa`: `ghost_text` registered in the architecture test, `inline.rs` deleted (in 7.7), the inline command
in `ghost_text` (7.9), gate green (986 passed). The phase ran in 11 steps (`7.0` … `7.10`) in three runs; the single
HTTP client, the split cd check and `inline.rs`'s deletion in 7.7 became deviations. See the Phase 7 notes in
section 9.

### Phase 8: dropdown (done: [research](hexagonal-refactor/phase-8-dropdown.md))
1. Move the keys VOs, `Keymap` and `KeystrokeParser` (`keys.rs` tests). The `Decoder` goes to
   `terminal_session/infrastructure/adapters/stdin_key_decoder.rs`.
2. Move `Menu` with `MenuVisibility`, `MenuEffect` and `CommonPrefix`, and its 12 tests. `now` is passed in. The
   fixtures go to `completion/domain/test_fixtures.rs`.
3. Move the pure parts of `render.rs` and `icons.rs`: theme, colour, layout, text fitting and icon resolution, with
   `layout_decisions`, `truncation_and_wrap`, `colours_and_themes`, `resolves_like_q` and the tint test.
4. Build `ThemeRepository` + `FileThemeRepository` + `theme_json_mapper`. This unifies the two theme parsers; move
   `theme_file_parsing` and `theme_list_order`. Add the three theme use cases and `cli/theme_command.rs`.
   - The theme command gets the settings repository from bootstrap as a trait object (section 2), not the legacy
     `cli.rs`'s own `JsonFileSettingsRepository`.
   - `theme` with no name prints a non-string `autocomplete.theme` as pretty JSON (`{:#}`); `Settings::string` would
     print `dark` instead. The legacy `cli.rs` converts it with `settings::infrastructure::adapters::setting_value_json`,
     which dropdown may not import (rule 3), so dropdown's infrastructure does that `SettingValue` to JSON conversion
     itself, with the same output.
5. Move `Renderer`, the kitty graphics code and the icon assets to the terminal_session infrastructure adapters, with
   the round-trip tests unchanged. `keys.rs`, `menu.rs`, `icons.rs` and `render.rs` were deleted file by file with their
   last items (8.1b, 8.2b, 8.4b, 8.6).

Done at `da2f950`: `dropdown` registered in the architecture test, `keys.rs`, `menu.rs`, `icons.rs` and `render.rs`
gone, the theme command in `dropdown` (8.5c), the renderer, the kitty graphics, the icon assets and the stdin decoder in
`terminal_session`'s adapters, gate green (1091 passed). The phase ran in 19 steps (`8.0a` … `8.7`) in three runs. See
the Phase 8 notes in section 9.

### Phase 9: terminal_session (the biggest phase; one commit per step)
1. Pure domain from `tracker.rs`: `MarkerSplitter` (7 tests), `ShellMarker`, `OutputPiece`, `ShellEvent`, `EditBuffer`,
   `ShellSuggestionColor` and `ScreenCommand`. Build the `MarkerTracker` entity test-first: it takes the cursor
   column in and returns events + screen commands.
2. Infrastructure: `OutputFilter` (4 tests), `EditBufferExtractor`, `CellTagger` and `AlacrittyTerminalScreen`.
   - The 26 marker/extract tests run through the real screen adapter. The alacritty model is in-memory and fast, so a
     real object beats a fake.
   - Delete `tracker.rs` and its four never-read fields (`has_seen_prompt`, `shell_path`, `tty`, `user`).
3. Domain from `pty.rs`: `InsertionReconciler`, `InsertionLock`, `EchoHold`, `KeyRouter`, `RenderGuard` +
   `ScreenState`, `SessionDeadlines`, `WrappedShellCommand` and `TerminalBackground`, with their 7 pure tests. Then
   build the `TerminalSession` aggregate.
4. Ports + doubles: `TerminalScreen`, `TerminalWriter` (+ a capturing double that replaces the `sink`),
   `WrappedShell` and `GeneratorProcessReaper`.
5. The use cases in the event table. Move each of the 22 `Session` tests to the use case it exercises. Each test
   arranges a `TerminalSession` + `AlacrittyTerminalScreen` + `RecordingCompletionEngine` + `CapturingTerminalWriter`.
6. Driver and CLI:
   - `session_runner`, which keeps the panic pass-through, the credits (`pty_reader_waits_for_credits`) and the
     one-write-per-batch rule;
   - `platform_unix` / `platform_windows`, `terminal_probe` (`probe_replies`), `fallback_shell` and the adapters;
   - the commands `pty_command`, `debug_shell_command` and `key_tester_command`, with their tests from `cli.rs`.
   - The `#[cfg(test)]` panic injection stays at the driver's call site.
   - The settings repository comes from bootstrap as a trait object (section 2), and the lenient load (at start and
     on every reload) moves into terminal_session's own infrastructure: on a load error it logs the same warn-level
     `settings: <error>: <cause>…` text as `settings::infrastructure::load_or_defaults` (D5) and uses
     `Settings::default()`. Importing
     `load_or_defaults` or `JsonFileSettingsRepository` from a registered module breaks fitness rule 3.
7. Delete `pty.rs`. `types::Msg` and `types::Model` are gone.

### Phase 10: diagnostics
1. Domain:
   - `DoctorRun`, with `doctor_group_soft_fix_abort` split between `DoctorRun` and `RunDoctorUseCase`;
   - `CheckOutcome` and `CheckFix` (data, no closures);
   - `SystemFacts`, `ToolVersion` (with `doctor_version_parsing`), `DoctorCheckCatalog` and `UsernameScrubber`.
   - Write **new** tests for each check evaluated from facts. Only 4 doctor tests exist today.
2. Add the ports and adapters. The spinner and the SIGINT cursor restore live in `terminal_doctor_reporter`. Add the
   `CheckFixer` bridge in bootstrap. The settings repository is injected by bootstrap, and diagnostics' own
   infrastructure does the lenient load the way Phase 9 step 6 says; `settings::infrastructure::load_or_defaults`
   then has no caller and is deleted.
3. Write the three use cases. Add the CLI handlers (with `diagnostic_toml`). Delete `doctor.rs` and `cli.rs`.

### Phase 11: cleanup and documentation
1. Delete `types.rs`. Remove `#![allow(dead_code)]` and delete whatever the compiler then
   flags, including:
   - `logger::trace`;
   - `pub` items that are only used locally.
2. Register all 10 modules in the architecture test, with no exceptions left.
3. Update `docs/SPEC.md`: §4.1 (module table → the new tree), §4.3 (message and type names) and §10 (test locations).
   The README needs no change.

---

## 6. Source file → destination (completeness checklist)

| Today | Goes to |
|---|---|
| `main.rs` | `main.rs` (thin), `bootstrap/cli.rs` |
| `cli.rs` | settings, dropdown (theme), ghost_text (inline), diagnostics (diagnostic, debug logs), terminal_session (debug shell, key tester), bootstrap (completion, print_error), shared (OutputFormat, style) |
| `settings.rs` | settings/*, shared (DataDirectory, JsonDocumentStore, AtomicFile) |
| `logger.rs` | `shared/infrastructure/logger.rs` |
| `integrations.rs` | shell_integration/*, shared (quoting, os helpers, style, AtomicFile), terminal_session (pre-cmd, run files) |
| `doctor.rs` | diagnostics/* |
| `keys.rs` | dropdown domain (keys, keymap, keystroke parser), terminal_session adapter (stdin decoder) |
| `menu.rs` | dropdown `Menu` + `CommonPrefix`; fixtures → `completion/domain/test_fixtures.rs` |
| `render.rs` | dropdown domain (theme, colour, layout, text fitting), dropdown theme mapper, terminal_session `overlay_renderer` |
| `icons.rs` | dropdown (icon slot/resolver), terminal_session (`kitty_graphics`, `icon_assets`) |
| `pty.rs` | terminal_session/* |
| `tracker.rs` | terminal_session/* |
| `types.rs` | split across shared, specs, completion, history, terminal_session, bootstrap; deleted |
| `shell_parser.rs` | completion tokenizer + Token / CommandLine / AliasTable |
| `spec.rs` | specs domain model + `fig_spec_mapper` |
| `specs.rs` | specs domain (index, locator, retention), `embedded_spec_bundle`, shared temp_dir |
| `parser.rs` (+ `tests.rs`) | completion (argv parser, prefix cache, `CommandLineParser`), specs (locator, semver, version selector, `quickjs_spec_repository`, `fig_version_diff`) |
| `suggest.rs` | completion services and VOs, completion fs adapters, specs mapper (`template_json`, `fig_type`) |
| `engine.rs` | completion entities and use cases, `bootstrap/engine_server.rs`, recency adapter, specs mapper |
| `js.rs` | specs adapters (runtime, module wrapper, process executor, native ls), specs VOs |
| `inline.rs` | ghost_text/*, history/* |
| `shell/`, `assets/`, `tests/fixtures/`, `tests/snapshots/` | unchanged; only the `include_str!` / `include_bytes!` / `CARGO_MANIFEST_DIR` paths are updated |

---

## 7. Risks and how each is held

| Risk | Guard |
|---|---|
| Typed `SpecRuntime` replaces JSON results. This is the largest semantic change. | Its own step (5.3); 18 js tests + parser and engine tests green; `{"$fn"}` wire format unchanged; `all_bundle_modules` ignored test run |
| Abort-on-newer-request semantics when the inbox moves to bootstrap | Single channel kept; `wait_aborts_and_defers_on_complete`, `cli_version_detection_survives_an_aborted_parse` |
| `Rc`-address identity (`GeneratorKey`, arg key, spec cache by JSON) | Encapsulated in `GeneratorKey::for_arg`, not redesigned; identity tests listed in 5.4 and `generator_keys_differ_per_index` |
| Latency and the one-write-per-batch rule in the PTY loop | `ProcessShellOutputUseCase` returns one byte buffer; the driver writes once; round-trip render tests |
| Setting defaults drift when call-site literals go | Per-key equality tests (Phase 2.2) |
| Windows-only code (`platform_windows`, job objects, console) | Compiles and tests on the CI `windows-latest` job; moved verbatim, no logic edits |
| Real-shell tests need zsh/fish/pwsh | Unchanged skips when a shell is absent; CI Linux installs zsh and fish |
| Doctor rewrite from closures to facts | New per-check tests before the old code is deleted |

---

## 8. Verification

- **Per step:**
  - `cargo build --locked`;
  - `cargo test --locked`: the pass count never drops below the Phase 0 baseline, except for duplicates removed on
    purpose, which the commit message names;
  - `cargo clippy --all-targets --locked`: no new warnings;
  - the architecture test.
- **Per phase:** `cargo test --locked --release -- --ignored all_bundle_modules`.
- **Output equivalence:**
  - `TABSMITH_INIT_SNAPSHOT_TEST=1` `init` snapshots are unchanged (the existing snapshot test);
  - `tabsmith settings list --all` and `tabsmith inline show-customizations --format json` output is diffed against
    a capture taken in Phase 0.
- **Manual smoke** (SPEC §10 checklist) after Phases 6, 9 and 11 (after Phase 7 the user runs item 4 plus the LLM
  fake-server check of [`phase-7-ghost-text.md`](hexagonal-refactor/phase-7-ghost-text.md), "Output-equivalence
  guard"; after Phase 8 items 1-3, 5 and 6 plus a custom theme file, the `system` theme in a light terminal, icons in a
  kitty, a ghostty and a tmux pane (the overlay has no golden coverage), a slow generator (the spinner after about 200 ms:
  P8-S1) and the two `pty.rs` theme moments no test reaches (a settings reload with `tabsmith theme light` from a second
  terminal, and a late OSC 11 reply with `system`: P8-S2), see [`phase-8-dropdown.md`](hexagonal-refactor/phase-8-dropdown.md),
  "Manual smoke"), with `TABSMITH_HOME=$(mktemp -d)`, using
  `cargo run -- debug shell zsh` (and bash and fish):
  1. `git ch` shows the menu.
  2. Tab inserts and branches appear.
  3. Esc hides the menu.
  4. `echo hi` then `ec` shows ghost `ho hi`, and → accepts it.
  5. The prompt at the bottom row draws the menu above.
  6. Resizing leaves no artefacts.
  7. `vim` draws no overlay.
  8. `exit` code is propagated.
  9. `tabsmith doctor` passes.
- **End:** push the branch and let the CI matrix (ubuntu, macos, windows) run. Ask the user before pushing.

---

## 9. Implementation decisions

Taken while implementing, after a read-only pass over every legacy file found places where the plan is silent or
where following it literally would change behaviour. They bind every phase.

- **D1 Behaviour beats the plan's letter.** Where a "dedupe" or "unify" would change behaviour, both variants stay
  explicit and the deviation is recorded. Known cases: `which` vs `js::find_exe` (exec bit, PATHEXT order, relative
  names), the two `script_timeout`s (f64 in the engine, i64 in js), the strict CLI theme parser vs the lenient
  renderer one, doctor's terminal check vs `LaunchPolicy`, `disableForCommands` (checked on every spec load, not
  once per request), the two generatorContext builders (process-env fallback), doctor's install fix vs
  `ReinstallIntegrations`, and the three atomic writes (`AtomicFile` has three methods, one per old helper).
- **D2 Fitness test scope.** The layer rules govern production code: `#[cfg(test)]` items and sibling `tests.rs`
  files are exempt from rules 1-3, so tests keep their fixtures and may wire real adapters. Every module may use the
  shared kernel, including `shared::infrastructure` from its own infrastructure. A module's infrastructure may not
  reach `crate::bootstrap`.
- **D3 Lazy data directory.** `DataDirectory` carries a deferred "no home" error that its accessors return, so
  commands that work without `HOME` today (`init`, `_ should-launch`, `_ get-shell`, help, `version`, `completion`,
  `settings` with only `TABSMITH_SETTINGS_PATH`, the `_ pty` fallback) keep working, and the others fail at the same
  moment with the same message.
- **D4 clap identifiers are frozen** (variants, fields, attributes, doc comments, aliases, order). Clap types that
  span modules (`DebugCmd`) live in `bootstrap/cli.rs`.
- **D5** Code that moves into domain/application and logs today returns the text to its infrastructure caller, which
  logs it with the same text, level and condition.
- **D6** A test that mixes two modules is split; the half in the module the plan assigns keeps the name, the other is
  `<name>_<aspect>`, and together they assert at least what the original did.
- **D7** Items used only by tests get `#[cfg(test)]` instead of being deleted in Phase 11.
- **D8** Catch-all arms on domain enums become explicit arms with the same mapping.
- **D9** Tests that name their own module path (self re-exec with `--exact`) get that string updated when they move.

Phase 1 notes: `anyhow` is gone from shared infrastructure (typed errors whose `{:#}` chain prints the old text);
`shell_serde` was dropped because nothing (de)serialises a `Shell`; the one remaining module cycle is the legacy
`cli.rs` → `bootstrap::cli::Cli` back-edge for the `settings --help` fallback, removed in Phase 2.

Phase 2 notes: `SettingNumber` holds `SettingValue::Number` as unsigned, negative or float like serde_json's
`Number`, so a settings.json number keeps its integer or float form. `SettingKey` is not validated (D1: `settings "" v` stores the
empty key); the get and set use cases take the key as `&str` and build the `SettingKey` inside, as the canonical
shape above does, while `Settings::{get, with, without}` keep `&str` keys because the catalog constants and the legacy
callers pass them. `Settings` exposes `entries()` (key-ordered pairs) as its
primitives view in place of `to_primitives()`. `infrastructure/load_or_defaults.rs` gives the legacy pty.rs and
doctor.rs the defaults plus the old `settings: <error>` warning when settings cannot be loaded; the settings commands
stay strict. It is reachable only while those files are unregistered (Phase 9 step 6 and Phase 10 step 2 replace it).
`cli::matching_keys` is inlined into `DeleteSettingsUseCase::execute(pattern, announce)`, which hands the matching
keys to `announce` before it saves, so `Removing …` still prints even when the save then fails;
`settings_delete_wildcards` is split between that use case and `settings_command.rs` (D6). The render tests moved to
`settings_command.rs`, but `settings_set_parses_json_or_string` follows `parse_value` into `setting_value_json.rs`.
`SettingsModule` also holds the `DataDirectory` (private, behind `locate_settings_file()`) so the settings command
still fails on a missing home before any other step, as `settings::settings_path()?` did.
`run_settings_command` takes a `print_root_help` callback from bootstrap, which removed the back-edge above.
`settings::user_home` became `shared::infrastructure::data_directory_from_env::user_home_from_env` with
`SharedError::UserHomeNotFound` (its plain message, no `TABSMITH_HOME` hint), and the `home`/`settings_path`/
`state_path`/`Store` family was deleted in Phase 2, not Phase 11, because shims never cross phases.

Phase 3 notes: `HistoryEntry` is built only through `HistoryEntry::new(HistoryEntryPrimitives)`; the primitives
struct lives in `history_entry.rs`, keeps `Default` for test literals and is what `to_primitives()` returns, while
the entity derives only `Clone`, `Debug`, `PartialEq`. Its one transition is `finish(exit_code, end_time)` (the
tracker's old field writes). Equality stays structural and no `identity()` method was added: nothing reads the
glossary identity (session id, start time) yet. The JSONL wire format lives in the adapter's private serde
`HistoryRecord`. `HISTORY_IN_MEMORY` (5000) is in the domain next to `HistoryRepository`, because the production
memory-only fallback (`InMemoryHistoryRepository`, no data directory) keeps the same cap; `HISTORY_COMPACT_AT` stays
in the adapter. `recent()` returns `Rc<Vec<HistoryEntry>>`, the load-time snapshot plus this process's appends,
shared rather than copied because ghost text reads it per keystroke; it never re-reads the file. The blank-command
skip belongs to the `HistoryRepository` contract (InMemory and JSONL both skip), so `RecordCommandUseCase` does not
repeat it. `HistoryModule { history, record_command }` is built on the engine thread by `build_history_module`;
`history` is the shared read side (ghost text, history rows, LLM context) that only bootstrap may hand to
completion and ghost_text once they are modules (Phases 6-7). `history_error.rs` was not added: nothing in the
module is fallible (load and append log and carry on, as before). `SharedDependencies::from_env()` (shared
infrastructure) builds the dependencies for both `bootstrap::cli` and the legacy engine, so nothing outside
`bootstrap` depends on `bootstrap` and the module graph stays acyclic. The Phase 3 review added a fitness rule:
a registered module's domain or application may not use a legacy flat module (`LEGACY_MODULES` in
`architecture_tests.rs`, kept by hand; infrastructure still may, for the bridges). `specs` is not in that list
because the legacy `specs.rs` shared the planned module's name, so Phase 5 checked by hand that the specs domain
and application never reached the legacy `crate::specs::` items; once `specs.rs` was gone, the list's comment says
`specs` is the registered module instead. The rules still do not check the plan's `anyhow` rule (section 1,
"Errors").

Phase 4 notes (decisions P1-P18 of [`phase-4-shell-integration.md`](hexagonal-refactor/phase-4-shell-integration.md),
which cites the code behind each):

- **Files section 4 lists that the phase did not create**: `domain/value_objects/integration_status.rs`,
  `domain/value_objects/restart_reminder.rs`, `application/ports/user_confirmation.rs` and
  `infrastructure/adapters/stdin_user_confirmation.rs`.
  - One `ShellIntegrationError` covers checks and operations (P2). A check returns `Result<(),
    ShellIntegrationError>` (installed is `is_ok()`), Display is the user text and no variant has a `source()`.
    `IntegrationStatus` is the status-row DTO in `application/integration_status.rs`, not a domain VO. The
    `Environment` and `SpecBundle` variants carry their cause chain pre-rendered (P3) by
    `shared::infrastructure::error_chain::display_chain` (the Phase 4 review moved it to shared; settings'
    `load_or_defaults` uses it too), so `{}`, `{:#}` and anyhow's `{:#}` print the same text.
  - No `RestartReminder` VO (P7): `RestartReminderRepository::save_pending` (best effort) is all the module needs.
  - No `UserConfirmation` port (P8). The root refusal, the `[Y/n]` prompt (`confirm`/`answer` in
    `infrastructure/cli/user_confirmation.rs`), the paths resolution and the shell sets stay in the CLI handlers in
    the old order: the prompt comes before the paths are resolved, so a port would have no application caller.
    `ShellEnvironment` is smaller than planned (binary, parent shell, launch facts, pwsh `$PROFILE`); the handlers
    call the shared `which`/`is_root` directly.
- **Added beyond section 4**: the `InitScriptTemplates` port (the application cannot reach the embedded scripts any
  other way), the `IntegrationInstaller` application service (P6, shared by the use cases, never a use case itself),
  the DTOs `IntegrationFailure`, `IntegrationStatusReport`, `SetupProgress` and `UninstallProgress` (P9-P10: setup
  and uninstall report each event through a callback, so every line prints at its old place),
  `adapters/file_data_directory_purger.rs`, the test-only `infrastructure/testing/integration_fixtures.rs` (P15) and
  `bootstrap/bridges.rs` (`SpecBundleBridge` over `specs::ensure_extracted`). `build_shell_integration_module`
  takes the `SpecBundleInstaller` as a second parameter (P12); bootstrap builds the module in each command arm that
  needs it. `SessionContextRepository` has `last_modified` beside `save`/`find`/`remove`/`prune_older_than` (P17):
  the pty reloads a context only when its file's modification time changed. `embedded_shell_scripts/tests.rs` holds
  24 moved script tests (the plan said 22 real-shell tests plus the snapshot tests). `IntegrationStatusReport` keeps
  P10's `failures: Vec<IntegrationFailure>` although status builds only `Listing` failures; the status JSON's
  `Operation` arm says it is never reached (no listing-only type, so every run still words a failure alike).
- **Accessors in place of `to_primitives()`**: `Integration` (P5) and `SessionContext` (P17) keep read-only
  accessors instead of section 1's `to_primitives()`. `GetIntegrationStatusUseCase` maps `describe()`,
  `shell().name()` and `file_name()` into each `IntegrationStatus` row, which also carries the check result (not an
  entity property, so a `to_primitives()` would not replace the mapping); the file adapter maps `SessionContext`
  into its private serde `SessionContextRecord` (precedent: `Settings::entries()`).
- **`IntegrationPaths`** (P1) replaces `integrations::Ctx`, which had no home in the plan: home, data root, binary,
  `ZDOTDIR`, fish config dir, and pwsh `$PROFILE` when known. `integration_paths_from` (`os_shell_environment.rs`)
  resolves it through `ShellIntegrationModule::locate_integration_paths()` at the moments the old code built `Ctx`.
  The script and backup dirs are still joined onto `data_root()` as before, so `DataDirectory::shell_dir()` and
  `backups_dir()` stay unused: carrying them would take `IntegrationPaths::new` to 8 parameters (a clippy warning,
  no budget) and change the test fixture, and deleting them would delete two Phase 1 tests.
- **clap** (P4, D4): `When` and the clap `IntegrationTarget` became the mirrors `IntegrationPhaseArg` and
  `IntegrationTargetArg` over the domain `IntegrationPhase` and `IntegrationTarget`; `ShellArg`, `InitArgs`,
  `SetupArgs`, `UninstallArgs` and `IntegrationsCmd` keep their names. Setup's shell set (`Shell::ALL` filtered by
  `which`) stays apart from `IntegrationTarget::All`. Legacy `cli::debug_shell` keeps its signature and renders
  through `init_command::init_script` (P11). `_ get-shell` has no use case: no plan use case covers it.
- **Fitness rules**: the Phase 4 review added rule 7 (no `use` item in `src/main.rs`, whose private imports every
  module could reach as `crate::<name>`; `main` calls by full path), turned rule 6 into an allow-list (a `mod.rs`
  holds only attributes and child declarations, so no `type` alias or wrapper fn either) and made rules 1-2 read the
  crate an `extern crate .. as ..` line links. Its second round added rule 8 (`#[path]`, `include!`, `#[macro_export]`
  and `#[macro_use]`). The checker reads only the registered modules and `src/main.rs`, so a macro exported from
  bootstrap or a legacy file still passes, as do `#[cfg_attr(.., path = ..)]` and a space after `#[` or before
  `!`. Rule 3 flags only another module's use cases (a `*_use_case` file or a `*UseCase` type, on any path under it
  since Phase 5) and `infrastructure` paths, so it cannot see the `IntegrationInstaller` application service;
  legacy `doctor.rs` builds one (P14), so Phase 10 greps diagnostics for it by hand. The rules still do not check
  the `anyhow` rule: at the end of Phase 4 a grep finds it in shell_integration only in `infrastructure/cli` and test
  code.
- **terminal_session** is not registered (Phase 9 finishes it); each step checked it locally. `_ pre-cmd` runs
  through `infrastructure/cli/pre_cmd_command.rs` and `ReportSessionContextUseCase` (save, then prune the contexts
  unchanged for a day). `types::SessionCtx` is gone: `EngineMsg::Session { env, alias }` already has its Phase 6
  shape (`ts` was never read), the engine keeps `session_env`/`session_alias`, and the pty holds a `SessionId` and a
  `Box<dyn SessionContextRepository>` over the file adapter. `SessionId` accepts what `_ pre-cmd` accepted
  (non-empty, ASCII letters, digits, `-` and `_`), not only the hex ids the wrapper generates (D1).
- **Env reads at dispatch**: bootstrap evaluates `SharedDependencies::from_env()` (env vars only, no I/O) when it
  dispatches `init`, `integrations`, `setup`, `uninstall`, `_ should-launch` and `_ pre-cmd`, so setup and uninstall
  read the data directory env before their prompt and `_ pre-cmd` before its id check. Paths still resolve when a
  command needs them (D3), and every output is unchanged. `integrations reinstall` asks `which pwsh` once instead of
  once per half.
- **Checks**: `scripts/hexagonal-refactor/integrations-diff.sh` and `precmd-diff.sh` (P16) diff scrubbed transcripts
  of the integration commands and of `_ pre-cmd` against the baseline binary; both stayed empty through the phase.
  The local tag `phase4-start` marks the phase start for `testlock.py`: 15 tests are CHANGED against it, each one a
  restructuring in the research's "Test moves" table. The phase ran on macOS only: the pwsh script tests (pwsh is not
  installed here), every `cfg(windows)` path and the new ungated tests first run in CI.
- **History**: 16 Phase 4 commit subjects are over the 72-character limit and stay in history (no rewrite):
  f5b1b99, 9f5ff3a, c6cede1, 2cfa53b, d78fd05, d39adaa, a9aa205, b33b9e6, b78bf3b, bc3929d, 2875954, 8191e36,
  c6b6533, ea2dc6b, 10a30a5 and 140fad1. `29fce49` is over the clippy budget (4 warnings); the next commit fixes it.
- **Hand-offs** (details in the research's "Hand-off notes for later phases"):
  - Phase 5: switch `SpecBundleBridge` to `specs::InstallSpecBundleUseCase`. Its typed error prints only its Display
    under `{:#}`, so the bridge renders the whole chain with `display_chain` (never a third hand-written copy); check
    `error: could not extract the completion specs: creating <dir>: <io>` byte for byte. Done in Phase 5 (see its
    notes).
  - Phase 6: `EngineMsg::Session { env, alias }` is renamed with the enum (bootstrap's `EngineMessage`), and
    `ApplySessionContextUseCase` takes over the engine's `session_env`/`session_alias`,
    `session_update_refreshes_first_token_items` and the five Phase 4 engine tests on the session context.
  - Phase 9: `debug_shell_command` (terminal_session) may not call shell_integration: bootstrap passes it a render
    callback built over `init_command::init_script` (precedent `print_root_help`), and the shell is still read from
    `$SHELL` inside the handler. `DebugCmd` moves to `bootstrap/cli.rs` and imports `ShellArg` from `init_command`.
    The pty's `Box<dyn SessionContextRepository>` becomes the injected port, `session_id()` generation moves, and
    terminal_session is registered.
  - Phase 10: doctor's own `IntegrationInstaller` wiring, its two unconditional restart-reminder marks and its raw
    session check move into diagnostics through the `CheckFixer`/`SystemFactsProbe` bridges.
    `RestartReminderRepository` gains `find`/`delete` for doctor's reader and clearer (still through
    `JsonDocumentStore`, keeping the other keys). `SystemFacts` carries each check result as `Result<(),
    ShellIntegrationError>`, which is neither `Clone` nor `PartialEq` (`Io` holds a `std::io::Error`).
  - Phase 11: use or delete `DataDirectory::shell_dir()`/`backups_dir()` (with their two tests) and update the
    glossary's `DataDirectory` row. `LEGACY_MODULES` still lists `integrations` (harmless); `types::now_ms` is still
    used by the engine.

Phase 5 notes (decisions P1-P18 and deviations 1-26 of [`phase-5-specs.md`](hexagonal-refactor/phase-5-specs.md),
which cites the code behind each; the phase ran as 17 steps, `5.1a` … `5.5`, as planned in its open question 9):

- **Order of the deletions**: `spec.rs` went in 5.1b, `specs.rs` became `specs/mod.rs` by `git mv` in 5.2b and
  `js.rs` became `quickjs_spec_runtime.rs` by `git mv` in 5.3c, each as soon as its last importer had switched, not
  in step 5 (deviation 17); no shim was needed. `EngineMsg::Timer` was deleted in 5.3d (deviation 6, P15), not in
  Phase 11. The engine's JSON halves and `template_json` moved into the mapper in 5.1, touching Phase 6 files early
  (deviation 22), and `SuggestionKind` dropped its serde derives with every user switched in 5.1, with no `types.rs`
  alias (deviation 23). The runtime's and the mapper's tests live in the sibling files
  `quickjs_spec_runtime/tests.rs` and `fig_spec_mapper/tests.rs` (plan section 1), with unchanged test paths
  (`specs::infrastructure::adapters::quickjs_spec_runtime::tests::all_bundle_modules`, which CI runs by name).
- **Model**:
  - The spec model's fields are private behind accessors and plain `with_*` builders (deviation 24, open question 1):
    Option/Vec accessors return references, `default` is read through `Arg::default_value()`, and
    `Subcommand::merged_with_generated` owns the generateSpec merge.
  - `GeneratorScript::Object(SpecValue)`, not a typed `ExecRequest` (deviation 5, D1): a script object becomes an
    `ExecRequest` only when the generator runs, so a malformed one still fails then with `bad script input: …`.
    `SpecValue` is an exact std-only tree (P5) with its own `Unsigned`/`Negative`/`Float` numbers (open questions 4
    and 8: not `SettingNumber`, not the judge's canonical text, which re-parsed some floats differently).
    `spec_value_to_json` calls `.expect` on `Number::from_f64`, so a `SpecValue::Float` is built only from parsed
    JSON: a computed NaN or inf would panic on the engine thread.
  - `SpecFunctionRef` is a type alias (deviation 18). The types completion's domain must name (`TaskId`,
    `TaskOutcome`, `SpecValue`, `SpecFunctionCall`/`SpecFunctionKind`/`SpecFunctionResult`, `ScriptResolution`,
    `CustomGeneratorResult`, `TemplateSuggestions`) live in `specs/domain`, because rule 1 forbids a domain file from
    naming an `application` path; their glossary rows are VOs, not port types (deviation 7).
  - `suggestion_from_json` is private in the mapper (`47b14a1`), a departure from P8's table: nothing outside the
    mapper calls it, and `function_result` lives in the same file.
- **Location, versions and bundle**:
  - `SpecVersionSelector` is cumulative, every diff ≤ the target (deviation 19). `spec_location_table` and
    `resolve_order` were not split into a domain part and an fs part (deviation 20, P9): `find_global_spec` takes an
    injected `is_file`, and each test moved whole. `SpecLocator` keeps the `bin/console` rule but not "dotslash": the
    single-token dotslash rule stays in `ParseCaches::parse`.
  - There is no `LoadSpecIndexUseCase`, and `SpecBundle` has only `install` (deviation 21, open question 5).
    `InstallSpecBundleUseCase` only delegates to it: the `ensure_extracted` branching (fast path, extract, touch +
    cleanup, the `cfg(feature)` split) stays in `EmbeddedSpecBundle` (deviation 26, P10).
  - `SpecError` is `Aborted`, `Timeout`, `Invalid(text)` and `BundleUnavailable(text)`, with no `NotFound`
    (deviation 2, P6, open question 3). Its text is pre-rendered, it has no `source()`, and it derives `Clone` and
    `PartialEq`; `BundleUnavailable` is rendered by `display_chain`. parser.rs's `From<SpecError> for ParseError` and
    the engine's three former catch-alls have explicit arms (D8).
- **Runtime**:
  - The typed `SpecRuntime` port and the typed parser `Host::call` follow the judge (open question 2).
    `QuickJsSpecRuntime` implements the port; `start_exec`, `start_sleep`, `forget` and `apply_settings` exist only in
    the trait impl, so every caller imports the trait. The parser reads a decoded result with `let-else` on its
    kind's variant (deviation 15), which one test pins for all ten kinds (P18).
  - The legacy engine's generator state machine stays on the JSON API (`start_call`, `call_json`, `wait_json`;
    deviation 8): the typed task half has no production caller until 6.4, which adds `read`, `read_exec_stdout` and
    `resolve_script` (deviation 9). So the script and custom readings exist twice, in `Engine::script_input`/
    `Engine::custom_result` (JSON) and in `fig_spec_mapper::function_result` (typed), until 6.4 removes the engine's.
  - Both generatorContext builders stay (deviations 4 and 25, D1): the production one is
    `fig_spec_mapper::generator_context_json`, beside the other JSON readers and writers, and the test-only one stays
    in the runtime tests. Both `script_timeout`s stay (the engine's f64 floor, the runtime's i64 floor).
  - The serde record in `exec_input.rs` keeps the identifier `ExecInput` (deviation 10). It has two parse entry
    points whose error texts differ, `__ts_exec` (`from_str`) and `Engine::script_input` (`from_value`); neither is
    routed through the other. `kill_live_children` stays a free function of `process_executor` (deviation 14); pty.rs
    calls it by its full path.
  - `EngineInbox` in engine.rs implements `RuntimeEventSource` until Phase 6.6 (P11). Its sink only enqueues: the
    runtime calls it while it holds its task registry, so a sink must never re-enter the runtime.
    `cli_version_detection_survives_an_aborted_parse` is built over `ChannelRuntimeEventSource` (open question 7),
    and `wait_aborts_and_defers_on_complete` keeps the real inbox contract in the engine tests.
  - The InMemory doubles have a minimal surface and no Phase 5 consumer (deviation 16): `InMemorySpecRuntime`
    answers and records calls, its tasks never finish and its `apply_settings` does nothing;
    `InMemorySpecRepository` keys specs as the parser tests' Mock does (`name`, `dir|name`), records every find,
    honours `disableForCommands` and never caches.
- **Repository**:
  - `find(&SpecLocation, &SpecLookup) -> Result<Option<Rc<Subcommand>>, SpecError>` + `forget_loaded_specs()`, with
    no `NotFound` (deviation 1): `Ok(None)` means "try the next location", and an `Err` stops the parse at once (the
    `host.load(loc)?` loop in `ParseCaches::parse_prefix`).
  - `disableForCommands` stays inside `find` (deviation 3, D1, open question 10). It is stated once in the domain as
    `SpecLookup::disables`, which `QuickJsSpecRepository` calls first (after the name destructure, before dev mode
    and the cache, so a disabled name is never cached) and `InMemorySpecRepository` too. Phase 6.3 must not move it
    into `CompletionRequestPolicy`; section 4 and Phase 6 step 3 now say so.
  - `QuickJsSpecRepository::find` holds one `borrow_mut` of its private `Loader` for the whole find and calls
    `Loader::load`, moved verbatim, as P13's sketch has it, instead of inlining that body; `Loader::load`, `versioned`
    and `cli_version` keep their `js` parameter. The repository owns its own `EmbeddedSpecBundle`, built in `spawn`
    from the same `dependencies.data_directory` as `Engine::new`'s.
  - The `Parser` takes `Rc<dyn SpecRepository>` and `Rc<dyn SpecRuntime>` in `Parser::new` (deviation 11), and
    `Parser::load_spec` is `#[cfg(test)]` (D7) instead of being deleted in Phase 11. There are two 5 s `TIMEOUT`
    consts, the parser's and the repository's, never merged with `LOAD_TIMEOUT` or `SYNC_CALL` (deviation 13, P14).
  - Test homes (deviation 12, P16): `cli_version_detection_survives_an_aborted_parse` moved to
    `quickjs_spec_repository.rs`; `spec_identity_survives_dev_reload_and_cache_clear` stays in `parser/tests.rs`
    until 6.3 splits it (D6); `diff_merge` and `version_pick` are in `fig_version_diff.rs`. The engine tests build
    their `Parser` with the helper `parser_over(&Rc<QuickJsSpecRuntime>)`, `report_session_context` takes
    `&mut Rc<QuickJsSpecRuntime>`, and `Engine::with` takes the `Parser` after the settings.
- **TDD**: new seams went RED → GREEN → commit → REFACTOR → commit; 5.3d, 5.3e, 5.3f and 5.4a committed each
  test-only GREEN or REFACTOR on its own as `test(specs): …`. Tests that passed as soon as they were written (5.4a's
  local and unknown finds, `a5f6072`) were added after the base implementation as contract-pinning tests.
  `QuickJsSpecRepository` is a verbatim move, not test-first; its three characterization tests (`d5edb2c`) came
  after it, passed with no production change, and each one failed under a temporary mutation of `Loader::load`.
- **Fitness rules**:
  - Run 1's review widened rule 3 (`aa16a20`, `1cfe712`): another module's use case is flagged by its `*_use_case`
    file or its `*UseCase` type on any path under it, so a module root or a legacy shim that re-exports it under its
    own name is caught. `36f6576` documents the adapter back door, and `1a03f9e` reads every imported short name
    file-wide before any line is checked, so a `use` below the code that uses it no longer escapes (55 fitness
    tests).
  - The accepted rule 3 ceilings, read by hand: a use case re-exported under another name (a `type` alias, a wrapper
    fn), an `as` rename, a `super::` path, and an adapter re-exported by a legacy file or a legacy module root. No
    `pub(crate) use` shim is left at the end of the phase.
  - `specs` is registered (`d7079b9`). `spec` and `js` stay in `LEGACY_MODULES`; `specs` cannot be listed there, and
    rule 3 checks neither a legacy module reached from infrastructure nor section 2's edge set (specs → shared,
    settings). So these greps ran by hand at the end and found nothing outside `#[cfg(test)]` code: a legacy module
    under `src/specs` (every grouped `use crate::{` reviewed too), a module other than shared and settings, `anyhow`
    (only tests and two doc comments of `embedded_spec_bundle.rs` name it), the old `crate::spec::`, `crate::js::`
    and `crate::specs::{ensure_extracted, version, resolve, user_dir, testutil, SpecIndex}` paths, and `ItemType`
    (only suggest.rs's test input string).
- **Checks**: the local tag `phase5-start` (`1a466e2`) marks the phase start for `testlock.py`: 46 tests are CHANGED
  against it, each one a restructuring in the research's "Test moves" table.
  `should_report_a_build_without_embedded_specs_like_anyhow` compiles only under `--no-default-features`, which
  neither the gate nor CI builds: `cargo check --locked --no-default-features --all-targets` ran from 5.2b on, and
  at the end `cargo test --locked --no-default-features` ran the test too (1 passed). `integrations-diff.sh` and
  `precmd-diff.sh` stayed empty, and the release `all_bundle_modules` run passed at the end of every runtime,
  repository and closing step.
- **Phase 4's Phase 5 hand-off, closed**: `SpecBundleBridge` runs specs' `InstallSpecBundleUseCase`, which
  `build_specs_module` builds. The failure text is the whole chain: `EmbeddedSpecBundle` renders it once through
  `shared::infrastructure::error_chain::display_chain` into `BundleUnavailable`'s pre-rendered text, which has no
  `source()`, so `spec_bundle_failure(anyhow::Error::from(e))` prints it whole under `{:#}`; no hand-written renderer
  was added. `should_keep_the_whole_chain_of_a_failed_extraction` (unchanged) and the `should_render_…_like_anyhow`
  tests pin the text. The 5.2b sandbox run, repeated on the final commit with both binaries (`env -i`, a regular
  file at `$SB/data/specs`), prints `error: could not extract the completion specs: creating $SB/data/specs: File
  exists (os error 17)` from `setup --no-confirm` and `Failed to fix: creating $SB/data/specs: File exists (os error
  17)` from `doctor`, byte for byte as the baseline binary does.
- **History**: `f8e6055`'s subject reads `rtk make specs …` because the repo's RTK command hook rewrote a word
  inside the `-m` string; later commits were written with `git commit -F`. Four subjects are over 72 characters:
  `9ba2510`, `e1f1dd2` and `c2f0825` (73) and `843b6dc` (74). None was rewritten.
- **Hand-offs** (details in the research's "Hand-off notes for later phases"):
  - Phase 6:
    - `CommandLineParser` is the `Parser` renamed, and `trait Host` is deleted: `host.load` becomes
      `specs.find(loc, &SpecLookup)` (`None` means "try the next location"), and `host.call` becomes
      `runtime.call(typed, Some(&generator), TIMEOUT)`. The parser tests' Mock becomes `InMemorySpecRepository` +
      `InMemorySpecRuntime`; a test helper may wrap the JSON callbacks through `call_arguments`/`function_result`
      (D2), so the test bodies stay the same. Completion may not import `fig_spec_mapper` (rule 3) and reads results
      through the typed port only. `disableForCommands` stays in the repository.
    - The engine's generator state machine leaves the JSON API. `GeneratorRun` returns
      `StartCall(SpecFunctionCall, GeneratorContext)`, `StartExec(ExecRequest, GeneratorContext)`, `StartSleep(u64)`
      or `Settle`, and the use case calls `read`, `read_exec_stdout` and `resolve_script`. A script's timeout is the
      `let timeout = …` in `Engine::script_input` exactly: `script_timeout(settings)` (the f64 floor)
      `.max(generator.script_timeout().unwrap_or(0)).max(request.timeout().unwrap_or(0))`, applied with
      `ExecRequest::with_timeout` (new in 6.4) after `ScriptResolution::Ready` and before `start_exec`;
      `cache_key_source` is still the text taken before any timeout is set. The engine's `script_input`/
      `custom_result` readings, and its `exec_input::ExecInput` and `generator_context_json` imports, go with it.
    - `GenCache` becomes `GeneratorCache<SpecValue>` and still stores raw pre-step values, which a step of another
      kind may read (`Engine::fetch`, and the `self.cache.store` in `Engine::advance`). A malformed result must still
      map to `[]`, never to a task error.
    - `ApplyCompletionSettingsUseCase` runs the `EngineMsg::Settings` arm of `Engine::handle` in today's order, on
      **every** Settings message: `apply_settings(&s)` (the runtime's i64 floor); replace the session's settings;
      `forget_loaded_specs()`; reset the `ParsePrefixCache`, keeping `first`; reset the generator slots (slots, slot
      arg, previous search term); clear the first-token cache.
    - The f64 `script_timeout` moves to `GeneratorTriggerPolicy`. The single-token dotslash/`bin/console` rule moves
      with `ParseCaches::parse` and never reuses `spec_location`.
    - `EngineInbox` and its two tests (`wait_aborts_and_defers_on_complete`,
      `should_deliver_a_runtime_event_read_by_the_engine_loop_to_the_runtime`) move unchanged to
      `bootstrap/engine_server.rs` (`EngineMessage::Runtime(RuntimeEvent)`); everything is still built inside the
      engine thread closure.
    - The index build (`build_specs_module(&dependencies).install_spec_bundle.execute()`, then the adapter's
      `load_index`, which ensures the bundle a second time) moves to `engine_server` with the same log line and
      fallback, and the same `SharedDependencies` builds the repository's `EmbeddedSpecBundle`.
    - `ApplySessionContextUseCase` takes over `session_update_refreshes_first_token_items` and the five Phase 4
      engine tests on the session context, with their helpers `engine_with_wrapper_env`, `report_session_context`
      and `parser_over`.
    - Split `spec_identity_survives_dev_reload_and_cache_clear` (D6). `SpecIndex` exposes its completions in order,
      and `SpecLocation::cache_key()` builds the prefix-cache key.
  - Phases 8-9: `render::describe` goes to a dropdown domain service, with no new `terminal_session → specs` edge
    (P3, open question 6). `GeneratorProcessReaper` is bridged to `process_executor::kill_live_children`.
  - Phase 10: doctor still calls `build_specs_module` and `embedded_spec_bundle::version` directly. Diagnostics gets
    the bundle version and the "installed" fact through `SystemFacts` or a port, and its `InstallSpecBundle` fix goes
    through the `CheckFixer` bridge to `InstallSpecBundleUseCase`. Doctor's check path
    (`ctx.data_root()/specs/<version>/index.json` in `doctor::setup_checks`, where `ctx` is Phase 4's
    `IntegrationPaths`) and the fix's env-derived root stay as they are.
  - Phase 11: `EngineMsg::Timer` is already gone, and `Parser::load_spec` goes with the parser in Phase 6.

Phase 6 notes (decisions Q1-Q33 and deviations 1-38 of
[`phase-6-completion.md`](hexagonal-refactor/phase-6-completion.md), which cites the code behind each; the phase ran as
28 steps, `6.0a` … `6.7`, in six runs, as its "Step briefs" planned):

- **Order and deletions** (deviations 1, 2, 29): the tokenizer and the result rows (plan step 5's VOs, moved and made
  private) came first (run 1), then the whole parser (run 2), the suggestion pipeline and the pty port (run 3), the
  generator vocabulary, recency and the typed port (run 4), the session, the runner and the use cases (runs 5-6), and
  the server and the close (run 6). Twelve engine-loop characterization tests (6.0a) came before any engine edit. Each
  legacy file left with its last item, not in step 7 (Q21): `shell_parser.rs` by `git mv` to
  `command_line_tokenizer.rs` (6.1a); `parser.rs` deleted and `parser/tests.rs` `git mv`'d to
  `command_line_parser/tests.rs` (6.3e); `suggest.rs` deleted (6.4a; git's rename detection pairs it with
  `generator_cache.rs`); `engine.rs` `git mv`'d to `bootstrap/engine_server.rs` (6.6b). The one intra-phase shim, 6.3d's
  `Parser { parser, cache }` facade (a struct, not a re-export), went in 6.3e. `Parser::load_spec` went with its last
  caller in 6.3d, not in Phase 11.
- **Result rows and the request** (deviations 3-6): `AliasTable` is a type alias of the std map. `GeneratorKey` is a
  newtype with `for_arg` (+ `#[cfg(test)] from_raw`); the argument key (`generator_key::argument_key`) shares its
  hasher, so section 7 overrides section 4's old "policy ← arg key" line. `CompletionRequest` keeps `cwd: PathBuf` and
  `shell: Shell`; the session stores a `ServedRequest` (a new VO, without the cwd), never the DTO. The rows,
  `ParsedCommandLine` and `ServedRequest` are built from `…Parts` structs, never by a positional constructor (clippy's
  `too_many_arguments`). 6.2b deleted the accessors nothing read (D7); test-only readers carry `#[cfg(test)]`
  (`ParsedCommandLine::spec_name()`), and `CompletionResult::suggestions()` became production when the legacy
  `local_ghost_text` first read it.
- **Parser** (deviations 7-10, 33, 34): `CompletionError` (the old `ParseError`) has no Display; only its derived Debug
  is printed. `ArgvState` and its API live in `argv_parser.rs`. `ParsePrefixCache` is owned by `CompletionSession` and
  passed `&mut` to `CommandLineParser::parse`; `CommandLineParser::clear_caches(&self, cache)` forgets the loaded specs
  first, then clears the cache keeping the first-token spec. `disableForCommands` stays in `SpecRepository::find`
  (Phase 5 deviation 3, Q23). The parser test fixture keeps the name `Mock` and its `specs`/`fns`/`loads` fields, now
  over the InMemory doubles, so no parser test body changed for it; `no()` stays a private fn of
  `command_line_parser.rs`, and `ParsedCommandLine` keeps the accessor `completion()`. The parser's 5 s `TIMEOUT` and
  `dev_mode` are private to `command_line_parser.rs`, apart from the repository's copies; both `script_timeout`s and
  both generator-context builders stay (D1, Q7).
- **Pipeline** (deviations 22, 23, 36): `after_sep` is in `suggestion_query.rs`, `same_row` private in the ranker,
  `cmp_first` private in the assembler, `exe_name` and the PATH split in `PathExecutableCatalog` (so
  `first_token_items` takes the executables), `SYNC_CALL` in the trigger policy, `SLICE` in the runner and
  `FIRST_TOKEN_TTL` in the session; the escapes are not merged into `ShellQuoting`. `DirectoryLister` is synchronous
  and `list_dir` moved whole into `FsDirectoryLister`; `first_token_sources` stays whole in
  `path_executable_catalog.rs`'s tests, the catalog's output fed to the domain merge;
  `empty_search_term_is_no_prefix_match` lives in `suggestion_filter.rs` until Phase 7.3; suggest.rs's function names
  are kept (only types took glossary names). `domain/test_fixtures.rs` holds the pipeline helpers from 6.2d,
  `application/test_fixtures.rs` the use-case harness.
- **Generators** (deviations 11-13, 35, 37): `GeneratorRunner`, an application service `Rc`-shared by
  `CompleteCommandLineUseCase` and `ProgressGeneratorsUseCase`, executes the `GeneratorCommand`s (the plan says "the
  use case"). The commands keep the Phase 5 hand-off payloads (`StartCall(SpecFunctionCall, GeneratorContext)`,
  `StartExec(ExecRequest, GeneratorContext)`, `StartSleep(u64)`, `Settle(GeneratorOutcome)`); nothing is boxed,
  because clippy's `large_enum_variant` did not fire (O8). The runner keeps the cache protocol (`fetch`), the
  directory listing and the port calls. Its private `start(run, command, step)` has an explicit arm per command (D8);
  `script_input` sends a `StartExec` through `fetch` itself, so `start`'s `StartExec` arm (and `StartCall`/`StartSleep`
  in the Exec step) is never reached today, and the debounce sleep goes through `start(StartSleep)`. `SpecRuntime`
  gained `read` and `read_exec_stdout` only: no `resolve_script`/`ScriptValue` (Q9, amending Phase 5 deviation 9).
  `GeneratorCache<V>` is generic: production holds `SpecValue`, and the moved `cache_ttl_swr_max_age` keeps `Value`
  (test code, D2). `effective_debounce` (0 → 200 ms) and `run_budget` are extracted free functions of the trigger
  policy, called where `update_slots` computed them, with plain `+`/`*` (never `saturating_*`); `update_slots`' list of
  fired keys is `triggered_keys`, so it no longer shadows the policy's `triggered`. `CompletionSession` is a
  field-access aggregate on purpose: about 37 moved accessors with no new logic, while the generator orchestration
  (when to fire, the slots of a new argument, the superseded check, run order, the cache protocol) sits in the runner,
  which drives the session through fine-grained accessors; `generator_cache_mut()` is the one escape hatch. This kept
  the moves verbatim; it is not the end design (hand-off below).
- **Recency** (deviation 14, O5): `SuggestionRecency` sits in `CompletionSession`, behind `RecencyRepository { load,
  save, flush }`. `StateFileRecencyRepository` keeps the pending copy (set before the `saving` swap), the 1 s
  `recency` thread and the newest-wins merge with the file another session wrote, so section 4's "Recency + merge"
  stays in the adapter. The two recency tests moved with it, CHANGED only by `Recency` → `RecencyRecord`.
- **Notices, use cases and the module** (deviations 15, 24-26, 28, 32): D5 goes through one `CompletionNotice`
  callback (`&mut dyn FnMut`: `GenerateSpecFailed`, `GeneratorFailed`, `NoSuggestions`), called at the old moments;
  `EngineServer::log_notice` logs each with its old text and level, with explicit arms. The lifecycle and
  request-pipeline tests live at the use-case seam over the InMemory harness (the 12 lifecycle tests on
  `ProgressGeneratorsUseCase` are arranged through `CompleteCommandLineUseCase`); `GeneratorRun` has 9 pure
  transition tests, written after the lifecycle suite; there is no runner-level suite. D6 splits happen only across
  modules: `spec_identity_survives_dev_reload_and_cache_clear` (specs/completion) and
  `session_update_refreshes_first_token_items` (completion/bootstrap); `cache_ttl_swr_max_age` moved whole. The five
  Phase 4 session-context tests kept their bodies; their helpers keep the engine-era names `engine_with_wrapper_env`
  and `js` (O2). `CompletionModule` exposes the recency repository (the `HistoryModule` precedent); `spawn` loads it
  before the env snapshot and flushes it after the loop; there is no `LoadSpecIndexUseCase`. Unit-struct use cases are
  built as values, never with `::default()`. The harness builds one `GeneratorRunner` per `CompletionDoubles`, lazily
  in a `OnceCell`, and hands it to both generator use cases as the module does; it builds a parser per
  `complete_command_line()` call, because it never builds `ApplyCompletionSettingsUseCase`, the module's other parser
  user.
- **Engine server** (deviations 16-20, 30, 31): `send_reply`, the one `Msg::Completion` sender, adds the local ghost to
  every reply (Parse, Generators, error); the use cases return `ghost: None` and `error()`'s ghost flag is gone, so the
  event table's idle and `Complete` rows are amended (Q15). Until Phase 7.6 the legacy ghost/LLM code is one `impl
  EngineServer` block marked `// legacy ghost text: Phase 7.6 replaces this block`; there is no `legacy_ghost_text.rs`
  (Q14). `EngineMessage` and `EngineInbox` have their own files, `engine_message.rs` and `engine_inbox.rs` (one type
  per file; section 4 and the Phase 5 hand-off put both in `engine_server.rs`, O3); the server still owns the inbox
  instance. The pty receives a `CompletionEngineStarter` (`fn(Sender<Msg>, Settings) -> Rc<dyn CompletionEngine>`),
  called where `engine::spawn` was, so no legacy → bootstrap edge appears in production; the test double
  `RecordingCompletionEngine` forwards `CompletionEngineCall`s, named after the port methods, into a channel (Q12).
  `spawn` builds on the engine thread, in order: inbox, runtime, `SharedDependencies::from_env()`, repository, index,
  history module, completion module, recency, session (with the env snapshot), server (Q26).
  `CommandLineParser::new` now runs inside `build_completion_module`, after the index and history loads; it does no
  I/O, so the observable order is unchanged. `spawn`'s doc no longer claims the thread ends when the senders drop
  (M5). `types::now_ms` has no engine caller left; it stays until Phase 11.
- **Files beyond section 4** (deviation 21): domain `annotation_kind.rs`, `cache_lookup.rs`, `generator_step.rs`,
  `generator_outcome.rs`, `generator_command.rs`, `generator_slot_view.rs`, `served_request.rs` and
  `services/completion_result_builder.rs` (a domain service, Q24); application `completion_notice.rs`,
  `parse_context.rs`, `generator_runner.rs` and `test_fixtures.rs`; bootstrap `engine_message.rs` and
  `engine_inbox.rs`. Section 4 now lists the completion and bootstrap layouts as built.
- **Test doubles outside completion** (deviation 27): `FixedClock::advance` (it moves only the `Instant`, not
  `now_ms`), the scripted `InMemorySpecRuntime` (`completing`, `completing_execs`, `finishing_sleeps` (opt-in: plain
  sleeps keep running, a 6.4c correction of the design), `interrupting`, `applied_settings` and, from 6.5c,
  `exec_requests`) and `InMemorySpecRepository::forgets()`, each landed before its first consumer (the Phase 5
  deviation 16 precedent). The runtime double's ceiling (its `ponytail:` note): one scripted outcome per function, one
  for every exec and one for every sleep, each ready at the first wait. So the lifecycle suite pins outcomes and their
  order, not slice timing, which only the 6.0a engine suite covers.
- **Fitness rules**: `completion` is registered (`f3fc30a`) with no rule weakened and no completion code changed for
  it; `LEGACY_MODULES` is untouched and keeps `engine`, `parser`, `shell_parser` and `suggest`, whose files are gone
  (CON-T27). Rule 3's accepted ceilings (Phase 5 notes) gain one: a use case that names a sibling use case of its own
  module passes (rule 3 reads only another module's). The hand check for it, every
  `src/<module>/application/*_use_case.rs` above its `#[cfg(test)]` grepped for a `*UseCase`/`*_use_case` other than
  its own, found none in the 24 files. The Phase 4 `IntegrationInstaller` ceiling carries over to completion, the
  first registered module with application services other modules depend on (section 2): rule 3 cannot see
  another module's application service, so an import of `CommandLineParser` or `GeneratorRunner` from ghost_text,
  dropdown or terminal_session passes every rule (plan section 1 allows only domain types, repository and port
  traits and DTOs). `grep -rnE 'completion::application::(command_line_parser|generator_runner|test_fixtures)' src
  --exclude-dir=completion` is empty at the end of Phase 6 (hand-off below). These
  greps also ran by hand at the end and found nothing outside `#[cfg(test)]` code: a legacy module under
  `src/completion` or `src/terminal_session/application/ports` (the one hit is `empty_search_term_is_no_prefix_match`
  calling `crate::inline::spec_ghost`, test code that Phase 7.3 moves; no grouped `use crate::{` exists there), a
  module other than shared, settings, specs and history under `src/completion`, `anyhow` or `logger` in completion's
  domain and application, a `pub(crate) use` under `src/completion`, and `crate::engine`, `crate::parser`,
  `crate::suggest`, `crate::shell_parser` or `EngineMsg` anywhere (only rule 3's legacy-module test fixture string,
  which stays byte-identical). From 6.6a every step also ran the fitness test on a copy with `terminal_session`
  registered: silent.
- **Checks**: the local tag `phase6-start` (`c38596c`, the hand-over docs commit; its `src/` equals `ff2db5a`) marks the
  phase start for `testlock.py`: 65 tests are CHANGED against it, each a restructuring in the research's "Test moves"
  table. That table counts 66: the 66th, `aborts_propagate`, changed only `ParseError::Aborted` →
  `CompletionError::Aborted`, an enum path testlock's loose form strips, so testlock reports it PATH-ONLY. MISSING 0,
  DUPLICATE 0, 92 NEW. No golden command reaches the engine; the guards were the unit tests, the 6.0a suite (20 runs at
  every engine step), the two inbox tests (50 runs at 6.5c, 6.6b and the end), `debug_shell_survives_closed_terminal`
  alone, `integrations-diff.sh` and `precmd-diff.sh` (empty at 6.4b and at the end), `cargo check --locked
  --no-default-features --all-targets`, and the release `all_bundle_modules` run after 6.3e, 6.4c, 6.4d, 6.4f, 6.5c,
  6.6b and 6.7. `integrations_zsh_powerlevel10k_prompt_wrapped` is pty-timing flaky under the gate's load: a lone
  failure is rerun alone, never skipped.
- **Open questions, defaults applied**: the rows went private in run 1 (O1); the five session tests kept their bodies
  through engine-era helper names (O2); `EngineInbox` has its own file (O3); the pty switched to the port in run 3
  (O4); the newest-wins merge stays in the adapter (O5); the user is asked for smoke items 1-2 after 6.4d and for the
  whole section 8 smoke after 6.7 (O6); Phase 7 decides TC2's shape (O7); no `GeneratorCommand` payload is boxed
  pre-emptively (O8).
- **History**: two subjects are over 72 characters, `0d1ea43` (6.4a) and `87dc339` (6.4c), both 73; from run 5 on,
  every step measured the subject before committing. `1f18684` (6.5a) is typed `test(completion)` but also rewrites
  the production doc comment of `ApplyCompletionSettingsUseCase`, so `refactor(completion)` would have fit. Every
  message went through `git commit -F`, and the RTK hook rewrote no subject. None was rewritten.
- **The Phase 6 hand-offs of the Phase 4 and 5 notes, closed**:
  - `CommandLineParser` is the `Parser` renamed (6.3d-6.3e) and `trait Host` is gone: the parser finds specs through
    `SpecRepository::find` and calls spec functions through `SpecRuntime::call`. The parser tests' `Mock` runs over
    `InMemorySpecRepository` + `InMemorySpecRuntime`; completion imports no `fig_spec_mapper`; `disableForCommands`
    stays in the repository.
  - The generator state machine left the JSON API in 6.4d. `GeneratorRun` returns the four `GeneratorCommand`s (6.4g),
    and `GeneratorRunner`, not the use case, executes them (deviation 11). There is no `resolve_script` (Q9): a script
    value is read with `read(.., SpecFunctionKind::Script, ..)`. The timeout is exactly `script_timeout(settings)`
    `.max(generator).max(request)`, applied with `ExecRequest::with_timeout` (6.4c) after `ScriptResolution::Ready`
    and before the exec starts; `cache_key_source` is still taken before it. The engine's JSON readings and its
    `ExecInput`/`generator_context_json` imports went in 6.4d.
  - `GeneratorCache<SpecValue>` stores raw pre-step values, and a malformed value still maps to `[]` (6.4a, 6.4d).
  - `ApplyCompletionSettingsUseCase` runs the Settings arm's sequence, in order, on every Settings message (6.5a).
  - The f64 `script_timeout` is in `GeneratorTriggerPolicy` (6.4a); the single-token dotslash/`bin/console` rule moved
    with `ParseCaches::parse` into `CommandLineParser` and never reuses `spec_location`.
  - `EngineInbox` and its two tests moved unchanged into their own file, `bootstrap/engine_inbox.rs`, not
    `engine_server.rs` (6.6b, Q13); everything is still built inside the engine thread closure.
  - The index build moved into `spawn` (6.5c) with the same log line and fallback, and the same `SharedDependencies`
    builds the repository's `EmbeddedSpecBundle`.
  - `ApplySessionContextUseCase` took the five Phase 4 session-context tests with byte-identical bodies (6.5a).
    `session_update_refreshes_first_token_items` was D6-split: the completion half keeps the name, and
    `session_update_refreshes_first_token_items_keeps_the_engine_running` sits in `engine_server.rs`'s tests.
    `parser_over` stayed with the engine tests until 6.5c replaced it with `spec_repository_over`, when the engine
    tests moved to `build_completion_module` (Q19). Phase 4's hand-off closes with it: `EngineMsg::Session { env,
    alias }` is `EngineMessage::Session` in `bootstrap/engine_message.rs`, and the session env and alias live in
    `CompletionSession`.
  - `spec_identity_survives_dev_reload_and_cache_clear` was split in 6.3d (the repository half keeps the name in
    `quickjs_spec_repository.rs`'s tests, the `_first_token` half is a parser test). `SpecIndex::completions()` (in
    order) feeds the first-token suggestions, and `SpecLocation::cache_key()` builds the parser's prefix-cache key.
- **Hand-offs** (details in the research's "Hand-off notes for later phases"):
  - Phases 7, 8 and 9: rule 3 cannot see completion's application services (Fitness rules above), so every step
    of these phases reruns `grep -rnE 'completion::application::(command_line_parser|generator_runner|test_fixtures)'
    src --exclude-dir=completion` by hand; it must stay empty outside `#[cfg(test)]` code.
  - Phase 7: one `impl EngineServer` block of `src/bootstrap/engine_server.rs`, marked `// legacy ghost text: Phase 7.6
    replaces this block`, holds `local_ghost_text(&self, result)`, `request_llm_ghost_text(&mut self, req, buffer,
    cwd, shell)` (thread `llm`, warn `engine: llm worker: {e}`) and `clear_ghost_text_cache(&self)`; the free
    `llm_history` follows it, the field is `ghost_cache: Arc<Mutex<inline::GhostCache>>`, and
    `llm_history_honours_history_count` is in its tests, the sibling `engine_server/tests.rs` (548 lines inline were over
    section 1's ~500). `send_reply` is the only `Msg::Completion` sender and calls
    `local_ghost_text`; the CommandFinished/CommandEnded arm calls `record_command.execute` (Finished only), then
    `clear_ghost_text_cache`, then `ResetGeneratorsUseCase`; the `Llm` arm calls `request_llm_ghost_text`. The twelve
    6.0a tests run under `bootstrap::engine_server::tests` (14 tests with `llm_history_honours_history_count` and the D6
    half), the two inbox tests under `bootstrap::engine_inbox::tests`. The other `crate::inline` users are `pty.rs`,
    `doctor.rs`, `cli.rs` and the test `empty_search_term_is_no_prefix_match` in
    `completion/domain/services/suggestion_filter.rs`, which imports `opts`/`ctx` from
    `completion/domain/test_fixtures.rs`. Step 4 names the three touchpoints and the event table is amended. 7.0
    creates `inline-diff.sh` (Q30); Q29 recommends `GhostTextError`; TC2 keeps `request_bodies`/`response_parsing`
    whole; M1, M2 → 7.1, M6 → 7.4, M7 → 7.5, M8, M9 → 7.7. `ghost_text` is registered at the end of Phase 7; `inline`
    stays in `LEGACY_MODULES`.
  - Phase 8: `completion/domain/test_fixtures.rs` exists; `comp`/`item` join it, built through
    `CompletionResultParts`/`SuggestionParts` (`settings(json)` only with `#[cfg(test)]` on each item, CON-T28). The
    menu reads the rows through accessors and `take_suggestions()`, applies `with_execute()` on the clone, and
    `GeneratorKey::from_raw` is test-only. `render::describe` goes to a dropdown domain service (Phase 5 P3).
  - Phase 9: `terminal_session::CompletionEngine` + `RecordingCompletionEngine`/`CompletionEngineCall` are the port and
    double the session use cases take as `Rc<dyn CompletionEngine>`. `CompletionEngineStarter` (`pty.rs`) dissolves
    into the module wiring, but the engine must still start inside `Session::start`'s `catch_unwind` (ENG-E30), and
    `debug_shell_command` receives the starter from bootstrap. The reply path is still `Sender<types::Msg>`
    (`Completion(Box<CompletionResult>)`, `Ghost`). `EngineInbox::next_msg`'s doc still ends "`None` when all senders
    are gone", which cannot happen (the inbox keeps its own sender, ENG-E6): reword it when the inbox is next touched.
  - Later cleanup (Phase 11 or its own step): `CompletionSession` is a state bag on purpose (deviation 37): group the
    slots, the runs and the generator cache behind one domain entity with intention-revealing methods, move the
    runner's generator rules into it and retire `generator_cache_mut()`, under the 6.5c lifecycle suite and the 6.0a
    suite. `GeneratorRun::new` and `CompleteCommandLineUseCase::new` are at clippy's 7-parameter limit, and the clippy
    budget of 3 is all used.
  - Phase 11: `types.rs` keeps `Msg`, `now_ms` (no engine caller), `now_secs` and `Model`. `LEGACY_MODULES` keeps every
    deleted name.

Phase 7 notes (decisions R1-R24 and deviations 1-18 of [`phase-7-ghost-text.md`](hexagonal-refactor/phase-7-ghost-text.md),
which cites the code behind each; the phase ran as 11 steps, `7.0` … `7.10`, in three runs):

- **Guards first** (deviation 1): step 7.0 added `scripts/hexagonal-refactor/inline-diff.sh`, two engine-server tests
  of the `Llm` arm and one pty test of `Msg::Ghost`, before any code moved. `inline-diff.sh` prints a scrubbed
  transcript of what the golden capture never shows of `tabsmith inline` and doctor's Inline check (valid and invalid
  profiles, a selected profile, `set-customization` by name and through the numbered chooser on a pty, a non-bool
  `inline.enabled`, a corrupted `settings.json`, no home). Diff it on the baseline binary against `target/debug/tabsmith`;
  the diff is empty after every step, as are `integrations-diff.sh` and `precmd-diff.sh`. The local tag `phase7-start`
  (`eef4c88`, the hand-over docs commit; its `src/` equals `7f48161`) is the second `testlock.py` baseline.
- **The pty** (deviations 2 and 3): `ghost_key` stays in `pty.rs` because it takes dropdown key types, so
  `ghost_keys_and_words` was D6-split: the `next_word` half keeps the name in `ghost_text.rs`, and
  `ghost_keys_and_words_keys` stays in the pty. The pty already holds `Option<GhostText>`; `CompletionResult.ghost` and
  `Msg::Ghost.text` stay text. `GhostText::new(computed_for, text)` takes the buffer first, the reverse of the old
  tuple, so only the 7.0 pty test would catch a swapped call.
- **Profiles and prompts** (deviations 4 and 5): `LlmProfile` is built by `from_setting` in production, with private
  fields behind accessors; `LlmProfile::new(LlmProfileParts)` is `#[cfg(test)]`. The profile and source reasons stay
  `String` texts (M8), so doctor and `inline show-customizations` print them as before. `LlmPrompt` carries `SYSTEM`,
  so `build_request` reads it (the one CHANGED test `request_bodies`). The user text keeps the header
  `OS/Shell/CWD`, then a blank line, then the numbered history (P7-T16).
- **One HTTP client** (deviation 6): `HttpLlmClient` serves both providers, so section 4's `openai_llm_client.rs` and
  `anthropic_llm_client.rs` were not created; `request_bodies` and `response_parsing` stay whole. It is the only
  `ureq` and `logger` user in `ghost_text`, and its one warn (`inline: profile '{name}' request to {url} failed: {e}`)
  is byte-identical.
- **The cd check** (deviation 7): `cd_target` is a pure domain function (the two `cfg!(windows)` expressions stayed),
  and the probe-backed `validate` lives in `LlmGhostTextJob`; `DirectoryProbe` also yields the fallback home, read
  lazily and only for a `~` target the env does not resolve (P7-T25). `cd_validation` is CHANGED: it passes
  `&FsDirectoryProbe`.
- **Threads** (deviations 8 and 9): `RequestLlmGhostTextUseCase` returns `Immediate` or a `Deferred` job; the `llm`
  thread, its warn `engine: llm worker: {e}` and the `Msg::Ghost` sends stay in `bootstrap/engine_server.rs`. The
  ports are held as `Arc<dyn … + Send + Sync>` because the job crosses to that thread. One `Arc<Mutex<GhostTextCache>>`,
  built by `GhostTextModule`, is shared by the request and clear use cases; the race where a late `insert` follows a
  `clear` is kept as it was (D1).
- **Local ghost on every reply** (deviation 10): `SuggestLocalGhostTextUseCase` takes `&CompletionResult` and runs for
  every reply, error replies included, as Phase 6 amended the table. Its `_ =>` arm became explicit arms (D8).
- **The inline command** (deviations 11-13): two module structs, `GhostTextModule` (engine thread, from the history)
  and `InlineCommandModule` (CLI, from an `Rc<dyn SettingsRepository>` that `bootstrap/cli.rs` builds with the same
  lazy `JsonFileSettingsRepository` the legacy `cli::settings_repository()` made, so fitness rule 3 holds).
  `GhostTextError` wraps `SettingsError` and the chooser's io error transparently (the printed text is the wrapped
  error's, `source()` continues with its cause). `GhostTextSources` holds `Result<LlmProfile, String>` rows, a DTO
  exception like `SettingValue`. The use cases hold the chooser as `Rc<dyn SourceChooser>`, because `FixedSourceChooser`
  keeps a `RefCell`. `NoCustomizationSelected` has no unit test (the design lists none); `inline-diff.sh` pins its
  text. `GetInlineStatusUseCase` keeps the lenient bool read (M9, P7-T43).
- **Placements and extra files** (deviations 14 and 15): `selected_source` sits in the resolver, `provider_name` is
  `LlmProvider::name`, the `historyCount` wrapper is private in the request use case (`llm_history_honours_history_count`
  moved path-only), `empty_search_term_is_no_prefix_match` moved to `spec_ghost_text.rs`. Files beyond section 4:
  `llm_ghost_text_request.rs`, `llm_ghost_text_answer.rs`, `llm_ghost_text_job.rs`, `inline_command_module.rs` and
  `http_llm_client.rs`.
- **Fitness rules** (deviation 16): `ghost_text` is registered (`353faaa`) with no rule weakened and no ghost_text
  code changed for it; `LEGACY_MODULES` keeps `inline`, whose file is gone (P7-T50), and the checker's synthetic
  `ghost_text/...` sources are untouched. The rule-3 ceilings of the Phase 4-6 notes carry over; the hand greps ran
  and found nothing outside `#[cfg(test)]` code: a legacy module, `shell_integration`, `dropdown`, `terminal_session`,
  `diagnostics` or `bootstrap` under `src/ghost_text`, a grouped `use crate::{`, a `pub(crate) use` in `ghost_text`,
  `pty.rs`, `cli.rs` and `doctor.rs`, `anyhow` or `clap` outside `infrastructure/cli/inline_command.rs` (and the
  tests of `ghost_text_error.rs`), `ureq` or `logger` outside `http_llm_client.rs`, `serde_json` or `json!` outside
  `http_llm_client.rs`, `inline_command.rs` and test modules, `print!`/`println!` outside `inline_command.rs` and
  `stdin_source_chooser.rs`, `crate::inline`, `GhostCache` or `fn ghost_rest` anywhere, a `Default` build of the unit
  structs, an `Arc<dyn LlmClient|DirectoryProbe>` without `+ Send + Sync`, and a `pub(crate)` field in the domain
  outside `LlmProfileParts`. `grep 'cfg!(' src/ghost_text` shows exactly the two `cfg!(windows)` of
  `cd_target_validator.rs`. `grep -rnE 'completion::application::(command_line_parser|generator_runner|test_fixtures)'
  src --exclude-dir=completion` is empty.
- **Eleven steps, `inline.rs` early** (deviation 17): the plan's four steps ran as 7.1-7.4 (domain), 7.5 (ports and
  adapters), 7.6-7.9 (use cases, the engine switch, the inline command) and 7.10 (the close). `inline.rs` and
  `mod inline;` went in 7.7, with the last item that used them, not at the end; no shim was needed. `cli.rs` lost its
  inline section in 7.9 and its `//!` no longer names `inline`. The `main.rs` `mod` order is alphabetical by
  convention only (fitness rules 6 and 7 check declarations, not order).
- **Stale comments repointed** (deviation 18, RC8): the `inline.enabled`, `inline.source` and `inline.historyCount`
  doc comments of `settings_catalog.rs` now name `get_inline_status_use_case.rs`, `doctor.rs`,
  `suggest_local_ghost_text_use_case.rs` and `pty.rs`; `ghost_text_source_resolver.rs` and `doctor.rs`;
  `request_llm_ghost_text_use_case.rs llm_history`. `grep -n 'inline\.rs\|engine\.rs\|engine_server\.rs\|cli\.rs'` over
  that file is empty.
- **Rechecks that changed a guard**: RC1, golden's inline coverage is 13 captures (plus 3 `doctor --all` and
  `completion bash/zsh/fish`) and never a valid profile, so `inline-diff.sh` carries the rest; RC2, with stdout in a file
  the chooser's prompt never reaches the pty, so the script queues the answer on the tty right after `Popen`; RC3, the
  slice holds no `#[cfg(..)]` attribute, so the cfg check is the attribute grep per touched file plus the `cfg!(`
  grep; RC4, the `serde_json` grep allows `http_llm_client.rs` and `inline_command.rs` (`render_sources` builds rows
  with `json!`). Every code citation of the briefs was line-shifted by Phase 6 and was located by symbol.
- **Checks**: test totals at the end of the phase (`353faaa`, the registration; the closing commit changes docs only):
  986 passed, 0 failed, 1 ignored (`all_bundle_modules`; 943 at `phase7-start`), clippy 3 warnings (budget 3, all used),
  golden identical, testlock against `ef807c1` MISSING 0, DUPLICATE 0, CHANGED 154 (150 plus four: `cd_validation` was
  already CHANGED against `ef807c1`), against `phase7-start` MISSING 0, DUPLICATE 0, CHANGED 5 (`cd_validation`,
  `ghost_keys_and_words`, `ghost_with_control_chars_is_never_shown_or_accepted`, `profiles_and_source`,
  `request_bodies`, each a restructuring in the research's "Test moves" table) and 43 NEW names, each defined once: the
  42 of that table plus `should_not_resolve_a_tilde_target_when_neither_env_nor_fallback_know_the_home`, split in
  run 1's review from the P7-T25 test. The guards were the unit tests, the 7.0 trio, the engine-server suite (20 runs
  at each engine step), `debug_shell_survives_closed_terminal` alone, inline-diff, `integrations-diff.sh`, `precmd-diff.sh`,
  `cargo check --locked --no-default-features --all-targets` and the release `all_bundle_modules` run.
  `integrations_zsh_powerlevel10k_prompt_wrapped` is pty-timing flaky under the gate's load: a lone failure is rerun
  alone, never skipped.
- **Open questions, defaults applied**: one `HttpLlmClient` (O1), `LlmPrompt` carries `SYSTEM` (O2), two module structs
  (O3), the `GhostTextError` wrapper (O4), the closed-port engine test kept (O5; it is the only automated run of the
  `llm` thread), the pty's ghost tuples moved now (O6), `SuggestLocalGhostTextUseCase` takes `&CompletionResult` (O7),
  the user is asked for the smoke once, after 7.10 (O8), and `ScriptedLlmClient` does not record the env (O9).
- **Deviations the runs reported**: 7.8's `NoCustomizationSelected` is pinned by inline-diff, not a unit test. 7.9's
  `InlineCommandModule` is built by bootstrap from an injected repository (deviation 11), and the data-directory env
  is now read when bootstrap builds the module, before `stdout_tty()` in `run_inline_command` (it was read inside each
  arm, after the tty check); both reads are side-effect free, so nothing observable changes (P7-T37).
  `StdinSourceChooser`, and the Enable, Disable, Status, SetCustomization and ShowCustomizations arms of
  `run_inline_command`, have no unit test of their own: golden, inline-diff and the pty groups cover them, and only
  `render_sources` has one (`inline_show_customizations`, moved path-only).
- **History**: one Phase 7 subject is over 72 characters: `fe4b320` ("refactor(ghost_text): name the select use
  case's locals after what they hold"), 76, the review fix round of run 3. It stays in history (no rewrite), as
  Phase 6's two did. Every other subject is at most 72. Every message went through `git commit -F` with the Sonnet 5.5
  trailer, and none was rewritten.
- **Manual smoke pending** (the user's, after the phase; section 8): (1) plan section 8 item 4 on `debug shell zsh`
  (`echo hi`, then `ec` shows `ho hi`, → accepts, Alt-f accepts one word); (2) `tabsmith settings inline.source
  '"specs"'` makes `git ch` ghost a subcommand, `'"history"'` only history; (3) the LLM against a local fake server
  (`phase-7-ghost-text.md`, "Output-equivalence guard"): with an `inline.profiles.fake` pointing at `127.0.0.1:8765`, `zzz sta`
  shows the ghost `tus` after the debounce, and with the server stopped and `-vv`, `pty.log` shows the same
  `inline: profile 'fake' request to … failed: …` line for the baseline and the new binary.
- **Hand-offs** (details in the research's "Hand-off notes for later phases"):
  - Phase 8: `Key`/`KeyCode`/`Mods` move; `pty.rs`'s `ghost_key` and `ghost_keys_and_words_keys` follow by path only.
    Nothing in ghost_text names dropdown; the ghost reaches render as `&str` (`FrameInput.ghost`).
  - Phase 9: `key_router` takes `ghost_key` and `ghost_keys_and_words_keys` (the "accept ghost" `KeyRoute`; whole
    versus next word through `GhostText::next_word`). `TerminalSession` holds the local and LLM `Option<GhostText>`,
    with `Session::ghost_text`'s guards (P7-T33) and the local-before-LLM order. `ApplyLlmGhostTextUseCase` replaces
    the `Msg::Ghost` arm with P7-T32 intact (`req == s.req && req == s.llm_req`, keyed by the current `s.sent`
    through the `zip`, `redraw` without `release_held`); `should_show_the_llm_ghost_text_of_the_current_request_behind_the_local_one`
    pins it. `llm_enabled` = `settings.bool(INLINE_ENABLED) && matches!(source(..), Ok(GhostTextSource::Profile(_)))`;
    the LLM debounce stays in `HandleTimerTick`; the request goes through `CompletionEngine::request_llm_ghost_text`,
    which Phase 6's port still declares.
  - Phase 10: doctor's `inline_outcome` and `is_local` read `ghost_text_source_resolver::source` and
    `LlmProfile::{base_url(), api_key_env()}`; when they move into a domain service, the catch-all arms over
    `GhostTextSource` (and `Err(_)`/`Ok(_)`) get explicit arms (D8).
  - Phase 11: `LEGACY_MODULES` keeps `inline`; `types::Msg::Ghost` dies with `Msg` in Phase 9; `cli::settings_repository()`
    stays for `theme` until Phase 8. `#![allow(dead_code)]` stays until then.

Phase 8 notes (decisions R1-R30 and deviations 1-25 of [`phase-8-dropdown.md`](hexagonal-refactor/phase-8-dropdown.md),
which cites the code behind each; the phase ran as 19 steps, `8.0a` … `8.7`, in three runs; where the notes differ from
the briefs, the notes win):

- **Guards first** (deviation 1): step 8.0a added `scripts/hexagonal-refactor/theme-diff.sh` (groups A-I: the listing over
  every kind of file and folder, `--folder`, valid custom themes with the banner and the saved `settings.json`, every
  `invalid theme file` text of the strict parser, built-in names against theme files of the same name, every
  `settings.json` state, the save-after-banner failure, the display of non-string values, the bold escapes on a pty and the
  clap conflicts) and three tests of the lenient theme read; 8.0b added the byte-hash tests as data (FNV-1a):
  `should_render_every_icon_exactly_as_at_the_phase_8_start` (92 rows) and
  `should_draw_the_overlay_bytes_it_drew_at_the_phase_8_start` (10 scenarios), whose values no later step changed, and
  seven keymap and menu characterisation tests. The local tag `phase8-start` (`7aafcef`) is the second `testlock.py`
  baseline.
- **Steps, runs and deletions** (deviation 2, 23, R1, R2, R22): the plan's five steps ran as 19 (`8.0a`, `8.0b`,
  `8.1a-c`, `8.2a-c`, `8.3a-c`, `8.4a-b`, `8.5a-d`, `8.6`, `8.7`) in three runs: 8.0a-8.2c and 8.4a; 8.4b alone (one
  commit by design: `slot()` stops returning bytes); 8.3a-8.7. `keys.rs`, `menu.rs`, `icons.rs` and `render.rs` went with
  their last items (8.1b, 8.2b, 8.4b, 8.6), so step 5's last sentence was edited in place. The one temporary shim
  (R13), `impl Theme { from_settings, from_json }` in `render.rs`, lived from 8.3b to 8.5d and went with its last
  caller; no shim and no `pub(crate) use` remains.
- **Keys** (deviations 3-6): `KeyEvent` is `terminal_session/domain/value_objects/key_event.rs`; the `Decoder` is
  `StdinKeyDecoder`; `KeyCode` and `Modifiers` have their own files; `Mods`, `Action` and `Binding` took their glossary
  names. The key value objects have private fields behind `const fn` builders and accessors (8.1c, after the verbatim
  move). `keymap_defaults_and_overrides` was not D6-split: it stays whole in the decoder's tests (R5, O4).
  `KeystrokeParser` is two free functions, `parse_keystroke` and `normalize`.
- **Menu** (deviations 7-10): `Menu::on_completion` and `Menu::loading` take `now`; `pty.rs` passes `Instant::now()`
  at both sites and the renderer still reads `Instant::now()` itself in `menu_patches` (R7, O2). `common_prefix` is a
  service returning the new `CommonPrefixOutcome`; `Menu::common_prefix` only applies it. The catch-all arms of
  `Menu::on_completion` and `handle` became explicit (D8). `item`, `comp` and `settings` moved to
  `completion/domain/test_fixtures.rs` (`settings` with `#[cfg(test)]`); `max_rows` stays in `menu.rs`; `Menu::vis()` is
  `#[cfg(test)]`; the 12 menu tests moved to `menu/tests.rs`.
- **Theme** (deviations 11-16, 19): `Rgb` is a tuple alias. `Theme` has private fields behind eight accessors and a
  `ThemeParts` constructor argument. `ThemeRepository` has four methods, `{folder, find, list, find_palette}`, and two
  results: the strict `ThemeDefinition` (the author, for the banner) and the lenient `ThemePalette` (the colour roles),
  so `theme_json_mapper.rs` has two entry points, `parse_theme_definition` and `theme_palette_from_json` /
  `parse_theme_palette`: the strict and the lenient parser differ on about 16 inputs and were kept apart (D1); the
  plan's "one parser, not two" was not followed. `ThemeChoice` shares only the built-in name list with the CLI;
  `built_in("")` is none and `from_name("")` is dark. `resolve_theme` is a domain service that takes the
  `ThemeRepository` trait object (the skill gives ports to use cases): the custom branch is its only reader of the theme
  file and three call sites in `pty.rs` would otherwise carry the same glue, a precedent Phase 9 may turn into a use
  case. The theme CLI has no use case for `--folder`; `ThemeSelection` and `CurrentTheme` are the DTOs, and
  `ThemeSelection` holds domain value objects (`ThemeDefinition`, `ThemeChoice`), an exception like `SettingValue` and
  Phase 7's `GhostTextSources`. `DropdownError` carries the settings error (printed as itself), the data directory, the
  invalid theme file and theme-not-found texts. The `SettingValue` to JSON conversion of `theme` with no argument is
  copied into `theme_command.rs` (the settings adapter is off limits to dropdown) and pinned by a parity test against the
  adapter. `SelectThemeUseCase` announces the choice before it saves, as the legacy command printed the banner first.
  `describe` became `MenuDescriber` (`menu_describer.rs`, deviation 16), `layout` stays `menu_layout_calculator::layout`
  with `MenuLayout` and `MenuSide` value objects.
- **Icons and the renderer** (deviations 17, 18, 24): icons cross the layers as ids: `IconId`, `IconSlot` and the
  42-name `ICON_NAMES` are domain, `PNGS`, `TEMPLATE_RGBA`, `kitty_image` and `tinted` are in `icon_assets.rs`, and
  `placeholders` and `color` in `icon_id.rs`; the byte types are `KittyImage` and `KittyPixels`. `tinted` now runs at
  transmit time, once per colour per session (performance only, P8-T35). `FrameInput` stays in `overlay_renderer.rs` with
  no clock field (O7). The renderer's test module stays inline: `overlay_renderer.rs` is 1147 lines, 551 of them tests,
  a pure move over section 1's "about 500" that Phase 9.4 may slice when `FrameInput` moves.
- **Files beyond section 4 and repointed comments** (deviations 20, 21): `key_code.rs`, `modifiers.rs`, `key_event.rs`,
  `common_prefix_outcome.rs`, `menu_side.rs`, `menu_describer.rs`, `theme_palette.rs`, `theme_author.rs`,
  `theme_selection.rs`, `current_theme.rs`, `entities/menu/tests.rs` and `infrastructure/adapters/theme_file_error.rs`
  (`ThemeFileError` is `pub(crate)` because the strict entry returns it); `KittyImage` and `KittyPixels` share
  `kitty_graphics.rs` as the adapter's own parameter types (precedent: `ShellArg` and `InitArgs` in `init_command.rs`).
  The `autocomplete.*` doc comments of `settings_catalog.rs` name `dropdown/domain/entities/menu.rs`,
  `overlay_renderer.rs` and `theme_resolver.rs`.
- **A test-only edge** (deviation 25): the moved icon hash test and the renderer tests import `SuggestionKind` and
  `completion::domain::test_fixtures::item`, an edge `terminal_session -> specs` and `completion` that section 2 does not
  list; it sits under `#[cfg(test)]` (D2). Phase 9, when `terminal_session` registers, either allows test-only edges
  explicitly or adds a section 2 edge check.
- **Fitness rules** (deviation 22): `dropdown` is registered at `da2f950`, the end of the phase, by one line in
  `REGISTERED_MODULES`; the architecture test passes (55 tests) with no rule weakened and no checker test touched.
  `LEGACY_MODULES` keeps `icons keys menu render` whose files are gone (Phase 11 sweep), and `terminal_session` stays
  unregistered until Phase 9, so its new adapters are not rule-checked yet (`stdin_key_decoder` has unpaired `[` in byte
  literals to check then; P8-T49: the scanner reads string and char literals as code, but only an unpaired `{` or `}`
  in one shifts the use-group tracking, and no moved domain file has one). Rule 3 never flags a legacy module reached
  from infrastructure, a grouped `use crate::{` or anything under `terminal_session`, so the hand greps G1-G19 ran and
  found nothing outside `#[cfg(test)]` code: a legacy module, `shell_integration`, `history`, `ghost_text`,
  `terminal_session`, `diagnostics` or `bootstrap` under `src/dropdown`; a grouped `use crate::{`; a `pub(crate) use` in
  `dropdown`, `terminal_session`, `pty.rs` and `cli.rs`; `anyhow` or `clap` outside `theme_command.rs` (and the tests of
  `dropdown_error.rs`); `serde`, `serde_json` or `json!` outside `theme_json_mapper.rs`, `theme_command.rs` and test
  modules; `print!` or `println!` outside `theme_command.rs`; `logger` or `alacritty_terminal`; `unicode_width` outside
  `text_fitting.rs`; `crate::settings::infrastructure` outside the parity test of `theme_command.rs`; a `pub` field in
  the domain outside the `*Parts` structs (`ThemeParts`, `MenuLayoutParts`, `FigPaletteParts`, `Base16PaletteParts`);
  `std::fs`/`env`/`process`/`thread`/`net`, `SystemTime` or `Instant::now` in dropdown's domain and application;
  `Theme::from_settings`, `Theme::from_json` or `crate::{keys,menu,render,icons}` anywhere; `SHADES` and `ACCENTS`
  outside `theme_palette.rs`; a bare `keys.rs`, `menu.rs`, `render.rs` or `icons.rs` in a comment. `grep 'cfg(' src/dropdown`
  shows only `#[cfg(test)]` and the `#[cfg(unix)]` of the chmod test in `file_theme_repository.rs`; `grep 'cfg!('` is
  empty, and the diff of `src` against `phase8-start` adds and removes no `cfg(windows)`, `cfg(target_os)`,
  `cfg(not(..))`, `cfg(all(..))` or `cfg!(`. `grep -rnE
  'completion::application::(command_line_parser|generator_runner|test_fixtures)' src --exclude-dir=completion` is empty.
  The legacy names that survive in prose are `KeyBinding::Action` (a variant) and doc-comment words ("Action id", "Slot of
  a row", "Image ids", "Image covering", "Kitty graphics").
- **Checks**: test totals at the end of the phase (`da2f950`, the registration; the closing commits change docs only):
  1091 passed, 0 failed, 1 ignored (`all_bundle_modules`; 986 at `phase8-start`), clippy 3 warnings (budget 3, all used),
  build warnings 0, golden identical, testlock against `ef807c1` MISSING 0, DUPLICATE 0, CHANGED 168 (Phase 7's 154 plus
  14), NEW 809, PATH-ONLY 23, against `phase8-start` MISSING 0, DUPLICATE 0, PATH-ONLY 10, NEW 105 (99 `should_` names
  and the six split names `truncation_and_wrap_spinner`, `colours_and_themes_theme_json`,
  `colours_and_themes_theme_choice`, `tmux_uses_placeholders_passthrough`, `theme_list_order_built_ins` and
  `theme_file_parsing_banner`) and CHANGED 26, each a restructuring in the research's "Test moves" table:
  `keystroke_grammar`, `decode_table`, `decode_multi_key_chunk`, `esc_not_last_and_double_esc`, `paste_across_chunks`,
  `ghost_keys_and_words_keys` (8.1a/8.1c: `Mods {..}` and `Key {..}` literals to builders, field reads to accessors,
  `Decoder` to `StdinKeyDecoder`), twelve menu tests (8.2a: the new `now: Instant` argument: `typing_and_space`,
  `full_insertion_reopens_only_for_new_arg_or_retrigger`, `esc_hides_for_the_line`,
  `backspace_into_previous_token_and_large_change`, `hidden_until_keypress_refreshes_items`,
  `large_change_counts_skipped_requests`, `error_reset_and_loading`, `only_show_on_tab`,
  `sticky_selection_across_recompute`, `common_prefix`, `execute_and_record`, `round_trip_arg_hint_spinner_and_wide_ghost`),
  `colours_and_themes` and `truncation_and_wrap` (8.3a, D6 halves), `layout_decisions` (8.3c, `MenuLayout::new` and
  accessors), `resolves_like_q`, `tmux_uses_placeholders`, `transmit_chunks` (8.4b: accessors, `ICON_NAMES`, `IconId`,
  `KittyImage`, `KittyPixels`) and `theme_list_order`, `theme_file_parsing` (8.5c, D6 halves). The four diff scripts
  (`theme-diff.sh`, `integrations-diff.sh`, `precmd-diff.sh`, `inline-diff.sh`) are empty against the baseline binary,
  `debug_shell_survives_closed_terminal` (the only automated start-up of `pty::run`) passes,
  `cargo check --locked --no-default-features --all-targets` passes, and the release `all_bundle_modules` run prints
  `1 passed`. `integrations_zsh_powerlevel10k_prompt_wrapped` and
  `specs::infrastructure::adapters::process_executor::tests::exec_status_output_and_timeout` are timing-flaky under the
  gate's load: a lone failure is rerun alone with `--exact`, never skipped.
- **Open questions, defaults applied** (O1-O8): private key fields (O1), the renderer reads `Instant::now()` itself and
  Phase 9 adds `FrameInput.now` (O2), `tinted` stays in `icon_assets.rs` (O3), `keymap_defaults_and_overrides` kept whole
  (O4), `Rc<dyn ThemeRepository>` in the session (O5), `Image` and `Source` renamed `KittyImage` and `KittyPixels` (O6),
  `FrameInput` stays in `overlay_renderer.rs` (O7), the user is asked for the smoke once, after 8.7 (O8).
- **Deviations the runs reported**: 8.3a's commit B body is longer than 72 characters (only subjects are limited);
  8.3c left two long test lines of `menu_describer.rs` unformatted (`cargo fmt` is not used); 8.5a's commit E
  (`7a81d7f`) alone has clippy 4 warnings (its test dropped a `must_use` result) and the next commit `5783c4b`
  fixed it forward, so a bisect landing exactly on E sees a red gate; 8.5b added tests beyond the research list (a
  non-UTF-8 theme file, no home, two banner cases, a module test) and its chmod test is unix-only (23 of 24 tests off
  unix); `ThemeArgs` and `run_theme_command` have no unit test (precedent `run_inline_command`; `theme-diff.sh` guards
  them); 8.5d added `parse_theme_palette` as a second lenient entry and ran a throwaway differential test between the
  old and the new theme code (13 values, 14 names against 5 backgrounds, the default settings and no home: all equal;
  deleted, not committed); the two late `resolve_theme` sites of `pty.rs` (OSC 11 reply, settings reload) have no
  automated test.
- **History**: no Phase 8 subject is over 72 characters, and every commit has the Sonnet 5.5 trailer
  (`git log --format='%h %s' phase8-start..HEAD`: 49 commits up to the registration, counting the research hand-over and the run
  hand-offs, plus the docs commits of this close). Every message went through `git commit -F`; none was rewritten.
- **Manual smoke pending** (the user's, after the phase; section 8): with `TABSMITH_HOME=$(mktemp -d) cargo run -- debug
  shell zsh` (and bash, fish): (1) `git ch` shows the menu; (2) Tab inserts and branches appear, a shared prefix
  inserts; (3) Esc hides it; (5) a prompt on the bottom row draws the menu above; (6) a resize leaves no artefacts;
  icons in kitty or ghostty (direct placements) and in tmux with `allow-passthrough on` (placeholders);
  `tabsmith theme light`, a custom theme file under `$TABSMITH_HOME/themes/` and the `system` theme in a light terminal
  show the right colours; `tabsmith theme` with no argument prints the saved value; (P8-S1) a spec whose generator runs
  `sleep 1` (saved as `$TABSMITH_HOME/specs/user/<name>.js`, SPEC section 6.3) shows the spinner on the description bar
  after about 200 ms and drops it when the reply arrives; (P8-S2) with a session running, `tabsmith theme light` and back
  to `dark` from a second terminal with the same `TABSMITH_HOME` recolours the menu, and with `system` in a terminal that
  answers OSC 11 late the theme follows the late reply. These are not the plan's items 7 and 8 (`vim`, `exit`), which
  Phase 8 does not touch.
- **Hand-offs** (details in the research's "Hand-off notes for Phase 9"):
  - Phase 9: `pty.rs` now imports, from dropdown, `Key`, `KeyAction`, `KeyBinding`, `KeyCode`, `Keymap`, `Modifiers`,
    `MenuEffect`, `Theme`, `Menu`, `ThemeRepository`, `resolve_theme` and `FileThemeRepository`, and from
    `terminal_session`'s adapters `StdinKeyDecoder`, `FrameInput`, `OverlayRenderer` and `kitty_graphics::detect`; its
    fields are `keymap`, `decoder`, `renderer` and `themes: Rc<dyn ThemeRepository>` (built in `Session::start`;
    `InMemoryThemeRepository` in the test `Session`). `KeyRouter` takes `ghost_key`, `is_ctrl`, `note_forwarded`'s key
    rule and the `on_keys` arms with explicit D8 arms over `Option<KeyBinding>` in the guard order of P8-T10;
    `StdinKeyDecoder` becomes a port or a domain service and the session owns it (the held raw bytes replay through
    `on_keys` from three non-stdin use cases, so decode once per chunk); `HandleKeystrokesUseCase` interprets
    `MenuEffect`. `Menu` takes `now` from `Clock::now_instant()`; `OverlayRenderer::menu_patches` and
    `Session::on_completion` still read `Instant::now()`, so the screen adapter adds `now` to `FrameInput`. The
    `TerminalScreen` port also needs `set_theme` (unconditional), `drop_clear` and `with_images(detect())`; ONE
    `OverlayRenderer` per session. `DropdownModule::themes` is what bootstrap hands to `terminal_session`; the three
    `resolve_theme` sites (start-up, late OSC 11, settings reload) become use cases; `Rc` holds only while the session
    stays on one thread (O5). The renderer's one legacy import is `crate::types::Model`, retargeted when `Model` moves
    (9.7). `tracker.rs` and `types.rs` are untouched by Phase 8.
  - Phase 11: `LEGACY_MODULES` keeps `icons keys menu render`; `types::Model` lives to 9.7; the watch list (D7): `Menu::vis()`,
    `show_on_tab`'s unused `settings` parameter, `MenuEffect::Insert.full`, the two dead-code fields `version` and
    `theme` of the mapper's private `ThemeFile` (they validate), `Theme::description_border()` (read only by
    `theme_palette.rs`), `KeyCode::F(0)`, the `Hash` derives, `MenuLayout::side()` (`#[cfg(test)]`), `IconId::Png(usize)`
    unvalidated, and the setting-value-to-JSON copy in `theme_command.rs`; `cli::settings_repository()` is gone.
    `#![allow(dead_code)]` stays until then.
