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
| `IntegrationPhase` | VO | `Pre` (top of the dotfile) or `Post` (bottom). | `When` |
| `IntegrationTarget` | VO | Dotfiles of one shell, or all. | `IntegrationTarget` |
| `IntegrationStatus` | VO | Installed, NotInstalled, Improper, FileDoesNotExist, PermissionDenied (+ reason). | `IntegrationError` |
| `Dotfile` | VO | Text of an rc file. Knows BOM, shebang, `# tabsmith ignore`, and how to insert, remove and check the pre/post blocks. | logic inside `install`, `uninstall`, `is_installed` |
| `IntegrationScript` | VO | `~/.tabsmith/shell/<rc>.<phase>.<ext>`: path + exact contents. | `script_path`, `script_contents` |
| `InitScript` | VO | Rendered `tabsmith init <shell> <phase>` output. | `init_output` |
| `LaunchFacts` / `LaunchDecision` | VO | Env + tty facts in; launch yes/no + reason out. | `launch_decision` args, `(bool, String)` |
| `RestartReminder` | VO | "Ask the user to restart the terminal" flag in `state.json`. | `mark_restart`, `doctor.prompt-restart-terminal` |

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
| `SessionId` | VO | Session identifier shared with the shell scripts. | `session_id()` |
| `SessionContext` | Entity | Env + aliases reported by `_ pre-cmd`; identity `SessionId`. | `types::SessionCtx` |
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
| `SystemFacts` | VO | Snapshot of env, OS, tool versions, file existence and integration statuses that checks evaluate. | direct I/O inside checks |
| `ToolVersion` | VO | Parsed `bash`/`fish` version. | `version_of` |
| `DiagnosticReport` / `DiagnosticSection` | VO | `tabsmith diagnostic` content. | `Section` |

## Words with more than one meaning: use the qualified name, never the bare word
| Word | Name for each meaning |
|---|---|
| lock | `InsertionLock` (wait for echo) · integration lock (OSC 697 Lock/Unlock, a field of `MarkerTracker`) |
| prompt | shell prompt · `LlmPrompt` · `UserConfirmation` (y/n question) |
| ghost | `GhostText` (ours) · shell autosuggestion (zsh/fish/PSReadLine's own) |
| history | `HistoryEntry` / `HistoryRepository` (store) · `SuggestionKind::History` (rows) · history mode (Ctrl-R in `Menu`) |
| session | `TerminalSession` · `SessionContext` · `SessionId` · `CompletionSession` |
| suggestion | `SpecSuggestion` (declared) · `SuggestionCandidate` (pipeline) · `Suggestion` (shown) |
| completion | the module/feature · `CompletionResult` (one reply) |
| host / pump | gone. `QuickJsSpecRuntime` replaces the js Host struct; `SpecRepository` + `SpecRuntime` replace the parser `Host` trait. The pumps become `pty_reader`, `ProgressGeneratorsUseCase` and runtime internals. |
| cache | `GeneratorCache`, `GhostTextCache`, `ParsePrefixCache`, first-token cache (in `CompletionSession`), spec cache and module cache (inside adapters) |
