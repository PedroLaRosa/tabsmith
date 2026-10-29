# tabsmith ubiquitous language

The names the hexagonal refactor ([`hexagonal-refactor-plan.md`](hexagonal-refactor-plan.md)) gives to every
domain entity and value object, grouped by module. Code, tests, commit messages and docs use exactly these names.
"Replaces" points at the code as of `4f26859`, before the refactor.

## shared (kernel)
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `Shell` | VO | A supported shell: Bash, Zsh, Fish, Pwsh. Built from a path or name. | `types::Shell` |
| `DataDirectory` | VO | tabsmith's home (`~/.tabsmith` or `TABSMITH_HOME`) and every path under it: settings, state, history, run, logs, themes, shell, backups, specs. | `settings::{home, settings_path, state_path}` + ad-hoc joins |
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
| `SuggestionKind` | VO | Subcommand, Option, Arg, File, Folder, Special, Shortcut, History, AutoExecute. | `types::ItemType` |
| `SpecLocation` | VO | Global `{name}` or Local `{dir, name}`; the spec's identity. | `SpecLocation`, `loc_key` |
| `SpecFunctionRef` | VO | Opaque handle to a function inside a loaded spec. | `FnId` / `{"$fn": n}` |
| `TextOrFunction` | VO | A literal or a spec function. | `StrOrFn` |
| `GeneratorTrigger` | VO | When a generator reruns: string, function, change, threshold, match. | `Trigger` |
| `GeneratorCachePolicy` | VO | TTL, max age, cache by directory, cache key. | `CacheCfg` |
| `GeneratorScript` | VO | Argv, legacy shell string, function, or typed `ExecRequest`. | `Script` (`Object(Value)` becomes typed) |
| `Repeatability` | VO | No / Times(n) / Unlimited. | `Repeat` |
| `FilterStrategy` | VO | Default / Prefix / Fuzzy. | `Strategy` |
| `ParserDirectives` | VO | flagsArePosixNoncompliant, optionsMustPrecedeArguments, optionArgSeparators. | `Directives` |
| `SpecLoadDirective` | VO | Locations / Inline spec / Function. | `LoadSpec` |
| `SpecIndex` | VO | Root spec names + diff-versioned set. | `SpecIndex` |
| `SemanticVersion` | VO | Parsed CLI version, ordering. | `SemVer` tuple + `semver*` |
| `GeneratorContext` | VO | cwd, env, shell, search term, dangerous flag passed to generators. | `js::GenCtx` |
| `ExecRequest` | VO | `executeCommand` input: command, args, cwd, env changes, timeout. | `js::ExecInput` |
| `ProcessOutput` | VO | stdout, stderr, status. | `types::ExecOutput` |
| `SpecFunctionCall` / `SpecFunctionResult` | Port types | Typed call into a spec function and its typed result (load spec, generate spec, alias, query term, trigger, post-process, custom, script). | `JsArg` + `serde_json::Value` |

## completion
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `Token` | VO | One shell word: unquoted text, raw text, byte span. | `shell_parser::Token` |
| `CommandLine` | VO | The command under the cursor: tokens, text after alias expansion, original text. | `CommandInfo` |
| `AliasTable` | VO | Alias name → expansion. | `parse_aliases` map |
| `TokenAnnotation` / `AnnotationKind` | VO | How a token was consumed (subcommand, option, option arg, subcommand arg). | `Annotation`, `AnnKind` |
| `ParsedCommandLine` | VO | Result of walking argv against a spec (current arg, search term, passed options, …). | `ParseResult` |
| `SuggestionCandidate` | VO | A row inside the filter/rank pipeline. | `suggest::Sugg` |
| `SuggestionQuery` | VO | How a row's query term is derived. | `Query` |
| `FilterOptions` | VO | fuzzy, auto-execute rules, dangerous, current token. | `FilterOpts` |
| `InsertionContext` | VO | Search term, raw token, shell, flags for composing an insertion. | `InsCtx` |
| `Insertion` | VO | Keystrokes that apply a suggestion: delete n, text, cursor back, execute. | `types::Insertion` |
| `Suggestion` | VO | A final, ranked row shown in the dropdown. | `types::Item` |
| `ArgumentHint` | VO | Name + description of the current arg. | `ArgInfo` |
| `CompletionPhase` | VO | Parse (first reply) or Generators (follow-up). | `Phase` |
| `CompletionResult` | VO | The reply for one request: suggestions, tokens, search term, loading, ghost, … | `types::Completion` |
| `CompletionRequest` | DTO | Request id, buffer, cursor, cwd, shell name, history mode. | `types::Request` |
| `GeneratorKey` | VO | Identity of a generator slot (arg identity + index). | `GenKey`, `gen_key` |
| `GeneratorSlot` | Entity | A generator's current result for the current arg (loading, generation). | `engine::Slot` |
| `GeneratorRun` / `GeneratorStep` | Entity / VO | One generator execution moving through Debounce → Script → Exec → PostProcess → Custom → Filter. | `engine::{Run, Step, Outcome}` |
| `GeneratorCache` / `CacheLookup` | Entity / VO | Generator results with fresh / stale / in-flight / miss lifecycle. | `GenCache`, `Lookup` |
| `ParsePrefixCache` | Entity | Parse states memoised per prefix + generateSpec cache. | `ParseCaches` |
| `SuggestionRecency` | Entity | Command → suggestion name → last insertion time (ranking boost). | `engine::Recency` |
| `CompletionSession` | Aggregate | Engine-thread state: current request, slots, runs, caches, fuzzy toggle, aliases, session env. | `Engine` fields |

## ghost_text
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `GhostText` | VO | Gray continuation after the cursor; accept all / next word. | `Option<String>` + `pty::{ghost_key, next_word, ghost_rest}` |
| `GhostTextSource` | VO | history, specs, history+specs, or an LLM profile. | `inline::Source` |
| `LlmProfile` / `LlmProvider` | VO | Provider, model, base URL, API key env var, max tokens, effort. | `Profile`, `Provider` |
| `LlmPrompt` | VO | System + user text sent to the provider (privacy-filtered history). | `history_prompt` output |
| `GhostTextCache` | Entity | Recent LLM completions with priorities, cleared per command. | `GhostCache` |

## dropdown
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `Menu` | Aggregate | Dropdown state: visibility, suggestions, selection, scroll, history mode, pop-out. | `menu::Menu` |
| `MenuVisibility` | VO | Visible, HiddenUntilKeypress, HiddenUntilShown, HiddenByInsertion. | `Vis` |
| `MenuEffect` | VO | Insert, Write, Bell, Rerequest, ToggleFuzzy. | `Effect` |
| `Key` / `KeyCode` / `Modifiers` | VO | A decoded keystroke. | `keys::{Key, KeyCode, Mods}` |
| `KeyAction` / `KeyBinding` / `Keymap` | VO | What a keystroke does; default + user bindings. | `Action`, `Binding`, `Keymap` |
| `Theme` / `Rgb` / `ThemeChoice` / `ThemeDefinition` | VO | Dropdown colours; dark/light/system/custom choice; parsed custom theme. | `render::Theme`, `cli::ThemeFile` |
| `MenuLayout` / `MenuSide` | VO | Geometry: above or below, rows, x0, width. | `render::{Layout, Side}` |
| `IconSlot` / `IconId` / `GraphicsSupport` | VO | Two-column icon cell; which icon; kitty / tmux / off. | `icons::{Slot, Image, Graphics}` |

## terminal_session
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `TerminalSession` | Aggregate | The wrapper's view of the running shell: request counter, menu, insertion lock, echo hold, ghost, timers, exit state. | `pty::Session` (minus plumbing) |
| `SessionId` | VO | Session identifier shared with the shell scripts: the `TABSMITH_SESSION_ID` the wrapper exports and the `run/<id>.json` file name. Valid when non-empty and made only of ASCII letters, digits, `-` and `_`, exactly what `_ pre-cmd` accepts; not narrowed to the hex ids the wrapper generates (D1). | `session_id()` + the `_ pre-cmd` id filter |
| `SessionContext` | Entity | The env (UTF-8 entries only) and alias text a shell last reported through `_ pre-cmd`, stamped with the report time in ms. Identity `SessionId` (the file name, not a field of the file). An empty env means 'use the wrapper's env'; an empty alias text means no aliases. | `types::SessionCtx` |
| `MarkerSplitter` | Entity | Stateful splitter of PTY bytes into output and OSC 697 markers. | `tracker::Splitter` |
| `ShellMarker` / `OutputPiece` | VO | A parsed OSC 697 marker; bytes / marker / window name. | marker strings, `Piece` |
| `MarkerTracker` | Entity | Prompt/command cycle from markers: cwd, shell, pid, exit code, integration lock, pending commands, deferral. | `tracker::Tracker` state |
| `ShellEvent` | VO | NewCommand(Option<HistoryEntry>), CommandEnded, PreExec. | `tracker::Event` |
| `EditBuffer` | VO | Command-line text, cursor, at-prompt flag, shell-autosuggestion flag. | `Extracted` |
| `ShellSuggestionColor` | VO | Colours the shell uses for its own autosuggestion. | `SuggestionColor` |
| `ScreenCommand` | VO | Tag cells from the cursor, clear tag, mark line start. | inline `tag` / `mark_line_start` calls |
| `ScreenState` | VO | Alt screen, origin/insert mode, pending wrap, size, scroll region, in-string. | ad-hoc reads in `suppressed` / `drawable` |
| `InsertionLock` | Entity | Hold the menu until the shell echoes an insertion (deadline, cap, expected buffer). | `lock` tuple |
| `EchoHold` | Entity | Typeahead not yet echoed + keys held until a reply arrives. | `unechoed`, `echo_pending`, `held` |
| `KeyRoute` | VO | Decision for one keystroke: forward, menu action, accept ghost, … | branches of `on_keys` |
| `WrappedShellCommand` | VO | argv + env changes for the child shell. | `pty::argv` + env setup |
| `TerminalBackground` | VO | Background colour reported by OSC 11 (for the `system` theme). | `terminal_bg` |

## diagnostics
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `DoctorRun` | Entity | One doctor run: strict/all policy, error and warning counts. | `doctor::Run` |
| `Check` / `CheckGroup` | VO | A named check with a severity; a headed group. | `Check` (closure-based) |
| `CheckSeverity` | VO | Normal, Soft, Skip. | `Kind` |
| `CheckOutcome` | VO | Ok, Warn(reason), Err(reason, info, fix). | `Outcome` |
| `CheckFix` | VO | CreateDataDirectory, InstallIntegration, ReinstallIntegrations, ResetSettings, InstallSpecBundle. | `Fix` closures |
| `SystemFacts` | VO | Snapshot of env, OS, tool versions, file existence and each integration's check result (`Result<(), ShellIntegrationError>`) that checks evaluate. | direct I/O inside checks |
| `ToolVersion` | VO | Parsed `bash`/`fish` version. | `version_of` |
| `DiagnosticReport` / `DiagnosticSection` | VO | `tabsmith diagnostic` content. | `Section` |

## Words with more than one meaning: use the qualified name, never the bare word
| Word | Name for each meaning |
|---|---|
| lock | `InsertionLock` (wait for echo) · integration lock (OSC 697 Lock/Unlock, a field of `MarkerTracker`) |
| prompt | shell prompt · `LlmPrompt` · the `[Y/n]` confirmation `setup`/`uninstall` ask (`confirm`/`answer` in shell_integration's `infrastructure/cli/user_confirmation.rs`; no port, not a domain type) |
| ghost | `GhostText` (ours) · shell autosuggestion (zsh/fish/PSReadLine's own) |
| history | `HistoryEntry` / `HistoryRepository` (store) · `SuggestionKind::History` (rows) · history mode (Ctrl-R in `Menu`) |
| session | `TerminalSession` · `SessionContext` · `SessionId` · `CompletionSession` |
| suggestion | `SpecSuggestion` (declared) · `SuggestionCandidate` (pipeline) · `Suggestion` (shown) |
| completion | the module/feature · `CompletionResult` (one reply) |
| host / pump | gone. `QuickJsSpecRuntime` replaces the js Host struct; `SpecRepository` + `SpecRuntime` replace the parser `Host` trait. The pumps become `pty_reader`, `ProgressGeneratorsUseCase` and runtime internals. |
| cache | `GeneratorCache`, `GhostTextCache`, `ParsePrefixCache`, first-token cache (in `CompletionSession`), spec cache and module cache (inside adapters) |
| profile | pwsh `$PROFILE` (the pwsh dotfile, carried by `IntegrationPaths`) · `LlmProfile` |
| script | `IntegrationScript` (files tabsmith writes) · `InitScript` / `InitScriptTemplates` (what `init` prints) · `GeneratorScript` (spec) |
