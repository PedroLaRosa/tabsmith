# tabsmith → DDD + hexagonal architecture: refactor plan

Status: Phases 0, 1, 2, 3 and 4 are done on branch `refactor/hexagonal-architecture` (gate green at `ab7052b`).
Phase 5 is in progress. Working notes, tools, the per-file source map and the Phase 3, 4 and 5 research are in
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
| `specs::RuntimeEventSource` | `bootstrap::EngineServer` inbox (single channel, keeps today's abort-on-newer-request semantics) |
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
domain/entities/subcommand.rs             ← spec::Subcommand (+ reindex)
domain/value_objects/{spec_option, arg, generator, spec_suggestion, suggestion_kind, spec_location,
                      spec_function_ref, text_or_function, generator_trigger, generator_cache_policy,
                      generator_script, repeatability, filter_strategy, parser_directives, spec_load_directive,
                      spec_index, semantic_version, generator_context, exec_request, process_output}.rs
domain/services/{spec_locator ← parser::spec_location + specs::resolve_in name rules and lookup order
                 (exists predicate injected; bin/console, dotslash rules),
                 spec_version_selector ← version pick (newest ≤ target),
                 bundle_retention_policy ← specs cleanup staleness rules}.rs
domain/repositories/spec_repository.rs    trait {find(&SpecLocation) -> Result<Rc<Subcommand>, SpecError>} + InMemory
domain/spec_error.rs                      (Aborted, Timeout, NotFound, Invalid)
application/install_spec_bundle_use_case.rs ← ensure_extracted orchestration
application/ports/spec_runtime.rs         typed task API: start(SpecFunctionCall, Option<&GeneratorContext>) -> TaskId,
                                          start_exec(ExecRequest, &GeneratorContext), start_sleep, wait(tasks, deadline)
                                          -> Result<Vec<Option<TaskOutcome>>, Interrupted>, forget, call;
                                          + #[cfg(test)] InMemorySpecRuntime (← parser/tests.rs Mock)
application/ports/{spec_bundle.rs, runtime_event_source.rs}
infrastructure/specs_module.rs
infrastructure/adapters/
  fig_spec_mapper.rs        ← spec::{normalise, subcommand, option, arg, generator, …, suggestion_from_json,
                               load_spec_result, fn_id}, ItemType::from_fig, suggest::{fig_type, template_json},
                               engine::{json_items, script_exec_input, script_string} (JSON halves); 8 spec tests
  fig_version_diff.rs       ← parser::{apply_versions, merge_*, shallow_merge, make_array, names, truthy}
  quickjs_spec_runtime.rs   ← js::Js + task registry; JSON ↔ typed results via fig_spec_mapper; one
                               generatorContext JSON builder (dedupes engine::gen_context_json / GenCtx::context_json)
  quickjs_module_wrapper.rs ← wrap_module, PRELUDE
  quickjs_spec_repository.rs ← parser::Loader (spec cache, Rc identity by JSON, diff-versioned specs, CLI version probe)
  process_executor.rs       ← js::{exec, build_command, key_eq, wait_output, git_bash, clean, LIVE, kill_live_children, job}
  native_ls.rs              ← js::native_ls
  embedded_spec_bundle.rs   ← specs::{ARCHIVE, version, ensure_extracted, extract_to, touch, cleanup, unpack,
                               SpecIndex::load_from, resolve (fs part), dev_folder, user_dir}
```

### completion
```
domain/entities/{completion_session ← Engine state (Current, slots, runs, fuzzy, aliases, env, first-token cache),
                 generator_slot ← Slot, generator_run ← Run/Step/Outcome + step transitions (engine 713-948)
                 returning GeneratorCommand {StartCall, StartExec, StartSleep, Settle},
                 generator_cache ← GenCache/Lookup, parse_prefix_cache ← ParseCaches,
                 suggestion_recency ← Recency + merge (engine 79-91)}.rs
domain/value_objects/{token, command_line, alias_table, token_annotation, parsed_command_line,
                      suggestion_candidate, suggestion_query, filter_options, insertion_context, insertion,
                      suggestion, argument_hint, completion_phase, completion_result, generator_key}.rs
domain/services/
  command_line_tokenizer.rs ← shell_parser.rs (whole; AST stays private; 16 tests in tests.rs sibling)
  argv_parser.rs            ← parser 589-954 (State, ArgState, update…result)
  suggestion_assembler.rs   ← assemble, static_options, short_name, option_chain
  history_suggestions.rs    ← history_rows, mix_history
  fuzzy_matcher.rs          ← fuzzysort
  suggestion_filter.rs      ← filter, partial, prefix_score, auto_exec, same_row, after_sep, cmp_locale, cmp_first
  suggestion_ranker.rs      ← update_priorities, dedupe, rank
  insertion_composer.rs     ← insertion, name_match, is_var_ref, unquote, escape_* (uses shared ShellQuoting)
  suggestion_finalizer.rs   ← to_item, highlight, args_hint
  generator_trigger_policy.rs ← triggered, dir_changed, cache_key, is_path_template, script_timeout, debounce/budget
  path_template.rs          ← shell_expand, inserted_directory, help_items
  first_token_suggestions.rs ← first_token_items minus fs, is_root_spec, exe_name
  completion_request_policy.rs ← compute's pure parts: mid-word guard, arg key, fuzzy strategy, prefix,
                               autocomplete.disableForCommands rule (moved out of the loader)
domain/repositories/recency_repository.rs trait + InMemory
domain/completion_error.rs                ← ParseError (Aborted, NoSuggestions)
domain/test_fixtures.rs (#[cfg(test)])    ← menu::tests::{comp, item, settings} builders (shared by dropdown/terminal tests)
application/command_line_parser.rs        application service ← ParseCaches::parse, parse_prefix, substitute_alias,
                                          apply_load_spec, generate_spec over SpecRepository + SpecRuntime
                                          (parser/tests.rs → command_line_parser/tests.rs)
application/{complete_command_line, progress_generators, reset_generators, record_suggestion_insertion,
             apply_completion_settings, apply_session_context, toggle_fuzzy_search}_use_case.rs
application/completion_request.rs (DTO)
application/ports/{directory_lister ← list_dir, executable_catalog ← executables (PATH scan)}.rs
infrastructure/completion_module.rs
infrastructure/adapters/{fs_directory_lister, path_executable_catalog,
                         state_file_recency_repository ← load_recency/save_recency + throttled saver thread}.rs
```

### ghost_text
```
domain/entities/ghost_text_cache.rs       ← GhostCache
domain/value_objects/{ghost_text ← pty ghost_key/next_word/ghost_rest, ghost_text_source ← Source + BUILTIN_SOURCES,
                      llm_profile ← Profile + parse_profile (from SettingValue), llm_provider, llm_prompt}.rs
domain/services/{history_ghost_text, spec_ghost_text, llm_prompt_builder ← llm_history/is_private/usable/
                 history_prompt/SYSTEM/limits, llm_completion_cleaner ← clean, cd_target_validator ← validate/
                 split_words/expand (dir probe injected), ghost_text_source_resolver ← profiles/source}.rs
domain/ghost_text_error.rs
application/{suggest_local_ghost_text ← engine::ghost, request_llm_ghost_text ← engine::llm + llm_complete pipeline,
             clear_ghost_text_cache, set_inline_enabled, get_inline_status, select_ghost_text_source,
             list_ghost_text_sources}_use_case.rs
application/ghost_text_sources.rs (DTO)
application/ports/{llm_client.rs (+ ScriptedLlmClient), directory_probe.rs, source_chooser.rs ← choose_source}
infrastructure/ghost_text_module.rs
infrastructure/adapters/{openai_llm_client, anthropic_llm_client ← build_request/parse_response + ureq,
                         fs_directory_probe, stdin_source_chooser}.rs
infrastructure/cli/inline_command.rs      ← InlineCmd, inline, render_sources, provider_name, PROFILE_EXAMPLE
```

### dropdown
```
domain/entities/menu.rs                   ← menu::Menu (Instant passed in, never read inside)
domain/value_objects/{menu_visibility, menu_effect, key, key_action, key_binding, keymap, theme, rgb,
                      theme_choice, theme_definition, menu_layout, icon_slot, icon_id, graphics_support}.rs
domain/services/{common_prefix ← menu, keystroke_parser ← parse_keystroke/normalize,
                 menu_layout_calculator ← render::layout, color_parser ← parse_color/xterm256,
                 theme_resolver ← Theme::from_settings rules (dark/light/system luma/custom),
                 icon_resolver ← icons::{slot, spec_image, type_image, ascii_slot, wide_emoji, tint maths},
                 text_fitting ← render::{clean, cw, width, fit, wrap}}.rs
domain/repositories/theme_repository.rs   trait {find, list} + InMemory
domain/dropdown_error.rs
application/{select_theme, list_themes, show_theme}_use_case.rs
infrastructure/dropdown_module.rs
infrastructure/adapters/file_theme_repository.rs + theme_json_mapper.rs ← cli::{parse_theme, ThemeFile, ThemeAuthor},
                                           render::Theme::from_json (one parser, not two)
infrastructure/cli/theme_command.rs       ← ThemeArgs, theme, theme_banner, BUILTIN_THEMES (deduped with ThemeChoice)
```

### terminal_session
```
domain/entities/{terminal_session (aggregate), marker_splitter ← Splitter, marker_tracker ← Tracker state +
                 on_marker/end_prompt/new_cmd/pre_exec/flush_deferred (cursor column in, ShellEvents +
                 ScreenCommands out), insertion_lock, echo_hold, session_context ← SessionCtx}.rs
domain/value_objects/{session_id, shell_marker, output_piece, shell_event, edit_buffer, shell_suggestion_color,
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
                         cell_tagger ← tag/mark_line_start, overlay_renderer ← render::Renderer + frame/restore code,
                         kitty_graphics ← icons::{transmit, place, base64, tmux_graphics, detect},
                         icon_assets ← PNGS/TEMPLATE_RGBA, stdin_key_decoder ← keys::Decoder/decode*,
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
bootstrap/engine_server.rs ← engine::spawn/run/handle + types::EngineMsg (renamed EngineMessage); owns the inbox
                             (implements specs::RuntimeEventSource); dispatches events to use cases (table below)
bootstrap/channel_completion_engine.rs  implements terminal_session::CompletionEngine over Sender<EngineMessage>
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

Completion engine (`bootstrap/engine_server.rs`).
| Event (today's `EngineMsg`) | Use cases, in order |
|---|---|
| `Complete(request)` | `completion::CompleteCommandLine` → `ghost_text::SuggestLocalGhostText` → reply |
| idle with generator tasks pending | `completion::ProgressGenerators` → reply if changed |
| `CommandFinished(entry)` | `history::RecordCommand` → `ghost_text::ClearGhostTextCache` → `completion::ResetGenerators` |
| `CommandEnded` | `ghost_text::ClearGhostTextCache` → `completion::ResetGenerators` |
| `RecordInsert` | `completion::RecordSuggestionInsertion` |
| `Llm` | `ghost_text::RequestLlmGhostText` (on a worker thread, as today) |
| `Settings` / `Session` / `ToggleFuzzy` | `completion::ApplyCompletionSettings` / `ApplySessionContext` / `ToggleFuzzySearch` |
| `Shutdown` | driver stops; the recency adapter flushes |
| `ExecDone`, `Timer` | not events of the engine any more. They become `RuntimeEvent`s on the same channel, consumed by `QuickJsSpecRuntime` through `RuntimeEventSource`. `Timer` is never sent today and is deleted. |

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

### Phase 5: specs (in progress: [research](hexagonal-refactor/phase-5-specs.md))
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

### Phase 6: completion
1. Move the tokenizer and its VOs from `shell_parser.rs`, with its 16 tests.
2. Build the pipeline VOs and services from `suggest.rs`, splitting its 23 tests across the services.
   - `list_dir` and `executables` sit behind the `DirectoryLister` and `ExecutableCatalog` ports; their 3 fs tests
     move to the adapters.
3. Move `ArgvParser` (pure) and `ParsePrefixCache`.
   - Build `CommandLineParser` over `SpecRepository` + `SpecRuntime`; `trait Host` is deleted.
   - Move `parser/tests.rs` to `command_line_parser/tests.rs`, using `InMemorySpecRepository` + `InMemorySpecRuntime`.
   - Move the `disableForCommands` rule into `CompletionRequestPolicy`.
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

### Phase 7: ghost_text
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
4. Switch the engine-server dispatch to ghost_text. Delete `inline.rs`.

### Phase 8: dropdown
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
   the round-trip tests unchanged. Delete `keys.rs`, `menu.rs`, `render.rs` and `icons.rs`.

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
   - `EngineMsg::Timer`;
   - the test-only `Parser::load_spec` (move its test to the repository);
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
- **Manual smoke** (SPEC §10 checklist) after Phases 6, 9 and 11, with `TABSMITH_HOME=$(mktemp -d)`, using
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
because the legacy `specs.rs` shares the planned module's name, so Phase 5 must check by hand that the specs domain
and application never reach the legacy `crate::specs::` items, and should drop `specs.rs` from the list's story
once it is deleted. The rules still do not check the plan's `anyhow` rule (section 1, "Errors").

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
  `!`. Rule 3 flags only `*_use_case` and `infrastructure` paths, so it cannot see the `IntegrationInstaller`
  application service; legacy `doctor.rs` builds one (P14), so Phase 10 greps diagnostics for it by hand. The rules
  still do not check the `anyhow` rule: at the end of Phase 4 a grep finds it in shell_integration only in
  `infrastructure/cli` and test code.
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
    `error: could not extract the completion specs: creating <dir>: <io>` byte for byte.
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
