# tabsmith ubiquitous language

The names the hexagonal refactor ([`hexagonal-refactor-plan.md`](hexagonal-refactor-plan.md)) gives to every
domain entity and value object, grouped by module. Code, tests, commit messages and docs use exactly these names.
"Replaces" points at the code as of `4f26859`, before the refactor.

## shared (kernel)
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `Shell` | VO | A supported shell: Bash, Zsh, Fish, Pwsh. Built from a path or name. | `types::Shell` |
| `DataDirectory` | VO | tabsmith's home (`~/.tabsmith` or `TABSMITH_HOME`) and every path under it: settings, state, history, run, logs, themes, shell, backups, specs (`shell_dir` and `backups_dir` are `#[cfg(test)]`: only their tests call them). | `settings::{home, settings_path, state_path}` + ad-hoc joins |
| `ShellQuoting` | Domain service | Quote a string for a shell (POSIX single quotes, pwsh smart quotes). | `integrations::{quote, pwsh_sq}` |
| `Clock` | Port | Wall-clock ms/secs and monotonic `Instant`. | `types::{now_ms, now_secs}`, `Instant::now()` |

## settings
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `Settings` | VO | Immutable snapshot of the user's settings. Typed reads fall back to the catalog default. | `settings::Settings` |
| `SettingKey` | VO | Dotted key such as `autocomplete.height`, as `settings KEY [VALUE]` gives it to the get/set use cases. Not validated: `settings "" v` stores the empty key (D1). | string literals |
| `SettingValue` | VO | JSON-shaped value: Null, Bool, Number, Text, List, Object. | `serde_json::Value` in settings |
| `SettingNumber` | VO | The number inside `SettingValue::Number`: unsigned, negative or float, read as i64/u64/f64 exactly like serde_json's `Number`, so `5` never becomes `5.0`. | `serde_json::Number` in settings |
| `SettingKeyPattern` | VO | Wildcard pattern (`*`, `?`) for bulk delete. | `wildcard_match` |
| `SettingDefinition` | VO | Catalog entry: key, type, default, description. | `CatalogEntry` |
| `SettingsCatalog` | Domain service | The 28 definitions and the single source of defaults. | `CATALOG` + duplicated call-site defaults |
| `SettingsEditor` | Port | Opens the settings file in the user's editor, waits, and tells whether the editor exited successfully. | `cli::open_settings` (its editor launch) |

## history
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `HistoryEntry` | Entity | One executed command: command, shell, pid, session id, cwd, start/end time, exit code, hostname. Identity: (session id, start time). | `types::HistoryEntry` |

## shell_integration
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `Integration` | Entity | One installable integration: shell + dotfile path (+ whole-file phase for fish `conf.d`). | `integrations::Integration` |
| `IntegrationPhase` | VO | `Pre` (top of the dotfile) or `Post` (bottom). | `When` (clap mirror `IntegrationPhaseArg` in the CLI) |
| `IntegrationTarget` | VO | Dotfiles of one shell, or all: `Dotfiles(shell)` is that shell; `Dotfiles` without a shell and `All` are bash, zsh, fish, plus pwsh when it is on PATH. | `IntegrationTarget` (clap; now `IntegrationTargetArg` in the CLI) |
| `IntegrationStatus` | DTO | One row of `integrations status`: installed or not, description, shell name, dotfile name. (Why a check failed is a `ShellIntegrationError`.) | the `(bool, Integration)` rows of `status` |
| `Dotfile` | VO | Text of an rc file. Knows BOM, shebang, `# tabsmith ignore`, and how to insert, remove and check the pre/post blocks. pre block = `PRE_MARKER` + pre source line at the top (after BOM and shebang); post block = `POST_MARKER` + post source line at the bottom; source line `[ -f "<script>" ] && . "<script>"` (pwsh `if (Test-Path "<script>") { . "<script>" }`, a leading home written `${HOME}`); opt-out line `# tabsmith ignore`. | logic inside `install`, `uninstall`, `is_installed` |
| `IntegrationScript` | VO | A file tabsmith writes for an integration, `~/.tabsmith/shell/<rc>.<phase>.<ext>` or a whole fish `conf.d` file: path, exact contents, and the `should contain` check. | `script_path`, `script_contents`, `check_file` |
| `InitScript` | VO | Rendered `tabsmith init <shell> <phase>` output. | `init_output` |
| `LaunchFacts` / `LaunchDecision` | VO | Env + tty facts in; launch yes/no + reason out. | `launch_decision` args, `(bool, String)` |
| `RestartReminder` | Flag (repository) | `doctor.prompt-restart-terminal` in `state.json`: saved best effort by `RestartReminderRepository` after an install; doctor reads and clears it. | `mark_restart`, `doctor.prompt-restart-terminal` |
| `IntegrationPaths` | VO | Where one integration command reads and writes: the user's home, tabsmith's data root, the tabsmith binary the script files run, zsh's `ZDOTDIR`, fish's config dir, and pwsh's `$PROFILE` when already known (otherwise asked from `pwsh` only when a pwsh integration is listed). Resolved when a command first needs it: home, data root, binary. | `integrations::Ctx` |
| `ShellIntegrationError` | Domain error | Why an integration check or operation failed: `NotInstalled(reason)`, `Improper(reason)`, `FileDoesNotExist(path)`, `PermissionDenied(path)`, `Io(path, error)` shown as `<path>: <error>`, `Environment(message)` (home, tabsmith binary or pwsh `$PROFILE` unknown; message includes its causes), `SpecBundle(message)` (setup could not extract the completion specs). Display is the user-facing text; no source. | `integrations::IntegrationError` + the anyhow errors of `Ctx::from_env`, `exe_path`, `pwsh_profile` and setup's spec extraction |
| `IntegrationCatalog` | Domain service | The integrations of a shell (SPEC §5.5 table), from which bash dotfiles exist and pwsh's `$PROFILE`. | `integrations::integrations` |
| `LaunchPolicy` | Domain service | SPEC §5.2 first-match launch rules: `LaunchFacts` in, `LaunchDecision` (with its `✅`/`❌` reason line) out. | `launch_decision` |
| `InitScriptRenderer` | Domain service | The `init` output: header lines (parent shell, launch flag) plus the shell's template with `{{BIN}}` quoted for that shell. | `init_output` minus its I/O |
| `ManualInstallInstructions` | Domain service | The exact lines `setup` prints for a manual install (the lines install writes). | `manual_text` |
| `IntegrationInstaller` | Application service | Lists a shell's integrations, checks, installs, uninstalls and writes the script files of one `Integration` through the dotfile and backup repositories, and runs one of those operations over several shells, collecting `IntegrationFailure`s. Shared by the use cases; never a use case itself. | `Integration::{is_installed, install, uninstall, write_scripts, has_traces}`, `apply` |
| `IntegrationFailure` | DTO | One failure of a run over integrations: a shell whose integrations could not be listed, or one integration (its description) whose operation failed, with the reason. | the strings `apply` collected |
| `IntegrationStatusReport` | DTO | What `integrations status` shows: one `IntegrationStatus` per integration, then the shells whose integrations could not be listed. | `status`'s rows and errors |
| `SetupProgress` / `UninstallProgress` | DTO | What `setup` / `uninstall` report while they run (installed or not, automatic install failed, manual instructions, script files failed / integrations failed, data directory not removed). | the `println!`/`eprintln!`s of `setup` and `uninstall_all` |
| `ShellEnvironment` | Port | Facts the integration commands read from the running system: the tabsmith binary, the parent shell, the `LaunchFacts` (env + stdin tty/console), pwsh's `$PROFILE`. | `exe_path`, `parent_exe`, `should_launch`'s reads, `pwsh_profile` |
| `InitScriptTemplates` | Port | The shell code `init` prints for a shell and phase, still holding `{{BIN}}` (the embedded `shell/*` files; bash and zsh share one pre script). | `PRE_SH` … `POST_PS1` |
| `SpecBundleInstaller` | Port | Makes sure the completion specs are extracted (setup's last step); implemented by bootstrap. | `specs::ensure_extracted` called by `setup` |
| `DataDirectoryPurger` | Port | Deletes tabsmith's data directory for `uninstall --purge`; a missing one is fine. | `remove_dir_all(ctx.data)` |

## specs
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| Spec | Aggregate | The root `Subcommand` loaded from a `SpecLocation`. | `Rc<Subcommand>` + `loc_key` |
| `Subcommand` | Entity (in Spec) | Names, options, args, subcommands, load directive, parser directives. | `spec::Subcommand` |
| `SpecOption` | VO | A flag/option and its args. | `spec::Opt` |
| `Arg` | VO | A positional or option argument. | `spec::Arg` |
| `Generator` | VO | Template / script / postProcess / splitOn / custom / trigger / getQueryTerm / cache. | `spec::Generator` |
| `SpecSuggestion` | VO | A suggestion declared by a spec (static, additional or generated). | `spec::Suggestion` |
| `SuggestionKind` | VO | Subcommand, Option, Arg, File, Folder, Special, Shortcut, History, AutoExecute. Fig `type` strings map to it in the Fig spec mapper: `mixin` → Special, unknown → Arg. | `types::ItemType` (+ `from_fig`, `fig_type`) |
| `SpecLocation` | VO | Global `{name}` or Local `{dir, name}`. `cache_key()` (`global,,{name}` / `local,{dir},{name}`, trailing slash kept) names it in every cache; PathBuf equality never does. | `SpecLocation`, `loc_key` |
| `SpecFunctionRef` | VO (alias of u32) | Index of a function in the runtime's function table, written `{"$fn": n}`. | `FnId` |
| `TextOrFunction` | VO | A literal or a spec function. | `StrOrFn` |
| `GeneratorTrigger` | VO | When a generator reruns: string, function, change, threshold, match. | `Trigger` |
| `GeneratorCachePolicy` | VO | TTL, max age, cache by directory, cache key. | `CacheCfg` |
| `GeneratorScript` | VO | Argv, legacy shell string, function, or an `executeCommand` object kept as a `SpecValue`. It becomes an `ExecRequest` only when the generator runs, so a malformed object still fails then, with `bad script input: …`. | `Script` (`Object(Value)` → `Object(SpecValue)`) |
| `SpecValue` | VO | A JSON value written by spec code (a script object, a call argument, a task result), held exactly as read. Unsigned, negative and float numbers are kept apart; object keys are sorted. | `serde_json::Value` in the spec model and runtime |
| `Repeatability` | VO | No / Times(n) / Unlimited. | `Repeat` |
| `FilterStrategy` | VO | Default / Prefix / Fuzzy. | `Strategy` |
| `ParserDirectives` | VO | flagsArePosixNoncompliant, optionsMustPrecedeArguments, optionArgSeparators. | `Directives` |
| `SpecLoadDirective` | VO | Locations / Inline spec / Function. | `LoadSpec` |
| `SpecIndex` | VO | Root spec names in `index.json` order, then the user's own spec names, plus the diff-versioned set. | `SpecIndex` |
| `SemanticVersion` | VO | Parsed CLI version, with node-semver precedence; an unparsable version compares as 0.0.0. | `SemVer` + `semver*` |
| `SpecLocator` | Domain service | Where a command's spec lives (Global or Local, from the first token, isCommand or isScript; `bin/console` is `php/bin-console`). Which file a Global name loads from, over the roots the caller gives in order: root by root, `<name>.js`, else `<name>/index.js`; diff-versioned names only `<name>/index.js`; plain relative names only. | `parser::spec_location`, `specs::resolve_in` |
| `SpecVersionSelector` | Domain service | Which diffs of a diff-versioned spec apply for a detected CLI version: every diff whose version is ≤ it, oldest first, or all of them when the version is unknown. | key selection of `apply_versions` |
| `BundleRetentionPolicy` | Domain service | Which dirs under `~/.tabsmith/specs` are stale bundles to remove: tabsmith's own all-numeric version dirs other than the current one, unused for over 7 days, and `<version>.tmp-<pid>` dirs older than 10 minutes. | `specs::cleanup` rules |
| `SpecBundle` | Port | The embedded Fig spec bundle under the data directory: install it (extract if missing, mark in use, remove stale versions). | `specs::ensure_extracted` |
| `GeneratorContext` | VO | cwd, env, shell, search term, dangerous flag passed to generators. | `js::GenCtx` |
| `ExecRequest` | VO | `executeCommand` input: command, args, cwd, env changes, timeout. | `js::ExecInput` |
| `ProcessOutput` | VO | stdout, stderr, status. | `types::ExecOutput` |
| `SpecFunctionCall` / `SpecFunctionResult` | VO | A typed call into a spec function, and its typed result, for ten kinds: query term, trigger, alias, load spec, generate-spec cache key, generate spec, script, custom, post-process, filter template suggestions. | `JsArg` + `serde_json::Value` |
| `SpecFunctionKind` | VO | Which of the ten kinds a call is; it decides how a result is read. | |
| `TaskId` / `TaskOutcome` | VO | Handle of a started spec function, process or sleep (one id space), and its raw result: a `SpecValue` or the error text. | `js::{TaskId, TaskResult}` |
| `ScriptResolution` | VO | What a generator's script turned into: skip, invalid (serde text), or an `ExecRequest` + the cache-key source text. | inline in `engine::script_input` |
| `CustomGeneratorResult` | VO | A custom generator's suggestions + whether they are templated (first item has a `context` key). | inline in `engine::custom_result` |
| `TemplateSuggestions` | VO | What `filterTemplateSuggestions` receives: listed suggestions, or a templated custom result passed back unchanged. | `template_json` / raw custom value |
| `SpecLookup` | VO | What finding a spec needs from the current request: the settings, the `SpecIndex` and the `GeneratorContext` the CLI version probe and a versioned spec's `default(version)` run with. It also says whether `autocomplete.disableForCommands` disables a location's name. | `ParseCtx` + `gen_ctx` handed to `Loader::load` |
| `SpecError` | Domain error | Aborted (`aborted by a newer request`), Timeout (`timed out`), Invalid (text as is), BundleUnavailable (text as is). | `js::JsError` + the anyhow errors of `spec`/`specs` |
| `SpecRepository` | Repository | Finds the Spec at a `SpecLocation`. It answers none when the spec is missing, disabled by `autocomplete.disableForCommands`, or not a spec. `forget_loaded_specs` makes later finds look again; an unchanged spec keeps its `Rc`. | `parser::Loader`, `Host::load` |
| `SpecRuntime` | Port | Starts spec functions, processes and sleeps, waits for them (interrupted by a newer request), forgets them, and applies the script-timeout setting. | `js::Js` API, `parser::Host::call` |
| `RuntimeEventSource` | Port | The engine inbox as the runtime sees it: runtime events in arrival order, other messages kept for the engine, and whether a newer request is waiting. | `Js.rx` + `Js.deferred` |
| `RuntimeEvent` | Port type | A generator process finished: id + `ProcessOutput` or error text. | `EngineMsg::{ExecDone, Timer}` |
| `Interrupted` | Port type | A wait stopped because a newer request is waiting; no task was dropped. | `JsError::Aborted` from `wait` |

## completion
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `Token` | VO | One shell word: unquoted text, raw text, byte span. | `shell_parser::Token` |
| `CommandLine` | VO | The command under the cursor: tokens, text after alias expansion, original text. | `CommandInfo` |
| `AliasTable` | VO (alias of the std map) | Alias name → expansion, a type alias of `HashMap<String, String>` filled by `parse_aliases`. | `parse_aliases` map |
| `TokenAnnotation` / `AnnotationKind` | VO | How a token was consumed (subcommand, option, option arg, subcommand arg). | `Annotation`, `AnnKind` |
| `ParsedCommandLine` | VO | Result of walking argv against a spec (current arg, search term, passed options, …). | `ParseResult` |
| `CommandLineParser` | Application service | Parses a `CommandLine` against the specs found through `SpecRepository` and the spec functions run through `SpecRuntime`, using and filling the session's `ParsePrefixCache`; reports a failed generateSpec as a notice. | `parser::Parser`, `Host`, `JsHost`, `ParseCaches::parse*` |
| `ParseContext` | Application input | What one parse needs from the request: cwd, settings, env, shell, the `SpecIndex`. | `ParseCtx` |
| `ArgvState` | VO | Where the argv state machine stands after each token: the spec level, inherited persistent options, the argument being filled, what was passed. | `parser::State` + `ArgState` |
| `CompletionError` | Domain error | Aborted (a newer request is waiting) or no suggestions (with the reason). Printed only through Debug. | `parser::ParseError` |
| `CompletionNotice` | DTO | Something the completion code tells its caller to log, at the moment it happens: a failed generateSpec, a failed generator, no suggestions. | inline `logger` calls |
| `SuggestionCandidate` | VO | A row inside the filter/rank pipeline. | `suggest::Sugg` |
| `SuggestionQuery` | VO | How a row's query term is derived. | `Query` |
| `FilterOptions` | VO | fuzzy, auto-execute rules, dangerous, current token. | `FilterOpts` |
| `InsertionContext` | VO | Search term, raw token, shell, flags for composing an insertion. | `InsCtx` |
| `Insertion` | VO | Keystrokes that apply a suggestion: delete n, text, cursor back, execute. | `types::Insertion` |
| `Suggestion` | VO | A final, ranked row shown in the dropdown. | `types::Item` |
| `ArgumentHint` | VO | Name + description of the current arg. | `ArgInfo` |
| `CompletionPhase` | VO | Parse (first reply) or Generators (follow-up). | `Phase` |
| `CompletionResult` | VO | The reply for one request: suggestions, tokens, search term, loading, ghost, … | `types::Completion` |
| `CompletionRequest` | DTO | Request id, buffer, cursor, cwd (a path), shell (a `Shell`, not a name), history mode. | `types::Request` |
| `ServedRequest` | VO | The request the session is serving: its id, buffer, shell, clamped cursor, parsed command line, argument key, fuzzy flag, queries, triggered generators, first-token candidates and history prefix. | `engine::Current` |
| `CompletionRequestPolicy` | Domain service | Request-level rules: the mid-word guard, whether fuzzy search applies, the history prefix, the env lookup (case-insensitive on Windows). | parts of `Engine::compute` |
| `CompletionResultBuilder` | Domain service | Builds the reply for a served request: assembled, mixed with history, ranked by recency, filtered and turned into rows. | `Engine::reply` |
| `GeneratorKey` | VO | Identity of a generator slot (arg identity + index): a newtype built by `for_arg`; the argument key (`argument_key`) shares its hash. | `GenKey`, `gen_key` |
| `GeneratorSlot` | Entity | A generator's current result for the current arg (loading, generation). | `engine::Slot` |
| `GeneratorSlotView` | VO | A generator slot as the assembler sees it: key, rows, loading, and the query its rows match. | `suggest::GenView` |
| `GeneratorRun` / `GeneratorStep` | Entity / VO | One generator execution moving through Debounce → Script → Exec → PostProcess → Custom → Filter; each pure transition returns a `GeneratorCommand`. | `engine::{Run, Step}` |
| `GeneratorOutcome` | VO | Where a generator run ended this time: still pending, done with rows, or failed with a text. | `engine::Outcome` |
| `GeneratorCommand` | VO | What a generator run asks for next: start a spec function call or a process (each with its generator context), sleep, or settle with an outcome. | the start calls inside `engine::{begin, advance}` |
| `GeneratorRunner` | Application service | Starts, feeds and settles generator runs through the spec runtime, the directory lister and the clock, for both generator use cases. | `Engine::{update_slots, begin, fetch, advance, pump}` |
| `GeneratorTriggerPolicy` | Domain service | When a generator fires again (triggers, a changed directory, a debounce of 200 ms for 0), how long a run may take, the script-timeout floor, and the generator cache key. | `suggest::{triggered, dir_changed, cache_key}`, engine consts |
| `GeneratorCache` / `CacheLookup` | Entity / VO | Generator results with fresh / stale / in-flight / miss lifecycle. | `GenCache`, `Lookup` |
| `ParsePrefixCache` | Entity | Parse states memoised per prefix + generateSpec cache, each cleared wholesale before it would pass 1000 entries; a settings change clears it but keeps the first-token spec. | `ParseCaches` |
| `SuggestionRecency` | Entity | Command → suggestion name → last insertion time (ranking boost). | `engine::Recency` |
| `RecencyRepository` | Repository | Loads the suggestion recency, saves it (at most once a second, merged newest-wins with the file) and flushes a pending save at shutdown. | `load_recency`/`save_recency` + the engine's save thread |
| `CompletionSession` | Aggregate | Engine-thread state: settings, the served request, slots, runs, the generator cache, the parse-prefix cache, the recency, the first-token cache, fuzzy toggle, aliases, session env. | `Engine` fields |
| `DirectoryLister` | Port | Lists a directory for a path template: sorted, `../` last, control-char names dropped. | `suggest::list_dir` |
| `ExecutableCatalog` | Port | The executables on `PATH`, directory by directory in `PATH` order, each sorted. | `suggest::executables` |

## ghost_text
`LlmClient` is implemented by one `HttpLlmClient` for both providers.

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `GhostText` | VO | Gray continuation computed for one buffer: what is still to show after the typed buffer (none when empty or holding a control character), and the next word Alt-f accepts. Which key accepts it is key routing (`ghost_key`, in `KeyRouter`). | `(String, String)` tuples + `pty::{next_word, ghost_rest}` |
| `GhostTextSource` | VO | history, specs, history+specs, or an LLM profile. | `inline::Source` |
| `LlmProfile` / `LlmProvider` | VO | Provider (`openai`, `anthropic`), model, base URL, API key env var, max tokens, effort; read from an `inline.profiles.<name>` setting, or a reason text when invalid. | `Profile` + `parse_profile`, `Provider`, `cli::provider_name` |
| `LlmPrompt` | VO | System + user text sent to the provider: the OS, shell and cwd above the privacy-filtered, numbered history. | `SYSTEM` + `history_prompt` output |
| `GhostTextCache` | Entity | Recent LLM completions with priorities, cleared per command. | `GhostCache` |
| `GhostTextError` | Domain error | Settings (the settings error, printed as itself), customization not found, no customization selected, source chooser (the io error, printed as itself). | the `bail!`s and `?`s of `cli::inline` |
| `GhostTextSourceResolver` | Domain service | The profiles (sorted by name, each a profile or its reason), the selected source and its name. | `inline::{profiles, source}`, `cli::selected_source` |
| `CdTargetValidator` | Domain service | The directory a `cd` completion would change into, if any (quotes, `~` and `$VAR` expanded from the env). | the target half of `inline::validate`, `split_words`, `expand` |
| `LlmPromptBuilder` | Domain service | Which history lines the LLM may see (finished, not private, the last `historyCount`) and the prompt built from them (10 240-byte budget, none for a private buffer). | `llm_history`, `is_private`, `usable`, `history_prompt` |
| `LlmCompletionCleaner` | Domain service | The completion text kept from a provider reply (first usable line, unquoted, number and buffer stripped; none with a control character). | `inline::clean` |
| `LlmClient` | Port | Sends an `LlmPrompt` to a profile's provider; the raw completion text or none (transport failures are logged by the adapter). | the HTTP half of `llm_complete` |
| `DirectoryProbe` | Port | Is a path a directory; does it resolve to one; the home directory fallback. | `Path::is_dir`, `canonicalize`, `user_home_from_env` in `validate`/`expand` |
| `SourceChooser` | Port | Asks the user to pick one of the valid profiles, or none. | `cli::choose_source` |
| `GhostTextSources` | DTO | The selected source and every profile or its reason, for `inline show-customizations`. | `render_sources`' arguments |
| `LlmGhostTextRequest` | DTO | Buffer, cwd and shell of an LLM ghost-text request. | `EngineMsg::Llm` fields |
| `LlmGhostTextAnswer` | Application type | An LLM ghost text known at once (or none), or a job to run off the engine thread. | the two early sends and the spawn of `engine::llm` |
| `LlmGhostTextJob` | Application type | The worker half of an LLM request: prompt, provider call, clean, cd check, cache insert. | the `llm` worker closure + `llm_complete` |

## dropdown
Not in the table: the `*Parts` structs (`ThemeParts`, `MenuLayoutParts`, `FigPaletteParts`, `Base16PaletteParts`: named fields for a constructor that would otherwise take many parameters), `InMemoryThemeRepository` (the `ThemeRepository` double), `DropdownModule` (what bootstrap wires), `ThemeFileError` (the strict parser's error, an adapter error), and `icon_assets` / `kitty_graphics` (terminal_session adapters, below).

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `Menu` | Aggregate | Dropdown state: visibility, suggestions, selection, scroll, history mode, pop-out. | `menu::Menu` |
| `MenuVisibility` | VO | Visible, HiddenUntilKeypress, HiddenUntilShown, HiddenByInsertion. | `Vis` |
| `MenuEffect` | VO | Insert, Write, Bell, Rerequest, ToggleFuzzy. | `Effect` |
| `Key` / `KeyCode` / `Modifiers` | VO | A decoded keystroke. | `keys::{Key, KeyCode, Mods}` |
| `KeyAction` / `KeyBinding` / `Keymap` | VO | What a keystroke does; default + user bindings. | `Action`, `Binding`, `Keymap` |
| `KeystrokeParser` | Domain service | Parses `ctrl+k`, `shift+tab`, `+`, `f12` into a `Key`; `normalize` makes ctrl and alt letters case-insensitive. | `parse_keystroke`, `normalize` |
| `CommonPrefix` / `CommonPrefixOutcome` | Domain service / VO | The longest prefix the visible suggestions share and what inserting it does (fail, insert the selected row, insert a prefix). | `Menu::common_prefix` decision, `matching_type` |
| `Theme` | VO | The eight colours the overlay draws with (private fields, accessors). | `render::Theme` |
| `Rgb` | VO (alias of a tuple) | `(u8, u8, u8)`. | `render::Rgb` |
| `ThemeChoice` | VO | dark, light, system or a custom name; the three built-in names. | `BUILTIN_THEMES`, the `from_settings` match |
| `ThemeDefinition` / `ThemeAuthor` | VO | The strictly parsed custom theme file: its author, for the `theme NAME` banner. | `cli::{ThemeFile, ThemeAuthor}` |
| `ThemePalette` | VO | The colour roles a custom theme file gives (Fig `version "1.0"` or base16), each possibly missing; `theme()` fills the gaps from dark or the dark filler. | `Theme::from_json` rules |
| `ThemeRepository` (`FileThemeRepository`) | Repository | The themes folder: its path, the strict read of a theme file, the lenient read for drawing, the listing. Every read failure of the lenient path is "no theme". | `data_directory_from_env().themes_dir()` + `read_to_string` in `render` and `cli` |
| `ThemeResolver` | Domain service | Which `Theme` `autocomplete.theme` and the terminal background give (the repository is asked only for a custom name). | `Theme::from_settings` |
| `ThemeSelection` / `CurrentTheme` | DTO | What `theme NAME` announces (a custom definition or a built-in) / what `theme` prints (a name or a non-text value). | the `theme` command's locals |
| `ColorParser` / `TextFitting` | Domain service | `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()` and the xterm-256 downgrade / terminal-width `clean`, `fit`, `wrap`. | `parse_color`, `xterm256` / `clean`, `cw`, `width`, `fit`, `wrap` |
| `MenuLayout` / `MenuSide` | VO | Geometry: above or below, rows, x0, width. | `render::{Layout, Side}` |
| `MenuLayoutCalculator` / `MenuDescriber` | Domain service | Above or below, rows, x0 and width of the menu / the bold label and description the description bar and pop-out show. | `render::layout` / `render::describe` |
| `IconSlot` / `IconId` / `GraphicsSupport` | VO | Icon cell (two columns for every row kind; a spec badge is stored as given); which icon; kitty / tmux / off. | `icons::{Slot, Image, Graphics}` |
| `IconResolver` | Domain service | The icon cell and icon id for a suggestion under a graphics mode (emoji, `fig://` icons, type defaults). | `icons::{slot, spec_image, type_image, ascii_slot, wide_emoji}` |
| `DropdownError` | Domain error | Settings (printed as itself), data directory, invalid theme file, theme not found. | the `bail!`s of `cli::theme` |

## terminal_session
Not in the table: `icon_assets` (the embedded PNGs and the template RGBA; `tinted`, `kitty_image`) and `kitty_graphics` (the transmit/place bytes, `CLEAR_PLACEMENTS`, `detect`), adapters in `infrastructure/adapters/`; `EditBufferExtractor` (`edit_buffer_extractor.rs`) and `CellTagger` (`cell_tagger.rs`), the free functions that read the command line off the model and tag its cells; `headless_model` (the alias of the alacritty model); `DeferredMarker` and `LockSettlement` (return types of `MarkerTracker` and `InsertionLock`); `InMemory`/`Recording`/`Capturing` doubles of the ports; `TerminalSessionModule` (the module's composition entry) and `CompletionEngineStarter` (the alias of the engine-starting function); `TerminalSessionHarness` (`application/test_fixtures.rs`, the test arrangement every use-case test shares); `FrameInput` is a DTO of the application layer; and the `*Parts` structs (`TerminalSessionParts`, `EditBufferContextParts`, `ScreenStateParts`, `KeyFactsParts`, `SessionPortsParts`, `SessionRunnerParts`, `ProcessShellOutputParts`: named fields for a constructor that would otherwise take many parameters).

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `KeyEvent` | VO | A decoded `Key` with the byte range of the stdin chunk it came from; every byte of a chunk belongs to exactly one event. | `keys::KeyEvent` |
| `KeyDecoder` | Entity | Decodes stdin bytes (UTF-8, `ESC`, SS3, CSI, bracketed paste) into `KeyEvent`s; keeps only the paste state between chunks; reset when keys are held. | `StdinKeyDecoder` (the Phase 8 adapter), `keys::Decoder` |
| `OverlayRenderer` / `FrameInput` | Adapter / DTO | Draws the menu, description and ghost over the alacritty model as bytes, and the clear that restores the cells / the per-frame inputs, with the instant the spinner is judged at (an application DTO, `application/frame_input.rs`). | `render::{Renderer, FrameInput}` |
| `KittyImage` / `KittyPixels` | Adapter type | The id and the PNG or RGBA bytes of one icon, as the kitty graphics protocol transmits them. | `icons::{Image, Source}` |
| `TerminalSession` | Aggregate | The wrapper's view of the running shell: the latest request, the menu, the insertion lock, the echo hold, the ghost texts, the timers, the exit state; owns the `MarkerTracker`, the `MarkerSplitter` and the `KeyDecoder`. It decides and never acts: time and screen facts come in, values go out. | `pty::Session` (minus plumbing) |
| `SessionId` | VO | + `from_entropy(nanos, pid)`: 32 lowercase hex digits, the shape the shell scripts echo in `NewCmd=`. | `session_id()` |
| `SessionContext` | Entity | The env (UTF-8 entries only) and alias text a shell last reported through `_ pre-cmd`, stamped with the report time in ms. Identity `SessionId` (the file name, not a field of the file). An empty env means 'use the wrapper's env'; an empty alias text means no aliases. | `types::SessionCtx` |
| `MarkerSplitter` | Entity | Stateful splitter of PTY bytes into output and OSC 697 markers. | `tracker::Splitter` |
| `ShellMarker` / `OutputPiece` | VO | A parsed OSC 697 marker (total parse; each key keeps its own trimming; `Ignored` for empty values, unknown keys and the keys that set nothing) / what the splitter cuts a PTY batch into: bytes, a marker string, a window name. | marker strings, `Piece` |
| `MarkerTracker` | Entity | Prompt/command cycle from markers: cwd, shell, pid, exit code, integration lock, pending commands, deferral. Driven by the application: it takes the parsed marker, the cursor column and the wall-clock seconds, and answers with `MarkerEffects`. | `tracker::Tracker` state |
| `MarkerEffects` / `MarkerReaction` | VO | What one tracker step asks of its caller: the screen commands to apply now, in order, and the events to hand on after the batch / `Handled`, or `EditBufferNeeded` (the `PreExec` handshake). | |
| `ShellEvent` | VO | `NewCommand(Option<HistoryEntry>)`, `CommandEnded`, `PreExec`. | `tracker::Event` |
| `EditBuffer` | VO | + private fields; `cursor` is a byte offset; `at_prompt` means the cursor is on the command line, not `MarkerTracker::at_prompt`. | `Extracted` |
| `EditBufferContext` | VO | What reading the command line off the screen needs of the tracker (open cycle, shell, line start, list view, pwsh prompts, three colours), borrowed. | the `Tracker` fields `extract` read |
| `ShellSuggestionColor` | VO | Colours the shell uses for its own autosuggestion. | `SuggestionColor` |
| `PwshPredictionColor` / `TextEmphasis` | VO | PSReadLine's reported prediction colour and the dim/italic it requires. | `(SuggestionColor, Flags)` |
| `CellTag` | VO | Which cells are the prompt or the command line of cycle `n`: the `tabsmith:p` / `tabsmith:c{n}` URIs. | `TAG_PROMPT`, `format!("tabsmith:c{n}")` |
| `ScreenCommand` | VO | `Tag(CellTag)`, `ClearTag`, `MarkLineStart { redrawn_cycle }`. | inline `tag` / `mark_line_start` calls |
| `ScreenState` | VO | A fresh `Copy` snapshot of what the render guards read: alternate screen, origin and insert mode, pending wrap, size, scroll region, open string. | ad-hoc reads in `suppressed` / `drawable` |
| `GhostCandidates` | Entity | The local and the LLM ghost text, the LLM request id and its debounce deadline. | `ghost`, `llm`, `llm_req`, `llm_at` |
| `ResizeDebounce` | Entity | The quiet spell after a resize and the pending repaint (SPEC 7.6). | `resize_at`, `repaint` |
| `ShellExit` | Entity | Exit code, end of the output, the exit grace. | `exit_code`, `eof`, `exit_at` |
| `ReloadWatch` | Entity | When the settings and session-context files are looked at and the modification times seen. | `poll_at`, `settings_mtime`, `session_mtime` |
| `InsertionLock` | Entity | Hold the menu until the shell echoes an insertion (deadline, cap, expected buffer). | `lock` tuple |
| `EchoHold` | Entity | Typeahead not yet echoed + keys held until a reply arrives. | `unechoed`, `echo_pending`, `held` |
| `KeyRoute` / `KeyFacts` | VO | `Hold`, `MenuAction`, `Swallow`, `ShowOnTab`, `AcceptGhost`, `Forward` / what the router needs besides the key. | branches of `on_keys` |
| `CompletionQuery` | VO | What a refreshed line asks of the engine (request id, buffer, byte cursor, history mode); the service adds directory and shell. | the `CompletionRequest` literal |
| `KeyRouter` / `RenderGuard` / `SessionDeadlines` / `InsertionReconciler` | Domain service | Routes one key / whether the screen allows drawing / the nearest wake-up and the timer durations / the bytes and the expected buffer of an insertion. | `on_keys` arms, `drawable`, `next_deadline`, `insertion_bytes` |
| `WrappedShellCommand` | VO | The wrapped launch: argv, the five env changes of a session, the window title. | `pty::argv` + env setup |
| `TerminalBackground` | VO | A thin newtype over dropdown's `Rgb`: the background colour OSC 11 reported. | `terminal_bg` |
| `CompletionEngine` | Port | What the terminal session asks of completion: complete, a finished or ended command, an inserted suggestion, an LLM ghost text, settings, the session context, the fuzzy toggle, shut down. Never fails. | `Sender<EngineMsg>` in `pty.rs` |
| `TerminalScreen` | Port | The wrapper's mirror of the real terminal: pass shell output through the model and the filter, tag cells, read the command line and the render-guard state, resize, draw and clear the overlay. | `term`, `renderer`, `tracker.filter` |
| `TerminalWriter` / `WrappedShell` | Port | The user's terminal (stdout) / the wrapped shell's input and window size. | `emit`, `write_stdout`, `sink` / `writer`, `master` |
| `GeneratorProcessReaper` / `TerminalModes` | Port | Kills generator processes still running at the end / restores the terminal modes. | `kill_live_children` / `modes.restore` |
| `LiveSettings` (`LenientLiveSettings`) | Port | The settings a running session follows: the file's modification time and the lenient load (defaults and a warning when they cannot be loaded). | `load_or_defaults`, `settings_mtime` |
| `WorkingDirectory` (`ProcessWorkingDirectory`) | Port | The wrapper process's own directory, read at every request. | `std::env::current_dir()` in `Session::cwd` |
| `AlacrittyTerminalScreen` / `OutputFilter` | Adapter | The headless alacritty model (2000 lines of scrollback), its long-lived parser, the output filter and the session's ONE overlay renderer / filters the bytes written to the real terminal (keyboard-protocol upgrades, scroll region, open strings, held UTF-8). | `types::Model` + `Tracker::{proc, filter}` / `OutFilter` |
| `StdoutTerminalWriter`, `PtyWrappedShell`, `ConsoleTerminalModes`, `GeneratorProcessBridge` (bootstrap) | Adapter | The port implementations. | |
| `ShellOutputFeeder` | Application service | Feeds one PTY batch through the splitter, the tracker and the screen in stream order and returns the events. | `Tracker::feed` |
| `KeystrokeHandler` / `CompletionRequester` / `OverlayPainter` / `SessionFilePoller` | Application service | The key loop and the replay of held keys / the edit-buffer refresh and the request / drawing, redrawing and repainting the overlay / reloading settings and the session context. Shared by several use cases; never use cases themselves. | `on_keys`+`release_held` / `refresh` / `frame`+`redraw`+`take_repaint` / `poll_files` |
| `HandleKeystrokesUseCase`, `ProcessShellOutputUseCase`, `ApplyCompletionResultUseCase`, `ApplyLlmGhostTextUseCase`, `ResizeTerminalUseCase`, `HandleShellExitUseCase`, `HandleTimerTickUseCase`, `ApplyTerminalBackgroundUseCase`, `EndSessionUseCase`, `PrepareSessionLaunchUseCase` | Use case | One per event of the session (table in plan section 4, amended). | `Session::on_*`, `step`, `cleanup`, `run`'s resolution |
| `ShellEnd` / `LaunchRequest` / `LaunchEnvironment` / `SessionPortsParts` | DTO | How a shell ended / what the launch resolves a shell from / the env facts / the runtime ports one session runs on. | |
| `SessionUseCases` | Application service (composite) | All use cases of one session, wired once over `SessionPortsParts` in production (`launch_session`) and in the test harness; another module never reaches it (fitness rule 3). | the `on_*` delegates of `pty::Session` |
| `SessionRunner` / `SessionMessage` | Adapter (driver) | The event loop, the panic pass-through and the batching / the messages of its one inbox. | `pty::Session` (driver half) / `types::Msg` |

## diagnostics
As built in Phase 10 (`src/modules/diagnostics`, 64 files). A **check is data**: it carries a name, a severity and a subject, never a closure and never I/O. The rules live in the domain; the facts come through ports, one check at a time, so doctor stays lazy.

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `DoctorRun` | Entity | One doctor run: the `--all` and `--strict` policy and the error and warning counts. A Soft failure counts as a warning unless strict; the first unfixed failure aborts the run. | `doctor::Run` |
| `Check` | VO | One line of a group: name, `CheckSeverity` (decided when the group is built) and `CheckSubject`. | `Check` (closure-based) |
| `CheckGroup` | VO | A titled run of checks that doctor builds, prints and evaluates together (dotfiles, setup, environment, system). | `Run::group` arguments |
| `CheckSeverity` | VO | `Normal`, `Soft` (an error becomes a warning unless `--strict`) or `Skip` (never evaluated). | `Kind` |
| `CheckSubject` | VO | What a check evaluates: `Dotfile`, `ListingFailure`, `DataDirectory`, `ScriptsPointAtBinary`, `SettingsCorruption`, `SpecsExtracted`, `WindowsConsole`, `WindowsVersion`, `Environment(EnvironmentCheck, Rc<SystemFacts>)` or `System(SystemCheck)`. | the closure bodies |
| `EnvironmentCheck` | VO | A check of the environment group: `TerminalIntegrations`, `Session`, `AutocompleteEnabled`, `DeveloperMode`, `Inline`. | closures |
| `SystemCheck` | VO | A check of the system group: `Os`, `Bash { program }`, `Fish`. | closures |
| `CheckOutcome` | VO | `Ok`, `Warn(reason)` or `Err { reason, info: Vec<InfoLine>, fix: Option<CheckFix> }`. | `Outcome` |
| `InfoLine` | VO | One line of a failed check's details: plain text, or text around a command the reporter paints. | `info` strings |
| `CheckFix` | VO | What doctor can do to repair a failed check: `CreateDataDirectory`, `InstallIntegration`, `InstallMissingIntegrations` (installs what is missing; it is not a reinstall), `ResetSettings`, `InstallSpecBundle`. `marks_restart_reminder()` is the domain rule for the terminal restart reminder (the two integration fixes mark it, even when they installed nothing). | `Fix` closures; `ReinstallIntegrations` of the plan |
| `SystemFacts` | VO | The environment group's snapshot, read once when that group is built: the `TerminalEnvironment`, the session id and the `Settings`. Not a monolith: every other fact is asked through a port when its check is evaluated. | direct I/O inside checks |
| `TerminalEnvironment` | VO | The environment facts the terminal rule reads (`TERM_PROGRAM`, bundle identifier, `TABSMITH_TERM`, presence of emacs, CI and tabsmith launch variables, whether the parent is cmd, the tabsmith version); built from `TerminalEnvironmentParts`. | env reads in the terminal check |
| `OperatingSystem` | VO | `Linux`, `Macos`, `Windows` or `Other(name)`; the domain decides platform rules from it, never with `cfg`. | `consts::OS` |
| `ToolVersion` | VO | The numbers of a tool's version (`GNU bash, version 5.2.37(1)` is 5.2.37); never empty; compared by components. | `version_of` |
| `DiagnosticReport` | VO | The facts `tabsmith diagnostic` prints, section by section. | `Section` list |
| `DiagnosticSection` | VO | A named group of report rows, in insertion order. | `Section` |
| `DiagnosticValue` | VO | One value of a report row: `Text`, `Flag` or `Missing`. | `Section` rows |
| `DiagnosticsError` | Error | Why a fact could not be found (`IntegrationPaths`, `DataDirectory`); its text is the wrapped error's, unchanged. | `anyhow` context |
| `DoctorCheckCatalog` | Domain service | Free functions that hold every rule of doctor: which checks a group has, their names and headers, the severity rules and, per check, a pure function from facts to `CheckOutcome` (`dotfile_outcome`, `terminal_outcome`, `session_kind`, `inline_outcome`, `os_outcome`, `bash_outcome`, `fish_outcome`, `short_path`, ...). Where a fact is needed only sometimes the function takes a thunk. | the bodies of `doctor.rs` |
| `UsernameScrubber` | Domain service | Hides the user name in report values that can carry a path (a raw substring after a slash or backslash, no segment boundary). | the replace calls in `cli::diagnostics` |
| `CheckGroupBuilder` | Application service | Builds one group when asked and does that group's construction-time I/O through the ports (`which`, the bash login-file stats, the pwsh `$PROFILE` listing, the settings load and its warning). Not a use case. | the group arguments of `Run::group` |
| `CheckEvaluator` | Application service | Asks the ports for the facts of one check at the moment that check is evaluated, then calls the catalog; evaluating again after a fix asks again. Not a use case. | the check closures |
| `RunDoctorUseCase` | Use case | `execute(DoctorRequest) -> Result<DoctorVerdict, DiagnosticsError>`: builds a group only after the previous one passed, builds it before printing its header, evaluates check by check, repairs and re-evaluates, prints the summary and the restart epilogue. | `doctor::run` |
| `CollectDiagnosticReportUseCase` | Use case | `execute() -> DiagnosticReport` over the `DiagnosticHost`. | `cli::diagnostics` |
| `ShowRecentLogsUseCase` | Use case | Two methods so the command keeps its order: `paths() -> LogPaths` (no I/O besides the path), then `pty_tail(&LogPaths)` (the last 50 lines or `None`). | `cli::debug_logs` |
| `DoctorRequest` | DTO | What the user asked of `tabsmith doctor`: `all` and `strict`. | `Args` fields |
| `DoctorVerdict` | DTO | `Passed` or `Failed`; `Failed` makes the command exit 1. | the `bool` result |
| `LogPaths` | DTO | The `cli.log` and `pty.log` paths. | `RecentLogs` of the first design |
| `SystemFactsProbe` | Port | The machine as doctor asks it: parent executable, PATH lookup, environment, tabsmith version, operating system, file and directory existence, settings file and whether it loads, tool versions, console and Windows build. | direct calls in `doctor.rs` |
| `IntegrationProbe` | Port | `locate_paths`, `list`, `check` and `script_binaries` over the shell integrations. | `IntegrationInstaller` calls |
| `BundledSpecsProbe` | Port | Whether the bundle of this version is extracted below the data root. | `specs::version()` and a file check |
| `SettingsSnapshot` | Port | `load() -> Settings`; the lenient load that never fails. | `load_or_defaults` |
| `CheckFixer` | Port | `fix(&CheckFix, &IntegrationPaths) -> Result<(), String>`; the text is what follows `Failed to fix: `. | the `Fix` closures |
| `DoctorReporter` | Port | Everything doctor prints and the progress indicator: group header, outcome, fixing, failed to fix, re-running, summary, hint, restart notice. | `println!` calls |
| `LogReader` | Port | `read(path) -> Option<Vec<u8>>`. | `std::fs::read` |
| `DiagnosticHost` | Port | The host as the diagnostic report sees it: user, environment, shell versions, working directory, executable, WSL, version, platform. | env and process calls in `cli.rs` |
| `OsSystemFactsProbe` | Adapter | Implements `SystemFactsProbe` over the process table, environment, file system and child processes (the `cfg(windows)` bodies live here). | `doctor.rs` |
| `LenientSettings` | Adapter | Implements `SettingsSnapshot` over the `SettingsRepository`: on an error it logs the `settings: <error>` warning with the old text and uses the defaults. | `settings::load_or_defaults` |
| `FileRepairs` | Adapter | Creates the data directory and resets the settings file (backup copy, plain write, mode) with `std::fs`; takes a `Clock`. | the `Fix` closure bodies |
| `TerminalDoctorReporter` | Adapter | Implements `DoctorReporter` on stdout (styled when it is a terminal) and owns the `Spinner` on stderr. | `print_outcome` and the prints |
| `Spinner`, `restore_cursor_on_ctrl_c` | Adapters | `Running checks...` on stderr and the Ctrl-C handler that restores the cursor (SIGINT watcher on unix, `SetConsoleCtrlHandler` on Windows). | `doctor::Spinner` |
| `FileLogReader`, `OsDiagnosticHost` | Adapters | Implement `LogReader` and `DiagnosticHost` (`first_line_of` is the pure helper of the shell version). | `cli.rs` |
| `DiagnosticsModule`, `build_diagnostics_module` | Module | The three use cases over the adapters; building only constructs (no env read, no PATH lookup, no spawn, no file opened). | |
| `IntegrationProbeBridge`, `BundledSpecsBridge`, `CheckFixerBridge` | Bridges (`bootstrap/diagnostics_bridges.rs`) | The only code that names `IntegrationInstaller`, `integration_failures_error`, `embedded_spec_bundle::version` and `InstallSpecBundleUseCase` on behalf of diagnostics. `CheckFixerBridge` applies a fix with five explicit arms, renders any failure with `{:#}` and marks the restart reminder after an `Ok` iff `CheckFix::marks_restart_reminder()`. | `IntegrationInstaller` wiring in `doctor.rs` |

## Words with more than one meaning: use the qualified name, never the bare word
| Word | Name for each meaning |
|---|---|
| lock | `InsertionLock` (wait for echo) · integration lock (OSC 697 Lock/Unlock, a field of `MarkerTracker`) |
| prompt | shell prompt · `LlmPrompt` · the `inline set-customization` chooser prompt (`Select a customization: `, written by `StdinSourceChooser`; no domain type) · the `[Y/n]` confirmation `setup`/`uninstall` ask (`confirm`/`answer` in shell_integration's `infrastructure/cli/user_confirmation.rs`; no port, not a domain type) |
| ghost | `GhostText` (ours) · shell autosuggestion (zsh/fish/PSReadLine's own) |
| history | `HistoryEntry` / `HistoryRepository` (store) · `SuggestionKind::History` (rows) · history mode (Ctrl-R in `Menu`) |
| session | `TerminalSession` · `SessionContext` · `SessionId` · `CompletionSession` · the doctor `Session` check (`EnvironmentCheck::Session`: has the shell reported its environment, i.e. does the session's file under `run/` exist) |
| at prompt | `MarkerTracker::at_prompt` (a command cycle is open and no command runs) · `EditBuffer::at_prompt` (the cursor is on the command line, not in an inline TUI's rows) |
| shell | the launched shell (`launched_shell`: the program the wrapper spawns) · the tracker's `Shell` (what the prompt reported with `Shell=`; it wins once known and can be `None`) |
| screen | the real terminal · the model (the headless alacritty `Term`) · `TerminalScreen` (the port over the model, the filter and the renderer) |
| suggestion | `SpecSuggestion` (declared) · `SuggestionCandidate` (pipeline) · `Suggestion` (shown) |
| completion | the module/feature · `CompletionResult` (one reply) |
| host / pump | gone. `QuickJsSpecRuntime` replaces the js Host, which becomes its task registry; `SpecRepository` + `SpecRuntime` replace the parser `Host` trait. `EngineInbox` (`bootstrap/engine_inbox.rs`) implements `RuntimeEventSource`; the `EngineServer` reads `EngineMessage`s from it, which `ChannelCompletionEngine` sends for the pty. The pumps become `pty_reader`, `ProgressGeneratorsUseCase` + `GeneratorRunner`, and runtime internals. |
| cache | `GeneratorCache`, `GhostTextCache`, `ParsePrefixCache` (in `CompletionSession`), first-token cache (in `CompletionSession`), spec cache and module cache (inside adapters), detected CLI version memo and in-flight version probes (inside `QuickJsSpecRepository`) |
| profile | pwsh `$PROFILE` (the pwsh dotfile, carried by `IntegrationPaths`) · `LlmProfile` |
| script | `IntegrationScript` (files tabsmith writes) · `InitScript` / `InitScriptTemplates` (what `init` prints) · `GeneratorScript` (spec) |
| version | the bundle version (`assets/SPECS_VERSION`, `specs/<version>`) · the CLI version (`SemanticVersion`) · the diff version (a key of a diff-versioned spec's `versions`) |
| theme file | `ThemeDefinition` (the strict read: its author, for the `theme NAME` banner) · `ThemePalette` (the lenient read: its colour roles, for drawing) |
| run | `DoctorRun` (one doctor run and its counts) · `RunDoctorUseCase` (the use case that drives it) · the `run/` directory of session files (`DataDirectory::run_dir`) |
| check | `Check` (a doctor line, data) · `check` of the `IntegrationProbe` (does an integration's dotfile hold its block) · `CheckSubject` (what a check evaluates) |
| probe | a diagnostics port that asks the machine (`SystemFactsProbe`, `IntegrationProbe`, `BundledSpecsProbe`) · the terminal probe of `terminal_session` (`probe_terminal`: cursor position and OSC 11 at start-up) |
| report | `DiagnosticReport` (what `tabsmith diagnostic` prints) · `DoctorReporter` (the port that prints a doctor run) · `report_session_context` (`_ pre-cmd`) |
| fix | `CheckFix` (data: what to do) · `CheckFixer::fix` (doing it) · `fixing` (the reporter line `Attempting to fix automatically...`) |
| logs | the two log files `logs/cli.log` and `logs/pty.log` (`LogPaths`, `debug logs`) · `logger` (the `shared` writer) |
