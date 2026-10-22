# tabsmith research: the CLI command surface for autocomplete (from Amazon Q CLI / `q_cli`)

Source root for every citation below: `ref/qac/` (aws/amazon-q-developer-cli-autocomplete, workspace version `1.19.7`, `Cargo.toml:23`).
Citation format: `path:line` or `path:start-end`. "(inferred)" = not read verbatim from code. "(rec)" = recommendation for tabsmith.

Upstream names → tabsmith names (rec):

| upstream const | value | file | tabsmith |
|---|---|---|---|
| `CLI_BINARY_NAME` | `q` | `crates/fig_util/src/consts.rs:15` | `tabsmith` |
| `PTY_BINARY_NAME` | `qterm` | `consts.rs:18` | `tabsmith` subcommand (e.g. `tabsmith _ pty`) or a 2nd binary `tabsmith-pty` |
| `CHAT_BINARY_NAME` | `qchat` | `consts.rs:17` | n/a (out of scope) |
| `PRODUCT_NAME` | `Amazon Q` | `consts.rs:24` | `tabsmith` |
| `DATA_DIR_NAME` | `amazon-q` (unix) / `AmazonQ` (windows) | `consts.rs:29-32` | `tabsmith` / `Tabsmith` |
| `RUNTIME_DIR_NAME` | `cwrun` | `consts.rs:26` | `tabsmith-run` |
| `BACKUP_DIR_NAME` | `.amazon-q.dotfiles.bak` | `consts.rs:35` | `.tabsmith.dotfiles.bak` |
| env prefix | `Q_*`, `QTERM_SESSION_ID` | `consts.rs:95-144` | `TABSMITH_*`, `TABSMITH_SESSION_ID` |
| OSC prefix | `\033]697;` | shell scripts | keep (inshellisense uses its own, see other report) |

All user-visible strings below contain `q`/`Amazon Q`/`qterm`; a faithful rebrand substitutes them 1:1.

---

## 1. Global CLI behaviour

### 1.1 Root parser (`crates/q_cli/src/cli/mod.rs:284-295`)

```rust
#[derive(Parser)]
#[command(version, about, name = CLI_BINARY_NAME, help_template = HELP_TEXT)]
pub struct Cli {
    #[command(subcommand)] pub subcommand: Option<CliRootCommands>,
    /// Increase logging verbosity
    #[arg(long, short = 'v', action = ArgAction::Count, global = true)] pub verbose: u8,
    /// Print help for all subcommands
    #[arg(long)] help_all: bool,
}
```

- `-v` count → log level: 1=WARN, 2=INFO, 3=DEBUG, ≥4=TRACE (`mod.rs:301-312`). Any `-v` also logs to stdout (`mod.rs:313`); env `Q_LOG_STDOUT` (any value) also logs to stdout (`crates/fig_os_shim/src/env.rs:178-180`).
- Log file (in `logs_dir()`): `chat.log` for chat, `translate.log`, `mux.log` for `_ multiplexer`, otherwise `cli.log` **only if** max log level ≥ DEBUG (`mod.rs:314-323`). `Q_LOG_LEVEL` env sets level (`env.rs:174-176`).
- `--help-all`: prints `\nUSAGE:` (bold+underlined) `\n    q [OPTIONS] [SUBCOMMAND]\n` to **stderr**, then clap long help with template `{all-args}` to stdout; exit 0 (`mod.rs:481-490`).
- Custom `HELP_TEXT` (shown for `q --help`, `mod.rs:266-282`): a boxed "Popular Subcommands" table (chat, translate, doctor, settings, quit) + `To see all subcommands, use: ❯ q --help-all`. (rec) tabsmith: box lists `doctor`, `settings`, `theme`, `inline`, `integrations`.
- **No subcommand** → launches chat (`mod.rs:394`). (rec) tabsmith: print help (exit 0).

### 1.2 main / exit codes (`crates/q_cli/src/main.rs`)

- Multi-thread tokio runtime only when argv[1] ∈ {`init`,`_`,`internal`,`completion`,`hook`,`chat`}, else current-thread (`main.rs:29-34,63-69`).
- Clap parse error: `err.print()`; if kind is `UnknownArgument|InvalidSubcommand` **and** clap offered no suggestion, additionally prints to stderr:
  `\nThis command may be valid in newer versions of the Amazon Q CLI. Try running q update.` (`q` and `update` magenta). Exit code = clap's exit code (2 for usage errors, 0 for `--help/--version`) (`main.rs:36-58`).
- Command returns `Err(e)` → stderr `error: {e}` (`error:` bold red); with `-v` or log level > INFO prints `{e:?}` (eyre debug, backtrace). Exit code **1** (`main.rs:77-86`).
- Commands return `ExitCode` explicitly otherwise (0/1, or custom like `should-figterm-launch` 0/1/2).

### 1.3 `OutputFormat` (shared `--format/-f`) (`mod.rs:92-117`)

```rust
#[derive(ValueEnum, Default)] enum OutputFormat { #[default] Plain, Json, JsonPretty }
```
CLI values: `plain`, `json`, `json-pretty`. `print(text_fn, json_fn)`: Plain → `println!("{}", text)`, Json → `serde_json::to_string`, JsonPretty → `to_string_pretty` (2-space indent).

### 1.4 Telemetry gating (`mod.rs:465-478`)
No telemetry for `init`, `_`/`internal`, `completion`, `hook`, root. (Out of scope for tabsmith: drop telemetry entirely.)

---

## 2. Root command table (`mod.rs:125-227`)

| Command | aliases | hidden | Args type | tabsmith scope |
|---|---|---|---|---|
| `hook <sub>` | – | yes | `HookSubcommand` | maybe (see §14) |
| `debug <sub>` | – | no | `DebugSubcommand` | partial (§16) |
| `settings` | `setting` | no | `SettingsArgs` | **yes** (§3) |
| `setup` | `install` | no | `InstallArgs` | **yes** (§18) |
| `uninstall [-y/--no-confirm]` | – | yes | inline struct | **yes** (§18) |
| `update [-y] [--relaunch-dashboard=<bool>] [--rollout]` | `upgrade` | no | `UpdateArgs` (`update.rs:49-60`) | no |
| `diagnostic [-f fmt] [--force]` | `diagnostics` | no | `DiagnosticArgs` | **yes** (§20) |
| `init <shell> <pre\|post> [--rcfile X]` | – | no | `InitArgs` | **yes** (§11) |
| `theme [--list\|--folder\|<THEME>]` | – | no | `ThemeArgs` | **yes** (§8) |
| `issue [-f] [desc...]` | – | no | `IssueArgs` | yes (§20) |
| `login`/`logout`/`whoami`/`profile` | – | no | flattened `RootUserSubcommand` | no |
| `user <sub>` | – | no | – | no |
| `doctor [-a] [-s]` | – | no | `DoctorArgs` | **yes** (§13) |
| `completion [bash\|fish\|zsh\|fig]` | – | yes | `CompletionArgs` | **yes** (§17) |
| `_ <sub>` / `internal <sub>` | `_` | yes | `InternalSubcommand` | **yes** subset (§15) |
| `launch`, `quit`, `restart [app]`, `dashboard` | – | no | – | no (desktop) |
| `integrations <sub>` | `integration` | no | `IntegrationsSubcommands` | **yes** (§10) |
| `translate` | `ai` | no | – | no |
| `telemetry enable\|disable\|status [-f]` | – | yes | – | no |
| `version [--changelog[=all\|x.y.z]]` | – | yes | inline | **yes** (§19) |
| `chat`, `mcp`, `agent` (trailing var args) | – | no | – | no |
| `inline <sub>` | – | no | `InlineSubcommand` | **yes** (§9) |

Doc comments become clap help text, e.g. `/// Customize appearance & behavior` for settings (`mod.rs:135`), `/// Generate the dotfiles for the given shell` for init (`mod.rs:154`), `/// Fix and diagnose common issues` for doctor (`mod.rs:166`), `/// Manage system integrations` (`mod.rs:184`), `/// Inline shell completions` (`mod.rs:217`), `/// Get or set theme` (`mod.rs:156`), `/// Run diagnostic tests` (`mod.rs:151`), `/// Create a new Github issue` (`mod.rs:158`), `/// Setup cli components` (`mod.rs:138`).

---

## 3. `settings` (`crates/q_cli/src/cli/settings.rs`)

### 3.1 Clap definition (`settings.rs:42-81`)

```rust
#[derive(Subcommand)]
enum SettingsSubcommands {
    /// Open the settings file
    Open,
    /// List settings
    List { /// Show all available settings
           #[arg(long)] all: bool,
           /// Format of the output
           #[arg(long, short, value_enum, default_value_t)] format: OutputFormat },
    /// List configured settings
    #[command(hide = true)]
    All { #[arg(long, short, value_enum, default_value_t)] format: OutputFormat },
}

#[derive(Args)]
#[command(subcommand_negates_reqs = true)]
#[command(args_conflicts_with_subcommands = true)]
#[command(group(ArgGroup::new("vals").requires("key").args(&["value", "format"])))]
struct SettingsArgs {
    #[command(subcommand)] cmd: Option<SettingsSubcommands>,
    /// key
    key: Option<String>,
    /// value
    value: Option<String>,
    /// Delete a key (No value needed)
    #[arg(long, short)] delete: bool,          // -d / --delete
    /// Format of the output
    #[arg(long, short, value_enum, default_value_t)] format: OutputFormat,  // -f / --format
}
```
Consequences: `q settings --format json` without a key is a clap error (group `vals` requires `key`). `q settings list foo` conflicts. Note positional `key` vs subcommand name: `q settings open` is the subcommand, so a setting literally named `open`/`list`/`all` cannot be read positionally (inferred from clap semantics).

### 3.2 Behaviour (`settings.rs:84-230`)

| Invocation | Behaviour | Output | Exit |
|---|---|---|---|
| `settings open` | macOS: `open <settings_path>`; else spawn `$EDITOR <settings_path>` and wait; no EDITOR → error `The EDITOR environment variable is not set` (`:92-103`) | – | 0 / 1 |
| `settings list [--all] [-f F]` | **delegates** to `qchat settings list [--all] [--format F]` (format only appended when ≠ plain), exits 0 if child success else 1 (`:104-121`) | child's | 0/1 |
| `settings all [-f F]` (hidden) | same as `list` without `--all` (`:122-137`) | | |
| `settings KEY` (get) | `fig_settings::settings::get_value(key)` (`:143`). Found: Plain → if JSON string print raw string, else pretty JSON (`{:#}`); Json → compact JSON; JsonPretty → pretty JSON (`:145-152`). Missing: Plain → `Err("No value associated with {key}")` → `error: No value…` exit 1; Json/JsonPretty → prints `null`, exit 0 (`:155-161`) | | 0/1 |
| `settings KEY VALUE` (set) | value parsed as JSON, fallback to JSON string: `serde_json::from_str(v).unwrap_or_else(\|_\| json!(v))` (`:164`) → `true`→bool, `5`→number, `[...]`→array, `"x"`→string x, `foo`→string "foo". Writes file. No output. | – | 0 |
| `settings KEY VALUE -d` | error `the argument '--delete' cannot be used with '[VALUE]'\n Usage: q settings --delete {key}` (`:140-142`) | | 1 |
| `settings -d PATTERN` | PATTERN is a **glob** (`globset::Glob`) matched against all keys of the JSON map (`:169-172`). 0 matches → error `No settings found matching {key}`; 1 → prints `Removing "{key}"` (Debug-quoted) then removes; >1 → prints `Removing:` then `  - {key}` per key, then removes each (`:174-191`) | | 0/1 |
| `settings -d` (no key) | error `the argument '--delete' requires a <KEY>\n Usage: q settings --delete <KEY>` (`:198-202`) | | 1 |
| `settings` (nothing) | minimal build or remote → behaves like `settings --help` (`Cli::parse_from([q,"settings","--help"])` prints help and exits 0) (`:204-207`); else launches desktop + opens Settings UI (`:209-226`) | | 0 |

`settings list` real implementation lives in `qchat` (not in this repo). (inferred) tabsmith implementation:
- `list` (no `--all`): print every configured key, sorted, Plain as `key = <json value>` (mirrors `_ local-state all`, `local_state.rs:74-78`), Json → whole map object.
- `list --all`: print every known key from the catalog (§5) with type, default, description, and current value.
(rec) For tabsmith with no desktop app: bare `settings` → print help (same as minimal path).

---

## 4. Settings and state storage

### 4.1 Two stores

| Store | API | Backing | Semantics |
|---|---|---|---|
| **Settings** (user preferences) | `fig_settings::settings::{get_value,set_value,remove_value,get_bool_or,…}` (`crates/fig_settings/src/settings.rs:164-210`) | JSON file `settings.json` | user-editable, shown in `q settings` |
| **State** (internal flags/counters) | `fig_settings::state::{…}` (`crates/fig_settings/src/state.rs:121-163`) | SQLite table `state` in `data.sqlite3` | internal; edited with `q _ local-state` |

### 4.2 Settings file (`crates/fig_settings/src/lib.rs`, `crates/fig_util/src/directories.rs`)

- Path = `fig_data_dir()/settings.json` (`directories.rs:454-456`), with `fig_data_dir()` = `dirs::data_local_dir()/{DATA_DIR_NAME}` (`directories.rs:146-150`):
  - Linux: `$XDG_DATA_HOME/amazon-q/settings.json` or `~/.local/share/amazon-q/settings.json`
  - macOS: `~/Library/Application Support/amazon-q/settings.json`
  - Windows: `%LOCALAPPDATA%\AmazonQ\settings.json`
- **No env var overrides the path** (the old `FIG_DIRECTORIES_SETTINGS_PATH` only appears in commented-out tests, `lib.rs:428-433`). (rec) tabsmith: add `TABSMITH_SETTINGS_PATH` for tests.
- Format: **one flat JSON object** whose keys are dotted strings; there is **no nesting** — `"autocomplete.theme": "dark"` is a literal top-level key. Values are arbitrary JSON (bool/number/string/array/null).
- Load (`lib.rs:213-239`): create parent dirs; if file missing, create it containing `{}`; else parse with `serde_json::from_reader` under an exclusive `fd_lock` write lock + in-process `RwLock`. Parse failure ⇒ error (callers propagate → `error: …`, exit 1). Every API call re-reads the file (`Settings::Real` → `OldSettings::load()` each time, `settings.rs:55-151`) unless `init_global()` loaded it into a process-global cache (used by long-running qterm, `figterm/src/main.rs:479`).
- Save (`lib.rs:274-307`): create parent dirs, open with `create|write|truncate`, unix mode `0o600`, exclusive file lock, `serde_json::to_writer_pretty` (2-space indent, keys in serde_json `Map` order = BTreeMap ⇒ **sorted**, inferred from serde_json default without `preserve_order`), if serialization fails write `{}`.
- Set = load → insert → save (read-modify-write, `settings.rs:55-68`). Remove = load → remove → save (`:70-83`).
- Typed getters: `get_bool` → `Value::as_bool` (so `"true"` string is NOT true), `get_string` → `as_str`, `get_int` → `as_i64`; `*_or(default)` swallow errors (`lib.rs:325-347`, `settings.rs:112-151`).
- Corruption: `q doctor` check "Amazon Q settings corruption" offers fix that overwrites file with `{}` (`doctor/mod.rs:568-589`). Global loader also rewrites `{}` on parse error (`lib.rs:242-272`).

### 4.3 State DB (`crates/fig_settings/src/sqlite/mod.rs`)

- Path `fig_data_dir()/data.sqlite3` (`:74-76`); chmod to `0o600` if not already (`:93-103`).
- Migrations `000`…`005` (`:59-66`); `state` table: `CREATE TABLE state (key TEXT PRIMARY KEY, value TEXT);` (`migrations/004_state_table.sql`). Also `history` table (command, shell, pid, session_id, cwd, start_time, end_time, duration, hostname, exit_code) used for history suggestions, and `auth_kv` (out of scope).
- Values are `serde_json::Value` via rusqlite `serde_json` feature v0.32.1 (`Cargo.toml:131`): get = `SELECT value FROM state WHERE key=?1` (`:143-155`), set = `INSERT OR REPLACE` (`:161-171`), delete = `DELETE` (`:177-186`), all = `SELECT key, value` → map (`:210-228`). (inferred) encoding: strings/bools/objects stored as JSON text, integers as INTEGER.
- (rec) tabsmith may replace SQLite with `state.json` (same flat-map format as settings) unless the history feature needs SQL; keep the `_ local-state` CLI identical.

### 4.4 Keybinding settings
- Keys `autocomplete.keybindings.<keystroke>` → value = action identifier (e.g. `"toggleDescription"` or dashboard form `"autocomplete.toggleDescription"`), or `"ignore"` to disable a default binding (`crates/fig_settings/src/keybindings.rs:66-90,114-140`; `packages/dashboard-app/src/components/ui/keystrokeInput.tsx:33-34`, `hooks/store/useKeybindings.ts:16-36`).
- Keystroke syntax: modifiers joined by `+`: `control+r`, `shift+tab`, `command+i`, `esc`, `enter`, `tab`, `up`, `down`, `control+1`… (`crates/fig_settings/src/actions.json`).
- Hard-coded default actions (`actions.json`, 28 actions; `availability` WHEN_FOCUSED|ALWAYS):

| identifier | default bindings |
|---|---|
| insertSelected | enter |
| insertCommonPrefix | tab |
| insertCommonPrefixOrNavigateDown, insertCommonPrefixOrInsertSelected, insertSelectedAndExecute, execute | – |
| hideAutocomplete (ALWAYS) | esc |
| showAutocomplete, toggleAutocomplete (ALWAYS) | – |
| navigateUp | shift+tab, up, control+k, control+p |
| navigateDown | down, control+j, control+n |
| selectSuggestion1…10 | control+1 … control+9, control+0 |
| hideDescription, showDescription | – |
| toggleDescription | command+i |
| toggleHistoryMode | control+r |
| toggleFuzzySearch | – |
| increaseSize | command+= |
| decreaseSize | command+- |

(Dashboard shows slightly different defaults: toggleDescription `control+k`, navigateUp without `control+k`, navigateDown without `control+j` — `packages/dashboard-app/src/data/keybindings.ts:67-73,154-169`.) Ctrl/Alt + letter bindings match both cases (`crates/figterm/src/interceptor.rs` `insert_binding`).

---

## 5. Settings key catalog (settings.json)

Sources: dashboard schema `packages/dashboard-app/src/data/{autocomplete.ts,inline.ts,preferences.tsx,integrations.ts}`, enum `packages/api-bindings-wrappers/src/settings.ts:1-51`, Rust usages (grep). "Rust default" is what code uses when key absent.

### 5.1 Autocomplete (in scope)

| key | type | default | description / usage |
|---|---|---|---|
| `autocomplete.disable` | bool | false | UI "Enable Autocomplete" (inverted). Doctor check fails if true (`doctor/mod.rs:1178`). `autocomplete.ts:9-17` |
| `autocomplete.insertSpaceAutomatically` | bool | true | insert space after a suggestion that takes a mandatory arg (`autocomplete.ts:19-28`; `autocomplete/src/state/insertion.ts:218` `?? true`) |
| `autocomplete.immediatelyExecuteAfterSpace` | bool | false | show `↪` "execute" suggestion when search term empty (`autocomplete.ts:30-37`; `suggestions/index.ts:411-420`) |
| `autocomplete.scrollWrapAround` | bool | false | down at end wraps to top (`autocomplete.ts:39-46`) |
| `autocomplete.alwaysSuggestCurrentToken` | bool | false | add typed token as first suggestion (`autocomplete.ts:48-55`) |
| `autocomplete.fuzzySearch` | bool | false | substring/fuzzy instead of prefix (`autocomplete.ts:57-64`) |
| `autocomplete.preferVerboseSuggestions` | bool | false | insert `--message` instead of `-m` (`autocomplete.ts:66-75`) |
| `autocomplete.onlyShowOnTab` | bool | false | only show on Tab; also read by PTY interceptor (`figterm/src/interceptor.rs:24`) and hides auto-execute suggestion (`suggestions/index.ts:400-402`) |
| `autocomplete.disableForCommands` | string[] ("textlist") | [] | CLIs to not complete, e.g. `["git","npm"]` (`autocomplete.ts:86-93`; `autocomplete-parser/src/loadSpec.ts:224`) |
| `autocomplete.sortMethod` | "most recent" \| "alphabetical" | "most recent" | (`autocomplete.ts:95-102`; `suggestions/sorting.ts:57`) |
| `autocomplete.scriptTimeout` | number (ms) | 5000 | generator script timeout (`autocomplete.ts:104-111`; `state/generators.ts:94`) |
| `autocomplete.immediatelyRunDangerousCommands` | bool | false | allow 1-press execute of `isDangerous` suggestions (`autocomplete.ts:113-121`) |
| `autocomplete.immediatelyRunGitAliases` | bool | true | (`autocomplete.ts:122-130`) |
| `autocomplete.firstTokenCompletion` | bool | false | complete the command name itself (`autocomplete.ts:132-140`; `parseArguments.ts:1102`) |
| `autocomplete.hideAutoExecuteSuggestion` | bool | false | (`autocomplete.ts:142-149`) |
| `autocomplete.theme` | string | dashboard "system"; CLI `theme` prints "dark" if unset (`theme.rs:25,117`) | one of system, dark, light, cobalt, cobalt2, dracula, github-dark, gruvbox, monokai-dark, nightowl, nord, panda, poimandres, the-unnamed, synthwave-84, solarized-light, solarized-dark, or a custom file name (`autocomplete.ts:157-181`) |
| `autocomplete.height` | number (px) | 140 | max menu height (`autocomplete.ts:183-188`; `autocomplete/src/window.ts:8,23-28`; `fig_desktop/src/webview/window.rs:183`) |
| `autocomplete.width` | number (px) | 320 (+50 when history mode is `history_only`/`show`) | (`window.ts:7,11-21`) |
| `autocomplete.fontFamily` | string\|null | null | |
| `autocomplete.fontSize` | number\|null | null → 12.8px | (`autocomplete/src/fig/themes.ts:325-331`) |
| `autocomplete.alwaysShowDescription` | bool | (false) | enum only (`settings.ts:30`) |
| `autocomplete.navigateToHistory` | bool | (false) | enum only (`settings.ts:28`) |
| `autocomplete.hidePreviewWindow` | bool | false | commented out in UI |
| `autocomplete.iconTheme` | string | – | Linux desktop icons (`fig_desktop/src/platform/linux/icons.rs:23`) |
| `autocomplete.developerMode` | bool | false | load specs from `devCompletionsFolder`, no caching; doctor warns `autocomplete.developerMode is enabled` (`doctor/mod.rs:1218-1223`) |
| `autocomplete.devCompletionsFolder` | string\|null | null | spec dir for dev mode (`loadSpec.ts:121`) |
| `autocomplete.developerModeNPM`, `autocomplete.developerModeNPMInvalidateCache`, `autocomplete.devCompletionsServerPort` | bool/bool/number | – | spec dev server (`settings.ts:3-7`) |
| `autocomplete.personalShortcutsToken` | string | "+" | (`loadSpec.ts:73`) |
| `autocomplete.history.disableLoading` | bool | – | (`settings.ts:40`) |
| `autocomplete.keybindings.<keystroke>` | string | – | see §4.4 |
| `beta.autocomplete.auto-cache` | bool | false | cache all generators (`generators/helpers.ts:37`) |
| `beta.history.mode` | "off"\|"history_only"\|"show" | – | (`settings.ts:41-43`) |
| `beta.history.customCommand`, `beta.history.allShells`, `beta.history.ctrl-r` | string/bool/bool | – | (`settings.ts:44-46`) |
| `developer.autocomplete.host` | string | – | doctor warns if contains "localhost" (`doctor/mod.rs:1893-1921`) |
| `developer.autocomplete.build` | null\|"beta"\|"develop" | null (=production) | set by `q debug build autocomplete …` (`debug/mod.rs:291-313`) |

### 5.2 Inline (in scope)

| key | type | default | notes |
|---|---|---|---|
| `inline.enabled` | bool | **true** | `q inline enable/disable`; read by `init` (zsh post plugin sourcing) and doctor (`inline.rs:30`, `init.rs:33,155`, `doctor/mod.rs:724,737`; UI `inline.ts:8-14` "This setting only applies in new shell sessions.") |

(rec) new tabsmith keys for the LLM source (no upstream equivalent): `inline.source` = `"local"|"openai"|"anthropic"` (default `local`), `inline.model`, `inline.baseUrl`, `inline.apiKeyEnv` (name of env var holding key; never store key in settings), `inline.debounceMs`, `inline.historyCount`. Upstream equivalents exist only as env vars: `Q_INLINE_SHELL_COMPLETION_DEBOUNCE_MS`, `Q_INLINE_SHELL_COMPLETION_HISTORY_COUNT`, `Q_INLINE_SHELL_COMPLETION_CACHE_DISABLE` (`fig_os_shim/src/env.rs:214-224`).

### 5.3 PTY / shell-integration (in scope)

| key | type | default | used |
|---|---|---|---|
| `qterm.path` | string | – | if set, `init` emits `Q_TERM_PATH="<path>"` (`init.rs:202-204`) |
| `qterm.csi-u.enabled` | bool | false | CSI-u key reporting in PTY (`figterm/src/event_handler.rs:49`) |
| `ssh.remote-prompt` | "ask"\|"never"\|… | "ask" | (`figterm/src/main.rs:245-248`) |
| `ssh.remote-prompt.timeout` | int ms | 2000 | (`figterm/src/main.rs:256`) |
| `integrations.{terminal,hyper,vscode,iterm}.disabled` | bool | false | desktop only (`dashboard-app/src/data/integrations.ts`) |

### 5.4 Other (out of scope; keep reserved names only)
`app.launchOnStartup`(true), `app.disableAutoupdates`(false), `app.disableAutolaunch`, `app.hideMenubarIcon`(false), `app.beta`(false), `app.theme`, `app.autoupdate.check-period`, `telemetry.enabled`(true), `codeWhisperer.shareCodeWhispererContentWithAWS`(true), `api.timeout`, `api.codewhisperer.service`, `api.q.service`, `ai.terminal-hash-sub`(true), `ai.menu-actions`, `developer.dashboard.host`, `developer.dashboard.build`, `install.releaseUrl`.

---

## 6. State key catalog (state DB; `q _ local-state`)

| key | type | default | written/read by |
|---|---|---|---|
| `shell-integrations.enabled` | bool | true | `init` prints "integration disabled" stub if false (`init.rs:34,127-129`) |
| `dotfiles.enabled` | bool | true | gate for zsh inline plugin in `init` (`init.rs:161`) |
| `shell-integrations.immediateLogin` | bool | false | `init post` emits `q login` once, resets (`init.rs:186-190`) |
| `prompt.inline.count` | int | 0 | one-time inline announcement in zsh post (`init.rs:270-279`) |
| `prompt.input-method.<terminal_id>.count` | int | 0 | macOS, shown < 2 times (`init.rs:254-268`) |
| `desktop.auth-watcher.logged-in`, `cli.init.login-prompt.sent-at` | bool/int | true/0 | login nag (disabled) (`init.rs:281-292`) |
| `input-method.enabled` | bool | – | set true by `integrations install input-method` (`integrations.rs:182`) |
| `doctor.prompt-restart-terminal` | bool | false | doctor prints restart PS, then clears (`doctor/mod.rs:2334-2342`) |
| `plugin.developerMode` | bool | – | doctor warning (`doctor/mod.rs:1225`) |
| `developer.logging` | bool | – | `debug logs` sets true; ctrl-c sets false (`debug/mod.rs:329,341`) |
| `qterm.enabled` | bool | true | PTY exits immediately with `[NOTE] qterm is disabled. Autocomplete will not work.` if false (`figterm/src/main.rs:982-986`) |
| `user.enabled-shells` | string[] | [] | PTY (`figterm/src/main.rs:152-157`) |
| `ssh.remote-prompt.disable-host.<host>` | bool | false | (`figterm/src/main.rs:250-251`) |
| `pty.suppress_log_error` | bool | false | (`figterm/src/main.rs:518`) |
| `api.selectedCustomization` | object `{arn,name?,description?}` (camelCase) | – | `inline set-customization` (`fig_api_client/src/customization.rs:9-35`) |
| `update.new-version-available` | string | – | (`fig_settings/src/keys.rs:1`) |

---

## 7. Environment variables (read or set by the CLI/integration)

| var | meaning | ref |
|---|---|---|
| `QTERM_SESSION_ID` | UUID of current PTY session; socket = `sockets_dir/t/<id>.sock` | `consts.rs:109`, `directories.rs:382-384` |
| `Q_TERM` | set by PTY to its version; doctor compares to CLI version | `consts.rs:121`, `doctor/mod.rs:690-710`, `figterm/src/main.rs:434` |
| `Q_TERM_TMUX` | PTY version when inside tmux | `figterm/src/main.rs:436` |
| `Q_SHELL` | shell exe path to spawn inside PTY (emitted by `init`) | `init.rs:133-144`, `figterm/src/main.rs:392` |
| `Q_IS_LOGIN_SHELL`, `Q_EXECUTION_STRING`, `Q_SHELL_EXTRA_ARGS`, `Q_START_TEXT` | passed by pre script to PTY, removed from child env | `figterm/src/main.rs:418-444` |
| `SHOULD_QTERM_LAUNCH` | 0/1/2 result emitted by `init pre` | `init.rs:146-153` |
| `Q_TERM_PATH` | override PTY binary path | `init.rs:202-204` |
| `Q_FORCE_FIGTERM_LAUNCH` | force launch (exit 0) | `should_figterm_launch.rs:12,214-219` |
| `Q_TERM_DISABLED` | never launch PTY | `should_figterm_launch.rs:13,221-226` |
| `Q_DISABLE_AUTOCOMPLETE` | non-empty ⇒ PTY doesn't do autocomplete | `figterm/src/main.rs:348-353` |
| `PROCESS_LAUNCHED_BY_Q` | process spawned by product ⇒ no PTY; doctor error | `consts.rs:130` |
| `Q_PARENT`, `Q_SET_PARENT`, `Q_SET_PARENT_CHECK` | SSH remote socket chaining | `consts.rs:112-118` |
| `Q_ZDOTDIR` | fallback ZDOTDIR when locating zsh dotfiles | `fig_util/src/shell.rs:109-116` |
| `Q_DEBUG_SHELL` | set by `debug shell` | `debug/mod.rs:821` |
| `Q_USING_ZSH_AUTOSUGGESTIONS` | exported by inline zsh plugin when zsh-autosuggestions present; doctor error | `inline_shell_completion/guard_start.zsh`, `doctor/mod.rs:747-766` |
| `Q_LOG_LEVEL`, `Q_LOG_STDOUT` | logging | `env.rs:174-180` |
| `Q_INIT_SNAPSHOT_TEST` | deterministic `init` output for tests (`Q_SHELL="/bin/zsh"`, status 0, skip inline plugin/prompt) | `init.rs:36,137-151,165,270` |
| `Q_CI`/`CI`, `Q_CODESPACES`/`CODESPACES`, `SSH_CLIENT`/`SSH_CONNECTION`/`SSH_TTY`, `Q_FAKE_IS_REMOTE` | environment detection | `env.rs:150-172` |
| `Q_INLINE_SHELL_COMPLETION_{CACHE_DISABLE,HISTORY_COUNT,DEBOUNCE_MS}` | inline tuning | `env.rs:214-224` |
| `Q_AUTOSUGGEST_STRATEGY`, `Q_AUTOSUGGEST_HIGHLIGHT_STYLE` (`fg=8`) and other `Q_AUTOSUGGEST_*` zsh vars | inline zsh plugin config | `inline_shell_completion/config.zsh:10-93` |
| `EDITOR` | `settings open` (non-macOS) | `settings.rs:97` |
| `ZDOTDIR`, `__fish_config_dir`, `XDG_*`, `TMPDIR`, `LOCALAPPDATA` | path resolution | `shell.rs:106-123`, `directories.rs` |
| `INSIDE_EMACS`, `TERM_PROGRAM=WarpTerminal`, `__CFBundleIdentifier=com.vandyke.SecureCRT`, `__PWSH_LOGIN_CHECKED` | unsupported environments (no PTY; doctor errors) | `should_figterm_launch.rs:235-269`, `doctor/mod.rs:600-655` |

Settings are **not** overridable by env vars in upstream (no `Q_SETTING_*` mechanism). (rec) Optional for tabsmith: `TABSMITH_SETTINGS_PATH`, `TABSMITH_DATA_DIR`.

---

## 8. `theme` (`crates/q_cli/src/cli/theme.rs`)

```rust
const BUILT_IN_THEMES: [&str; 3] = ["dark", "light", "system"];
const DEFAULT_THEME: &str = "dark";
#[derive(Args)] struct ThemeArgs {
    #[arg(long, conflicts_with_all = &["folder", "theme"])] list: bool,
    #[arg(long, conflicts_with_all = &["list", "theme"])]   folder: bool,
    #[arg(conflicts_with_all = &["list", "folder"])]          theme: Option<String>,
}
```
Theme dir = `themes_dir()` = `resources_path/themes` (`directories.rs:277-279`): macOS `<App>.app/Contents/Resources/themes`, Linux `/usr/share/amazon-q/themes` (AppImage: `$CWD/lib/q-desktop/themes`), Windows `%LOCALAPPDATA%\AmazonQ\resources\themes` (`directories.rs:403-420`). (rec) tabsmith: `fig_data_dir()/themes` (user-writable) plus embedded built-ins.

Behaviour (`theme.rs:51-128`):
1. `--folder`: print theme dir path; exit 0.
2. `--list`: `read_dir(theme_dir)?` (error if dir missing) and print each file name with trailing `.json` trimmed (order = OS dir order; built-ins NOT listed). (rec) tabsmith: list built-ins + files, sorted.
3. `<THEME>`: try reading `<theme_dir>/<THEME>.json`:
   - found → parse as `{author?:{name?,twitter?,github?}, version?}` (parse error ⇒ error exit 1); print `\n`, then `› Switching to theme '<THEME bold>'` + ` by <name bold>` if author.name; if twitter: `  🐦 <twitter in rgb(29,161,242)>`; if github: `  💻 <underlined github.com/<github>>`; then `\n`. Set `autocomplete.theme = "<THEME>"`.
   - not found and THEME ∈ {dark, light, system} → print `› Switching to theme '<THEME bold>'`, set setting.
   - else error `'<THEME>' does not exist in <theme_dir>` (exit 1).
4. no args: print `autocomplete.theme` value (string raw; non-string pretty JSON); unset → `dark`.

Theme JSON file format (`packages/autocomplete/src/fig/themes.ts:85-124`):
```json
{
  "author": { "name": "…", "twitter": "@…", "github": "user" },
  "version": "1.0",
  "theme": {
    "textColor": "rgb(180,180,180)", "backgroundColor": "rgb(48,48,48)", "matchBackgroundColor": "rgb(95,89,56)",
    "selection": { "textColor": "rgb(253,253,253)", "backgroundColor": "rgb(30,90,199)", "matchBackgroundColor": "rgb(106,142,218)" },
    "description": { "borderColor": "rgb(65,65,65)", "textColor": "rgb(180,180,180)" }
  },
  "shade0": "#181818", "...": "...", "shade7": "#f8f8f8", "accent0": "#ab4642", "...": "accent7"
}
```
- `version: "1.0"` + `theme` → explicit colors; shades 0/1/5/6/7 derived from main bg/description border/description text/main text/selected text; shades 2-4 and accents from file or filler (`themes.ts:193-220`).
- otherwise base16-style `shade0..7`, `accent0..7`; mapping: bg=shade0, match bg=shade1, desc border=shade1, selected bg=shade2, desc text=shade5, text=shade6, selected text=shade7, selected match bg=accent6 (`themes.ts:221-249`).
- Colors: `#RRGGBB`, `#RRGGBBAA` (alpha premultiplied into rgb), or `rgb(r,g,b)` without spaces (`themes.ts:130-159`).
- Built-in dark/light values `themes.ts:9-44`; fillers `themes.ts:46-83`; `system` → dark/light by OS appearance; any load error → dark (`themes.ts:282-317`); custom themes merged over dark (`themes.ts:309`).
(rec) tabsmith renders in a terminal: map RGB to truecolor SGR `38;2;r;g;b`/`48;2;…`, degrade to 256/16 colors when `COLORTERM` lacks truecolor.

---

## 9. `inline` (`crates/q_cli/src/cli/inline.rs`)

```rust
#[derive(Subcommand)] enum InlineSubcommand {
    /// Enables inline
    Enable,
    /// Disables inline
    Disable,
    /// Shows the status of inline
    Status,
    /// Select a customization if you have any available
    SetCustomization { /// The arn of the customization to use
                       arn: Option<String> },
    /// Show the available customizations
    ShowCustomizations { #[arg(long, short, value_enum, default_value_t)] format: OutputFormat },
}
```
| cmd | behaviour | stdout | exit |
|---|---|---|---|
| `enable` | `settings.set_value("inline.enabled", true)`; then best-effort IPC `InlineShellCompletionSetEnabled{enabled:true}` to `figterm_socket_path($QTERM_SESSION_ID)` (failure only logged) (`:58-64,152-163`) | `Inline enabled` (magenta) | 0 |
| `disable` | same with false | `Inline disabled` (magenta) | 0 |
| `status` | `get_bool("inline.enabled").unwrap_or(true)` | `Inline is <enabled\|disabled bold>` | 0 |
| `set-customization [ARN]` | fetch list via API; empty → print `No customizations found` exit 1; ARN given: not found → `Customization not found` exit 1; found → save state `api.selectedCustomization` = `{"arn","name","description"}`, print `Customization <name bold> selected`. No ARN: interactive `dialoguer::Select` "Select a customization", items `"<name bold> - <description>"` + `None` (bold); pick None → remove state key, print `Customization unset`; Esc → error `No customization selected` (`:76-125`). Non-TTY `choose` returns index 0 (`util/mod.rs:257-265`) | | 0/1 |
| `show-customizations [-f F]` | Plain: `No customizations found` or for each: `<name bold>\n` + description (if any) + `\n`; Json: array of `{arn, name?, description?}` (`:126-146`) | | 0 |

The PTY keeps an in-memory `INLINE_ENABLED` flag (default true) toggled by that IPC (`figterm/src/inline/mod.rs:58,170,377-378`); remote sessions reject it (`figterm/src/message.rs:292-293`). Setting persists for new shells.
(rec) tabsmith mapping: "customizations" → LLM provider profiles. `set-customization <name>` selects a profile from settings (`inline.profiles.<name>.*`), `show-customizations` lists them. Same output strings/format.

---

## 10. `integrations` (`crates/q_cli/src/cli/integrations.rs`)

### 10.1 Clap (`integrations.rs:17-70`)
```rust
enum IntegrationsSubcommands {
    Install   { #[command(subcommand)] integration: Integration, /// Suppress status messages
                #[arg(long, short)] silent: bool },
    Uninstall { …same… }, Reinstall { …same… },
    Status    { #[command(subcommand)] integration: Integration,
                #[arg(long, short, value_enum, default_value_t)] format: OutputFormat },
}
#[non_exhaustive] enum Integration {
    Dotfiles { /// Only install the integrations for a single shell
               #[arg(value_enum)] shell: Option<Shell> },   // bash|zsh|fish|nu
    Ssh, InputMethod,
    #[command(name = "vscode")] VSCode,
    #[command(alias = "jetbrains-plugin")] IntellijPlugin,
    AutostartEntry, GnomeShellExtension,
    #[doc(hidden)] All,
}
```
CLI names: `dotfiles [bash|zsh|fish|nu]`, `ssh`, `input-method`, `vscode`, `intellij-plugin`/`jetbrains-plugin`, `autostart-entry`, `gnome-shell-extension`, `all` (hidden). `--silent/-s` and `--format/-f` belong to the parent, so they go **before** the integration name: `q integrations uninstall --silent all` (as used in `internal/mod.rs:519-529`) (inferred clap semantics).

### 10.2 Behaviour
- `install X` (`:122-250`):
  - `all` → dotfiles(all shells) + ssh (+ input-method on macOS).
  - `dotfiles [S]`: shells = [S] or [bash, zsh, fish]; for each integration of each shell: if `is_installed()` Ok → skip; else `installed=true; install()`; errors collected as `"<describe bold>: <verbose_message>"`, final error `"\n\n" + errs.join("\n\n")`. Shell enumeration error → `"<shell>: <err>"`.
  - `ssh`: install if not installed.
  - `input-method` (macOS only, else error `Input method integration is only supported on macOS`), `vscode` (macOS), `intellij-plugin` (macOS), `autostart-entry` → error `Installing the autostart entry from the CLI is not supported`, `gnome-shell-extension` → error `Installing the GNOME Shell extension from the CLI is not supported`.
  - Output: if something installed and Ok and !silent: `Installed!` (+ status line for input method: `You must restart your terminal to finish installing the input method.`). If nothing installed, no error, !silent: `Already installed`.
- `uninstall X` (`:252-390`): `all` → dotfiles + ssh (+ input-method macOS; + autostart-entry & gnome ext on Linux). Dotfiles: uninstall each integration whose `is_installed()` is Ok. Output `Uninstalled!` / `Not installed` (unless silent). VSCode/IntelliJ print `Warning: … integrations are automatically reinstalled on launch`.
- `reinstall X` (`:102-117`): uninstall then install (for `all`: uninstall dotfiles, ssh, [im]; install dotfiles, ssh, [im]).
- `status X` (`:392-548`):
  - `all` → error `Checking the status for all integrations is currently not supported`.
  - `ssh`/`input-method`: Plain `Installed`/`Not installed`; Json `{"installed": bool}`.
  - `dotfiles [S]` (S is **ignored**; always bash, zsh, fish): Plain lines `✔ <describe>` (green ✔) or `✘ <describe>` (red ✘), then `"<shell>: <error>"` lines; Json:
    ```json
    {"integrations":[{"installed":true,"description":"zsh pre post into .zshrc","shell":"zsh","file_name":".zshrc"}],
     "errors":[{"shell":"fish","error":{"title":"…","message":null}}]}
    ```
    (`error` is the serialized `VerboseMessage {title, message}`, `error.rs:65-70`.)
  - `vscode`/`intellij-plugin`: `<App>: Installed|Not installed` per variant; others error.
- All return exit 0 on success; `Err` → exit 1.

### 10.3 Shell integration model (`crates/fig_integrations/src/shell/mod.rs`)

Which dotfiles (`get_shell_integrations`, `:132-191`), config dir from `Shell::get_config_directory` (`fig_util/src/shell.rs:106-123`):
- **bash**: dir `$HOME`; always `.bashrc`; plus each of `.profile`, `.bash_login`, `.bash_profile` that exists; if none exist, `.profile`. Each is a `DotfileShellIntegration{pre:true, post:true}`.
- **zsh**: dir `$ZDOTDIR` → `$Q_ZDOTDIR` → `$HOME`; `.zshrc`, `.zprofile`.
- **fish**: dir `$__fish_config_dir` or `~/.config/fish`; files `conf.d/00_fig_pre.fish` (Pre) and `conf.d/99_fig_post.fish` (Post) as `ShellScriptShellIntegration` (whole file owned by us).
- **nu**: none (but `init nu` still exists).

Script files for bash/zsh dotfiles: `fig_data_dir()/shell/<dotfile-without-leading-dot>.<pre|post>.<shell>` e.g. `~/.local/share/amazon-q/shell/zshrc.pre.zsh`, `bash_profile.post.bash` (`:93-98,403-415`).

Script file **contents** (`ShellScriptShellIntegration::get_contents`, `:293-331`); `rcfile` = file name up to first `.` (`get_prefix`, `:271-276`), e.g. `zshrc`, `00_fig_pre`:
- macOS:
  - bash/zsh: `[ -x ~/.local/bin/q ] && eval "$(~/.local/bin/q init zsh pre --rcfile zshrc)"` (no trailing newline)
  - fish: `test -x ~/.local/bin/q; and eval (~/.local/bin/q init fish pre --rcfile 00_fig_pre | string split0)`
- Linux/other:
  ```sh
  _Q_LOCAL_BIN="$HOME/.local/bin"
  [[ ":$PATH:" != *":$_Q_LOCAL_BIN:"* ]] && PATH="${PATH:+"$PATH:"}$_Q_LOCAL_BIN"
  unset _Q_LOCAL_BIN

  [ -n "$BASH_VERSION" ] && command -v q >/dev/null 2>&1 && eval "$(q init bash pre --rcfile bashrc)"
  ```
  (the `[ -n "$BASH_VERSION" ] && ` prefix only for bash; zsh omits it). Fish: `contains $HOME/.local/bin $PATH; or set -a PATH $HOME/.local/bin\ncommand -qv q; and eval (q init fish pre --rcfile 00_fig_pre | string split0)\n`.
  Note the indoc block ends with `\n`, then `format!("{add_to_path_line}\n{source_line}\n")` ⇒ blank line between.

Dotfile block text (`:418-423,512-521`):
```
# Amazon Q pre block. Keep at the top of this file.
[[ -f "${HOME}/.local/share/amazon-q/shell/zshrc.pre.zsh" ]] && builtin source "${HOME}/.local/share/amazon-q/shell/zshrc.pre.zsh"
...user content...
# Amazon Q post block. Keep at the bottom of this file.
[[ -f "${HOME}/.local/share/amazon-q/shell/zshrc.post.zsh" ]] && builtin source "${HOME}/.local/share/amazon-q/shell/zshrc.post.zsh"
```
(fish form, unused since fish uses conf.d: `test -f "<p>"; and builtin source "<p>"`). Path = script path with `$HOME` prefix replaced by `${HOME}` (macOS: `${HOME}/Library/Application Support/amazon-q/shell/…`, quoted so spaces are fine).

Install algorithm (`install`/`install_inner`, `:590-650`): if already installed → no-op. If dotfile exists: **backup** to `~/.amazon-q.dotfiles.bak/<YYYY-MM-DD_HH-MM-SS UTC>/<name>` (`backup.rs:8-21`, `directories.rs:310-323`), run `uninstall()` (strip old blocks), re-read. Pre: write script file; `contents = shebang + desc_pre + "\n" + source_pre + "\n" + rest` (shebang line `#!…\n` kept first, `split_shebang` `:759-768`). Post: write script file; `contents = contents + "\n" + desc_post + "\n" + source_post + "\n"`. Write dotfile only if changed (`File::create`, permission error → `PermissionDenied{path}`).

Uninstall (`:652-689`): read dotfile; regex-remove lines matching `(?mi)^#.*fig.*var.*$\n?` and `(?mi)^#.*Please make sure this block is at the .* of this file.*$\n?`; remove current source regex and all legacy regexes (`remove_from_text`, `:542-554`); `trim()` + `"\n"`; write. Then delete both script files (missing = ok).

`is_installed` (`:691-729`), returns first failing reason:
1. dotfile missing → `FileDoesNotExist(path)` ("File does not exist: <path>").
2. any line matching `(?mi)^\s*#\s*fig ignore\s?.*$` → treated as installed (escape hatch).
3. strip comments & blank lines: `(?m)^\s*(#.*)?\n` → "", then `trim()`.
4. legacy pattern present → `LegacyInstallation("<dotfile> has legacy <when> integration.")`. Legacy regexes (`:433-510`): `[ -s ~/.fig/shell/pre.sh ] && source ~/.fig/shell/pre.sh`, `[ -s ~/.fig/fig.sh ] && source ~/.fig/fig.sh`, old `eval "$(q init <shell> <when>)"` (pre variant preceded by `export PATH="${PATH}:${HOME}/.local/bin"`), `. "$HOME/<path>"`, `[[ -f "$HOME/<path>" ]] && . "$HOME/<path>"`, old CodeWhisperer brand `[[ -f "${HOME}/Library/Application Support/codewhisperer/shell/…" ]] && builtin source …`. (rec) tabsmith has no legacy; only detect its own blocks.
5. source line absent → `NotInstalled("<dotfile> does not source <when> integration")`.
6. position: pre regex anchored with `^` (start of filtered text), post with `$` (end) → else `ImproperInstallation("<dotfile> does not source <when> integration first|last")`.
7. script file must exist with byte-identical contents → `FileDoesNotExist` / `ImproperInstallation("<path> should contain:\n<contents>")` (`file.rs:33-75`). Also on unix walks up parents checking `access(R|W|X)`; failure → `PermissionDenied` (`file.rs:35-61`).

`describe()` strings: dotfile `"<shell> pre post into <dotfile_name>"` e.g. `bash pre post into .bashrc` (`:634-642`); script `"<shell> <when>"` e.g. `fish pre` (`:348-350`). `Display` for integration: `"<shell> (<path>)"` (`:238-242`).

Error display strings (`crates/fig_integrations/src/error.rs:17-63`): `Legacy integration: {0}`, `Improper integration installation: {0}`, `Integration not installed: {0}`, `File does not exist: {path}`, `Permission denied: {path}`. `verbose_message()` for PermissionDenied: title `Permissions denied for <path bold>`, message `To automatically fix the permissions run: sudo q debug fix-permissions\n\n  Error: <err red>` (`error.rs:84-105`).

### 10.4 SSH integration (`crates/fig_integrations/src/ssh/mod.rs`, unix-only)
- Writes `fig_data_dir()/ssh` (mode 0600):
  ```
  Match exec "command -v q && q internal generate-ssh --remote-host %h --remote-port %p --remote-username %r"
      Include "<data_dir>/ssh_inner"
  ```
- Appends to `~/.ssh/config` (creating `~/.ssh` 0700; backup first): `# Amazon Q SSH Integration. Keep at the bottom of this file.\nMatch all\n  Include "~/.local/share/amazon-q/ssh"\n` (`:74-86,140-178`).
- `generate-ssh` writes `ssh_inner` with `RemoteForward '/tmp/q-parent-<uuid>.socket' '<remote.sock>'`, `SetEnv Q_SET_PARENT=…`, `StreamLocalBindMask 600`, `StreamLocalBindUnlink yes`, `PermitLocalCommand yes`, `LocalCommand <exe> _ ssh-local-command '%r@%n' '<uuid>' 1>&2`; skipped (header only) for usernames `git`, `aur` or when `remote.sock` not connectable (`internal/generate_ssh.rs:16-110`).
- (rec) tabsmith: SSH forwarding only matters when the completion engine runs on the client side; since tabsmith runs fully inside the remote shell's PTY wrapper, the SSH integration can be omitted (inferred) — keep `integrations install ssh` returning an explanatory error or "not supported".

### 10.5 Other integrations
input-method (macOS IME), vscode, intellij, autostart-entry, gnome-shell-extension: desktop-only → out of scope. (rec) keep the names in the enum and return `"<X> integration is not supported by tabsmith"` errors, or drop.

---

## 11. `init` (`crates/q_cli/src/cli/init.rs`)

```rust
#[derive(Args)] struct InitArgs {
    /// The shell to generate the dotfiles for
    #[arg(value_enum)] shell: Shell,   // bash | zsh | fish | nu
    /// When to generate the dotfiles for
    #[arg(value_enum)] when: When,     // pre | post   (fig_integrations/src/shell/mod.rs:71-91)
    #[arg(long)] rcfile: Option<String>,
}
```
Always exits 0; prints the generated source to stdout followed by `\n`; on internal error prints `# Could not load source: <err>` (`init.rs:50-60`).

Generation algorithm `shell_init` (`init.rs:121-295`), parts joined by `"\n"`:
1. Linux and `--rcfile profile` → empty output (`:122-125`).
2. state `shell-integrations.enabled` false → only `shell_integrations_disabled_code` (`:127-129,297-308`): guarded `printf 'Amazon Q shell integration is disabled.\nRe-enable by running: <magenta q _ local-state -d shell-integrations.enabled>\n'`.
3. If parent process exe known: `Q_SHELL="<parent exe path>"` (not exported) (`:133-144`).
4. `pre`: `SHOULD_QTERM_LAUNCH="<0|1|2>"` = `should_figterm_launch_exit_status(quiet=true)` (§12) (`:146-153`).
5. `post` and shell==zsh and rcfile ∉ {zprofile} and state `dotfiles.enabled`(true) and setting `inline.enabled`(true): append the inline zsh plugin (zsh-autosuggestions fork) guarded by `Q_DOTFILES_SOURCED` (`:157-174`). (Bash rcfile exclusion `profile|bash_profile` is irrelevant because shell must be zsh.)
6. `post`: if state `shell-integrations.immediateLogin` → reset false and emit `q login` (`:186-190`).
7. `pre` + bash + JetBrains terminal → `Q_IS_LOGIN_SHELL=1` (`:195-200`).
8. setting `qterm.path` → `Q_TERM_PATH="<path>"` (`:202-204`).
9. The shell integration script for (shell, when) with `{{CLI_BINARY_NAME}}`/`{{PTY_BINARY_NAME}}` substituted (`fig_integrations/src/shell/mod.rs:193-228`): zsh pre = `scripts/pre.sh`; zsh post = `scripts/post.zsh`; bash pre = bash-preexec wrapped in `function __fig_source_bash_preexec() {…}` + call + `function __bp_adjust_histcontrol() { :; }` + `scripts/pre.sh`; bash post = same preexec wrapper + `scripts/post.bash`; fish = `scripts/pre.fish`/`post.fish`; nu = `pre.nu`/`post.nu`. (Contents covered by the shell-integration report.)
10. `pre` + JetBrains: guarded (`Q_JETBRAINS_SHELL_INTEGRATION`, assignment *before* sourcing) source of the IDE's own shell integration files (`:209-252`).
11. macOS `post`: input-method prompt ≤2 times per terminal (`:254-268`).
12. `post` + zsh + inline enabled: one-time announcement (state `prompt.inline.count` < 1) guarded by `Q_INLINE_PROMPT`: `printf '\nAmazon Q now supports AI-powered inline completions!\n\nTo disable run: <magenta q inline disable>\n\n'` (`:270-279,325-336`).

Helpers:
- `assign_shell_variable` (`:68-76`): bash/zsh `NAME="v"` / `export NAME="v"`; fish `set -g NAME "v"` / `set -gx NAME "v"`; nu `let-env NAME = "v";`.
- `guard_source(shell, export, GUARD, assignment, source)` (`:78-119`): lines =
  bash/zsh `if [ -z "${GUARD}" ]; then` | fish `if test -z "$GUARD"` | nu `if env | any name == 'GUARD' {`; then each source line indented 2 spaces; the guard assignment `  GUARD="1"` before (BeforeSourcing) or after (AfterSourcing) the source; closer `fi\n` | `end\n` | `}`; joined by `\n`.
- Shell dotfile calls: `eval "$(q init zsh pre --rcfile zshrc)"` etc. (§10.3). `debug shell` uses `eval "$(q init bash post)"` (no rcfile) (`debug/mod.rs:827`).
- Snapshot tests: `Q_INIT_SNAPSHOT_TEST=1 q init <shell> <when> --rcfile <file>` for bash{bashrc,bash_profile}, zsh{zshrc,zprofile}, fish{00_fig_pre,99_fig_post}; output linted by `shellcheck -s bash -` / `fish --no-execute` (`crates/q_cli/tests/init.rs:20-105`, snapshots in `tests/snapshots/`).

---

## 12. `_ should-figterm-launch` (`crates/q_cli/src/cli/internal/should_figterm_launch.rs`)

Exit codes (doc `internal/mod.rs:191-197`): **0** launch PTY, **1** don't, **2** fall back to `Q_TERM`-based check in the shell script. Prints reason lines unless quiet (`init` uses quiet).

Order (`:206-295`), first match wins:
1. `Q_FORCE_FIGTERM_LAUNCH` set → `✅ Q_FORCE_FIGTERM_LAUNCH`, 0.
2. `Q_TERM_DISABLED` → `❌ Q_TERM_DISABLED`, 1.
3. `PROCESS_LAUNCHED_BY_Q` → `❌ PROCESS_LAUNCHED_BY_Q`, 1.
4. `INSIDE_EMACS` → `❌ INSIDE_EMACS`, 1.
5. `TERM_PROGRAM == WarpTerminal` → `❌ TERM_PROGRAM = WarpTerminal`, 1.
6. `__CFBundleIdentifier == com.vandyke.SecureCRT` → 1.
7. `__PWSH_LOGIN_CHECKED` → 1.
8. in CI (`CI` or `Q_CI`) → `❌ In CI`, 1.
9. in SSH and no `Q_PARENT` → `❌ In SSH without Q_PARENT`, 1. (rec) tabsmith: allow SSH (runs locally on remote host).
10. WSL → `🟡 Falling back to old mechanism since in WSL`, 2.
11. `should_launch` (`:165-203`): parent process (`parent_status`, `:56-98`): no pid/exe/name → DontLaunch; in SSH && `Q_TERM` unset → `✅ In SSH and Q_TERM is not set` 0; in Codespaces → 1 if `Q_TERM` set else 0; parent valid iff exe file name ∈ {zsh, bash, fish, nu}. Grandparent (`:100-162`): valid iff exe name matches a known terminal's executable names (MACOS_TERMINALS / LINUX_TERMINALS + SPECIAL_TERMINALS, Linux also by cmdline), `is_special` from terminal table. Prints `✅|❌ <gp exe> | <gp cmdline first word basename> (<pid>) <- ✅|❌ <parent exe> (<pid>)`. macOS and grandparent not special → `🟡 Falling back to old mechanism since on macOS`, 2. Else `0` iff both valid, else `1`.
12. Windows: 2 if stdin is a console (`GetConsoleMode` ok) else 1 (`:302-311`); other OS: 2.

(rec) tabsmith: simplify to env checks 1-8 + "parent is a supported shell" + "stdin/stdout are TTYs"; return 0/1 (keep 2 semantics for compatibility).

---

## 13. `doctor` (`crates/q_cli/src/cli/doctor/`)

### 13.1 Clap (`mod.rs:122-130`)
```rust
#[derive(Args)] struct DoctorArgs {
    /// Run all doctor tests, with no fixes
    #[arg(long, short = 'a')] pub all: bool,
    /// Error on warnings
    #[arg(long, short = 's')] pub strict: bool,
}
```
No `--format` flag.

### 13.2 Flow (`doctor_cli`, `mod.rs:2079-2345`)
1. Unix root (euid 0): stderr `Running doctor as root is not supported.` (red bold); unless `--all`, also `If you know what you're doing, run the command again with --all.` and exit **1**.
2. If not `--all`: spinner (`spinners::Dots`, text `Running checks...`), hide cursor, Ctrl-C handler (show cursor, exit 1).
3. Delete `fig_data_dir()/update.lock` if present.
4. Run groups in order (each group = `run_checks_with_context(header, checks, context_fn, config)`):

| # | header (printed dark-grey only with `--all`) | checks | context |
|---|---|---|---|
| 1 | `Let's check if you're logged in...` | LoginStatusCheck | () |
| – | (then best-effort launch desktop app) | | |
| 2 | `Let's check your dotfiles...` | one DotfileCheck per shell integration of bash, zsh, fish | current shell |
| 3 | `Let's make sure Amazon Q is set up correctly...` | FigBinCheck, LocalBinPathCheck (unix), WindowsConsoleCheck (windows), SettingsCorruptionCheck, SshdConfigCheck, FigIntegrationsCheck | () |
| 4 | `Let's make sure the app is running...` (full build only) | AppRunningCheck, DesktopSocketCheck | () |
| 5 | `Let's see if the app is in a working state...` | PtySocketCheck (unix), AutocompleteDevModeCheck, PluginDevModeCheck, DashboardHostCheck, AutocompleteHostCheck, MidwayCheck, InlineCheck | () |
| 6 | `Let's check if your system is compatible...` (result ignored via `.ok()`) | SystemVersionCheck, BashVersionCheck, FishVersionCheck, ToolboxInstalledCheck (macOS) | () |
| – | minimal build → stop here | | |
| 7 | macOS: `Let's check q diagnostic...` | ShellCompatibilityCheck, BundlePathCheck, AutocompleteEnabledCheck, CliPathCheck, AccessibilityCheck, DotfilesSymlinkedCheck | desktop diagnostics |
| 8 | Linux full & not remote: `Let's check Linux integrations` | DisplayServerCheck, IBusEnvCheck, GnomeExtensionCheck, IBusRunningCheck, IBusConnectionCheck, SandboxCheck | linux ctx |
| 9 | Linux full & not remote: `Let's check q diagnostic...` | AutocompleteActiveCheck | desktop diagnostics |
| 10 | `Let's check your terminal integrations...` | SupportedTerminalCheck, ItermBashIntegrationCheck, ImeStatusCheck (macOS) | terminal ctx |

5. Per check (`run_checks_with_context`, `mod.rs:1944-2032`):
   - `get_type(ctx, platform)` → `NormalCheck | SoftCheck | NoCheck`; NoCheck ⇒ skip.
   - run `check(ctx)` → `Ok | Warning(msg) | Error{reason, info[], fix?, error?}`.
   - if `!strict` and SoftCheck: `Error` downgraded to `Warning(reason)`.
   - print result if `--all` or result is not Ok (stops spinner first).
   - `--all`: never fixes, never aborts; continue.
   - otherwise on `Error`: if `fix`: print `Attempting to fix automatically...`; run fix; on failure `Failed to fix: <err>`; on success `Re-running check...` + blank, re-fetch context, re-check, print result; if no longer `Error` → continue to next check. Then print blank line and **abort** (bail with the error) — this skips all remaining groups. Warnings never abort.
6. Output symbols (`print_status_result`, `mod.rs:317-346`):
   - Ok: `✔ <name>` (✔ green)
   - Warning: `● <msg>` (● yellow; the check name is NOT printed)
   - Error: `✘ <name>: <reason>` (✘ red), then if info non-empty: blank line + `  <info line>` each; with `--all` also `  <error:?>`.
7. Footer: on error: blank, `✘ Doctor found errors. Please fix them and try again.`, blank, `If you are not sure how to fix it, please open an issue with <magenta q issue> to let us know!`, blank. On success: (if not `--all`) `✔ Everything looks good!`; then blank, `  Amazon Q still not working? Run <magenta q issue> to let us know!`, blank. Then optional restart PS (state `doctor.prompt-restart-terminal`): `  PS. Autocomplete won't work in any existing terminal sessions, ` (bold) + `only new ones.` (bold italic), `  (You might want to restart your terminal emulator)`, and reset the flag.
8. **Exit code is 0 even when doctor found errors** (only root-refusal returns 1). (rec) tabsmith: exit 1 on errors (and with `--strict` on warnings) — deviation, document it.

### 13.3 Every check (name → type → logic → messages/fix)

| Check | name | type | logic / messages | fix |
|---|---|---|---|---|
| LoginStatusCheck `:1846-1863` | `Auth` | Normal | not logged in & not CloudShell → `Not authenticated. Please run q login` | – (drop in tabsmith) |
| DotfileCheck `:943-1044` | `<shell> <~/short path> integration check` e.g. `zsh ~/.zshrc integration check` | Normal if integration's shell == current shell (parent process); Soft if that shell exe is in PATH; else NoCheck | `is_installed()`: Legacy/NotInstalled/Improper → reason=msg, info `Run q integrations install dotfiles to reinstall shell integrations for <shell>`; FileDoesNotExist → reason=`File does not exist: …`; PermissionDenied → reason=title, info [message, ""] no fix; other → reason=err | async: `integration.install()` |
| FigBinCheck `:398-410` | `Amazon Q data dir exists` | Normal | `fig_data_dir()` must exist → `No file at path <p>` | – |
| LocalBinPathCheck `:412-434` (unix) | `PATH contains ~/.local/bin` | Normal | `$PATH` contains substring `.local/bin` → else `Path does not contain ~/.local/bin` | – |
| WindowsConsoleCheck `:1799-1844` | `Windows Console Check` | Normal | windows: `GetConsoleMode` on stdin & stdout must succeed, else `Windows Console APIs are not supported in this terminal` + 8 info lines (MinTTY/Git Bash/winpty advice) | – |
| SettingsCorruptionCheck `:568-589` | `Amazon Q settings corruption` | Normal | `OldSettings::load()` fails → `Amazon Q settings file is corrupted` | write `{}` to settings.json |
| SshdConfigCheck `checks/sshd_config.rs` | `sshd config` | Normal only when in SSH, else NoCheck | read `/etc/ssh/sshd_config`; unreadable & `Q_PARENT` set → Ok for amzn users else Warning `Could not read sshd_config, check <wiki> for more info`; unreadable otherwise → Error(err, info); regexes `(?m)^\s*AcceptEnv\s+.*(Q_\*\|Q_SET_PARENT)([^\S\r\n]+.*$\|$)` AND `(?m)^\s*AllowStreamLocalForwarding\s+yes([^\S\r\n]+.*$\|$)` must match else `SSHD config is not set up correctly` + info (AcceptEnv/AllowStreamLocalForwarding lines, restart sshd steps) | – (drop/optional) |
| FigIntegrationsCheck `:591-712` | `Amazon Q terminal integrations` | Normal | errors in order: `TERM_PROGRAM=WarpTerminal` → `WarpTerminal is not supported`; windows parent `cmd.exe` → `CMD isn't supported yet, please use Git Bash or WSL instead`; `powershell.exe` → `Powershell isn't supported yet, …`; `__PWSH_LOGIN_CHECKED` → `Powershell is not supported`; `INSIDE_EMACS` → `Emacs is not supported`; SecureCRT → `SecureCRT is not supported`; `PROCESS_LAUNCHED_BY_Q` → `Amazon Q can not run in a process it launched`; in CI → `Doctor can not run in CI`; then `Q_TERM`: == CLI version → Ok; either version ends with `-dev` → Warning `qterm is running with a different version than Amazon Q CLI, it looks like you are running a development version of Amazon Q however`; other value → `This terminal is not running with the latest integration, please restart your terminal` info `Q_TERM=<v>`; unset → `qterm is not running in this terminal, please try restarting your terminal` | – |
| AppRunningCheck / DesktopSocketCheck `:436-509` | `Amazon Q is running` / `Amazon Q app socket exists` | macOS only | desktop app | out of scope |
| PtySocketCheck `:772-941` (unix) | `Qterm Socket Check` | Normal | (1) `QTERM_SESSION_ID` unset → `Qterm is not running, please restart your terminal. QTERM_SESSION_ID is unset.`; (2) validate socket perms (`Failed to validate socket permissions: …`, info `Socket path: …`); (3) socket file missing → `Tried to find the socket file, but it wasn't there.` + info (`Amazon Q uses the /tmp directory for sockets.`, `Did you delete files in /tmp? …`, `Try making a new tab or window in your terminal, then run q doctor again.`, `No file at path: …`); (4) connect with 2s timeout else `Socket exists but could not connect: <err>`; (5) **round-trip test**: enable raw mode; after 0.2s send `InsertText{insertion:"Testing qterm...\n", immediate:true, insert_during_command:true}`; read a line from stdin with 1.2s timeout; trimmed line must equal `Testing qterm...` else Warning `qterm socket did not read buffer correctly, don't press any keys while the checks are running: <buf:?>`; timeout → `qterm socket write timed out after 1s`; disable raw mode; (6) `Diagnostics` request (1s): zsh autosuggest style fg indexed color 15 → Warning `ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE is set to the same style as your text, this must be different to detect what you've typed`; fish suggestion fg==15 → Warning `The Fish suggestion color is set to the same style your text, …`; bad response/EOF/error → errors `Failed to receive expected message from qterm` / `Received EOF when trying to receive qterm diagnostics` / `Failed to receive qterm diagnostics: <e>` | – |
| AutocompleteDevModeCheck `:1218-1223` | `Autocomplete dev mode` | Normal | setting `autocomplete.developerMode` == true → Warning `autocomplete.developerMode is enabled` | – |
| PluginDevModeCheck `:1225` | `Plugin dev mode` | Normal | state `plugin.developerMode` true → Warning | – |
| DashboardHostCheck `:1865-1891` | `Dashboard is loading from the correct URL` | Normal | out of scope | |
| AutocompleteHostCheck `:1893-1921` | `Autocomplete is loading from the correct URL` | Normal | `developer.autocomplete.host` contains `localhost` → Warning `developer.autocomplete.host = <h>, delete this setting if Autocomplete fails to load` | – |
| MidwayCheck `checks/midway.rs` | `Midway Auth` | Amazon-internal | drop | |
| InlineCheck `:714-770` | `Inline` | NoCheck if current shell ≠ zsh; Normal if zsh & inline enabled; Soft if zsh & disabled | disabled → Warning `Inline is disabled, to re-enable run: q inline enable`; `Q_USING_ZSH_AUTOSUGGESTIONS` set → Error `Using zsh-autosuggestions is not supported at the same time as Inline` info [`To fix either:`, `- Remove zsh-autosuggestions from ~/.zshrc and restart your terminal`, `- Disable Inline by running: q inline disable`] | – |
| SystemVersionCheck `:1573-1596` | `OS is supported` | Normal | OS support level: Supported → Ok; SupportedWithCaveat{info} → Warning(info); InDevelopment → Warning `Support for <os> is in development. It may not work properly on your system.\n<info>`; Unsupported → `<os> is not supported` | – |
| BashVersionCheck `checks/bash_version.rs` | `Bash is up to date` | Soft if current shell is bash else NoCheck | `bash --version` regex `GNU bash, version (\d+\.\d+\.\d+)` must satisfy `>=5.0.0` else Warning `Using Bash <v> may cause issues, it is recommended to either update to bash >=5 or switch to zsh.\n  - Install Bash 5 with Brew: brew install bash && bash\n  - Change shell default to ZSH: chsh -s /bin/zsh && zsh` | – |
| FishVersionCheck `checks/fish_version.rs` | `Fish is up to date` | Normal (skips silently if `fish` not in PATH) | `fish --version`, keep digits and dots, semver `>=3.3.0` else Error `your fish version is outdated (need at least 3.3.0, found <v>)` | – |
| ToolboxInstalledCheck `:1923-1942` | `Jetbrains Toolbox Check` | macOS | effectively always Ok (warning constructed but discarded — bug) | – |
| ShellCompatibilityCheck `:1063-1100` (macOS) | `Compatible shell` | Normal | parent exe must match `(bash\|fish\|zsh\|nu)` → `Current shell <s> incompatible`; `dscl . -read /Users/<user> UserShell` must match → `Default shell <s> incompatible`; no parent → `Could not get current shell`; dscl error → Warning `Could not get default shell` | – |
| AutocompleteEnabledCheck `:1169-1195` (macOS) | `Autocomplete is enabled` | Normal | setting `autocomplete.disable` true → Error `Autocomplete disabled.` info `To fix run: q settings autocomplete.disable false` | – |
| CliPathCheck `:1227-1276` (macOS) | `Valid CLI path` | Normal | exe ends with old name `cw` → Warning `The cw CLI has been replaced with q`; exe ∈ {`~/.local/bin/q`, `/usr/local/bin/q`, `/opt/homebrew/bin/q`} → Ok; ends with `target/{debug,release}/{q,q_cli}` → Warning `Running debug build in a non-standard location: <p>`; else Error `CLI (<p>) must be in <~/.local/bin/q>` | – |
| BundlePathCheck, AccessibilityCheck, DotfilesSymlinkedCheck, AutocompleteActiveCheck, ImeStatusCheck, Linux IBus/GNOME/Display/Sandbox checks, Hyper/VSCode/iTerm checks | desktop/IME | – | out of scope (the Linux `SandboxCheck`: Flatpak/Snap/non-docker container → error; Docker → warning — could be kept) | |
| SupportedTerminalCheck `:1370-1429` | `Terminal support` | NoCheck if remote; macOS Normal; else Soft | `Terminal::parent_terminal()` None → on Linux inside special terminal (tmux etc.) Warning `Unable to determine the current terminal. Try running q doctor outside of <pty> for a more accurate check.`; else Error `Unsupported terminal, if you believe this is a mistake or would like to see support for your terminal, run q issue` (macOS info `__CFBundleIdentifier: <v or <not-set>>`) | – |
| ItermBashIntegrationCheck `:1465-1518` | `iTerm bash integration configured` | macOS, current shell bash, `~/.iterm2_shell_integration.bash` exists: Normal in iTerm else Soft | regex `V(\d*\.\d*\.\d*)` in file must be ≥0.4.0 else Error `iTerm Bash Integration is out of date. …`; no version → Warning | – |
| SshIntegrationCheck `:1102-1135` | `SSH integration` | commented out of the run list | | |

Analytics name = lowercase name with non-alphanumerics → `_` (`:386-389`); dotfile checks use `dotfile_check_<file_name>` (`:967-969`) — telemetry only.

(rec) tabsmith doctor set: DotfileCheck, data dir, PATH contains install dir, Windows console, settings corruption, unsupported environments (FigIntegrationsCheck minus Q_TERM version → `TABSMITH_PTY` version check), PTY socket/session check (non-interactive variant), dev-mode warning, InlineCheck (+ "LLM provider configured and API key env var present" when `inline.source ≠ local`), OS check, Bash/Fish version, terminal support, sandbox. Add `--format` (plain/json) as an extension (not upstream).

---

## 14. `hook` (hidden) (`crates/q_cli/src/cli/hook.rs:12-56`)

All positional unless noted; send a hook protobuf to the desktop socket; exit 0 on success, **1 on any failure, silently** (`:58-65`).

| subcommand | args |
|---|---|
| `editbuffer` | `<session_id> <integration:i32> <tty> <pid:i32> <histno:i64> <cursor:i64> <text>` |
| `hide` | – |
| `init` | `<pid> <tty>` |
| `integration-ready` | `<integration>` |
| `keyboard-focus-changed` | `<app_identifier> <focused_session_id>` |
| `pre-exec` | `<pid> <tty>` |
| `prompt` | `<pid> <tty>` |
| `ssh` | `<pid> <tty> <control_path> <remote_dest> [--prompt]` |
| `clear-autocomplete-cache` | `[--cli <name>]...` |

Current shell scripts don't call `q hook` (they emit OSC 697 sequences that the PTY parses); only the legacy `app/onboarding.sh` does. (rec) tabsmith: omit `hook`, or keep `hook clear-autocomplete-cache [--cli X]...` as a local spec-cache clear.

---

## 15. Internal `_` subcommands (`crates/q_cli/src/cli/internal/mod.rs:159-294`)

`#[command(hide = true, alias = "_")]` on the enum; root variant `internal` also hidden.

| subcommand | args | behaviour | used by |
|---|---|---|---|
| `pre-cmd` | `[--alias <str>]` (allow_hyphen_values) | if `QTERM_SESSION_ID` unset → exit 1; connect `figterm_socket_path`, send `UpdateShellContext{update_environment_variables:true, environment_variables: all env vars, update_alias:true, alias}`; exit 0/1 (`:873-911`) | bash/zsh/fish post scripts every prompt (background) `(command q _ pre-cmd --alias "$(\alias)" > /dev/null 2>&1 &)` |
| `local-state` | see below | state get/set/delete/list | `init` disabled message |
| `callback` | `<handler_id> [<filename> <exit_code:i64>]` (both or none) | without file: copies stdin to `/tmp/fig-callback-<9 alnum>` and uses exit_code −1; sends callback hook | desktop |
| `install` | `[--dotfiles] [--input-method] [--no-confirm] [--force] [--global]` | same as `setup` | install scripts |
| `uninstall` | `[--dotfiles] [--input-method] [--binary] [--ssh]` | none set ⇒ all; BINARY: if built with `Q_IS_PACKAGE_MANAGED` print `Please uninstall using your package manager` else remove binaries, print `\nThe binary was successfully uninstalled\n` (bold); then uninstall the rest (`:308-337`) | package scripts |
| `get-shell` | – | print parent process exe path (no newline); exit 1 if unknown (`:391-399`, `fig_proto/src/util.rs:9-13`; skips `toolbox-exec` parents, `fig_util/src/process_info/mod.rs:86-96`) | pre/post scripts `Q_SHELL=$(q _ get-shell)` |
| `hostname` | – | print hostname (no newline) | |
| `should-figterm-launch` | – | §12 | pre.sh / pre.fish / pre.nu |
| `event` | `--name <n> [--payload <json>] [--apps <a>]...` | send event hook | desktop |
| `sockets-dir` | – | print `sockets_dir()` + `\n` | tests |
| `stream-from-socket` | – | bidirectional pipe stdin/stdout ↔ `remote_socket_path()` (reconnect loop, 5s timeout) | figterm remote |
| `figterm-socket-path` | `<session_id>` | print `sockets_dir()/t/<id>.sock` + `\n` | |
| `ipc` | `(--app \| --figterm <id>) --json <json> [--recv]` | send arbitrary protobuf (JSON) to desktop or PTY socket; `--recv` prints response JSON | debugging |
| `uninstall-for-all-users` | – | Linux only; for each user: `runuser -u U -- q integrations uninstall --silent all` and `q _ remove-data-dir --force` | packaging |
| `remove-data-dir` | `[--force]` | Linux only; without `--force` error `remove-data-dir is dangerous! If you meant to run this command, pass --force` | |
| `uuidgen` | – | print UUIDv4 | shell scripts (older) |
| `ibus-bootstrap`, `detect-sandbox` (Linux) | – | detect-sandbox prints `No sandbox detected`(0) / `You are in a Flatpak`/`Snap`/`Docker container`/`generic container`/`<engine> container` (1) | |
| `open-uninstall-page` | – | opens survey URL | |
| `ssh-local-command` | `<remote_dest> <uuid>` | notify PTY `NotifySshSessionStarted` | ssh_inner LocalCommand |
| `prompt-ssh` | `<remote_dest>` | deprecated no-op, exit 0 | |
| `attempt-to-finish-input-method-installation` (macOS) | `[bundle_path]` | | |
| `dump-state` | `<figterm\|web-notifications\|platform>` | | |
| `finish-update`, `swap-files`, `brew-uninstall` | | updater | |
| `generate-ssh` | `[--remote-host H] [--remote-port P] [--remote-username U]` | §10.4 | ssh config |
| `inline-shell-completion` | `--buffer <str>` (allow_hyphen_values; accepts `""`, `-`, `--`, `--foo bar`) | connect PTY socket (`$QTERM_SESSION_ID`), request `InlineShellCompletion{buffer}` with **5s timeout**; on `insert_text` print `<buffer><insert_text>\n`, exit 0; any failure → exit 1, nothing printed (`inline_shell_completion.rs:37-79`) | zsh inline strategy: `suggestion="$(command -v q >/dev/null 2>&1 && q _ inline-shell-completion --buffer "${BUFFER}")"` (`strategies/inline.zsh:9`) |
| `inline-shell-completion-accept` | `--buffer <str> --suggestion <str>` | fire-and-forget `InlineShellCompletionAccept{buffer, suggestion}` to PTY; exit 0/1 (`:81-108`) | zsh widgets on accept: `(q _ inline-shell-completion-accept --buffer "$BUFFER" --suggestion "$POSTDISPLAY" > /dev/null 2>&1 &)` (`widgets.zsh:126,153`) |
| `multiplexer` / `mux` | `[--websocket] [--port N]` | remote dev | |

`local-state` (`internal/local_state.rs:18-126`) — same shape as `settings`:
```rust
enum LocalStateSubcommand { /// Reload the state listener
                            Init,
                            /// List all the settings
                            All { #[arg(long, short, value_enum, default_value_t)] format: OutputFormat } }
#[command(subcommand_negates_reqs = true, args_conflicts_with_subcommands = true)]
#[command(group(ArgGroup::new("vals").requires("key").args(&["value", "delete", "format"])))]
struct LocalStateArgs { cmd: Option<…>, /// Key of the state
                        key: Option<String>, /// Value of the state
                        value: Option<String>,
                        #[arg(long, short)] /// Delete the state
                        delete: bool,
                        #[arg(long, short, value_enum, default_value_t)] format: OutputFormat }
```
- `all [-f]`: Plain `key = <json>` per line (map order); Json / JsonPretty whole object.
- `KEY`: like settings get (string raw / JSON; missing: Plain error `No value associated with KEY`, Json `null`).
- `KEY VALUE`: JSON-or-string parse, set, prints `Successfully updated state`.
- `KEY -d`: remove (no glob), prints `Successfully updated state`.
- `KEY VALUE -d` → error `Cannot delete a value with a value`; no key → `No key specified`.
- `init`: desktop listener restart (`State listener restarted` or connection error text referencing `fig launch`) — drop.

(rec) tabsmith needs: `get-shell`, `should-figterm-launch` (rename `should-pty-launch`, keep alias), `pre-cmd`, `local-state`, `inline-shell-completion`, `inline-shell-completion-accept`, `sockets-dir`, `figterm-socket-path`, `uuidgen`, `hostname`, `ipc` (debug).

---

## 16. `debug` (`crates/q_cli/src/cli/debug/mod.rs:159-247`) — autocomplete-relevant parts

| subcommand | args | behaviour | tabsmith |
|---|---|---|---|
| `build` | `<dashboard\|autocomplete> [production\|beta(alias staging)\|develop(hidden, alias dev)]` | set: `developer.<app>.build` = null/"beta"/"develop" and print `Amazon Q will now use the <build> build of <app>`; get: print current (`production` default) (`:291-313`) | no |
| `autocomplete-window` | `[on\|off]` | IPC set/toggle debug mode; failure prints `Could not update debug mode` (`:314-324`) | (rec) toggle a debug overlay |
| `logs` | `[--level L] [files...]` | sets state `developer.logging=true`, sets desktop log level (default DEBUG), `tail -n0 -qf` on `logs_dir/*.log` or given `<file>.log` (`qterm` expands to `qterm*.log`); Ctrl-C resets `developer.logging=false` and level INFO (`:325-386`) | yes (no desktop part) |
| `key-tester` | – | raw mode; for each read prints `<n> bytes : "<escape_ascii>" : [hex,…]`, scrolls; Ctrl-D (0x04) exits and prints `ctrl-d` (`:597-631`) | **yes**, useful for keybinding debugging |
| `diagnostics` | `[--watch] [--rate <f64, default 0.25, requires --watch>]` | shows edit buffer (cursor marked `│` magenta, `\n\t\r` escaped), shell context (Session ID, Process Name, CWD, TTY, Preexec, OSCLock), `Intercept: x, Global Intercept: y`; `--watch` uses alt screen, redraw every `rate` s (`:632-746`) | yes, but query the PTY socket instead of desktop |
| `devtools <dashboard\|autocomplete>` | | desktop | no |
| `shell` | – | launch `qterm -- <shell>` with minimal config: bash `--noprofile --norc --rcfile <tmp>` (tmp contains `eval "$(q init bash post)"`), zsh with `ZDOTDIR=<tmpdir>` whose `.zshrc` is `eval "$(q init zsh post)"`, fish `--no-config -C 'q init fish post \| source'`; env `Q_DEBUG_SHELL=1`; nu unsupported; prints `Starting Amazon Q debug shell` / `Ending Amazon Q debug shell` (`:816-870`) | **yes** |
| `fix-permissions` | – | must run via sudo (`SUDO_UID`), else error `This command must be run with sudo`; for each shell config dir (≠ home) walk files and for each integration file: chown to user, ensure dirs `|0o700`, files `|0o600`, skip symlinks; print `Fixing owner for <p>` / `Fixing permissions for <p>`; summary `The permissions should be fixed now!` or `The permissions have not been modified`, plus `If you continue to experience issues:\n  1. Run q doctor\n  2. Report the issue with q issue` (`fix_permissions.rs:31-130`) | **yes** (unix) |
| `app`, `input-method`, `prompt-accessibility`, `sample`, `verify-codesign`, `accessibility`, `query-index`, `get-index`, `list-intellij-variants`, `refresh-auth-token` | | desktop/update/auth | no |

---

## 17. `completion` (hidden) (`crates/q_cli/src/cli/completion.rs`)

```rust
#[derive(ValueEnum)] enum Shells { /// Bash shell completions
    Bash, /// Fish shell completions
    Fish, /// Zsh shell completions
    Zsh, /// Fig completion spec
    Fig }
#[derive(Args)] struct CompletionArgs { /// Shell to generate the completion spec for
    #[arg(value_enum, default_value_t = Shells::Zsh)] shell: Shells }
```
Prints `clap_complete::generate(gen, Cli::command(), "q")` + newline to stdout; `fig` uses `clap_complete_fig::Fig` (a TS/JS Fig completion spec for the CLI itself). Exit 0. (rec) tabsmith: add `powershell` and `elvish` (clap_complete supports them) — useful on Windows; keep `fig` so tabsmith can complete itself through its own spec engine.

---

## 18. `setup` / `install`, `uninstall`

### 18.1 `setup` (alias `install`) and `_ install` (`internal/mod.rs:117-150`, `installation.rs:33-155`)
```rust
#[derive(Args)] struct InstallArgs {
    /// Install only the shell integrations
    #[arg(long)] dotfiles: bool,
    /// Prompt input method installation
    #[arg(long)] input_method: bool,
    /// Don't confirm automatic installation.
    #[arg(long)] no_confirm: bool,
    /// Force installation of q
    #[arg(long)] force: bool,
    /// Install q globally
    #[arg(long)] global: bool,
}
```
Components: neither `--dotfiles` nor `--input-method` ⇒ all components; else only the chosen ones.
Flow:
1. unix root and not `--global`: stderr `Installing as root is not supported.` (red bold); unless `--force` also `If you know what you're doing, run the command again with --force.` → exit 1.
2. shell integrations (not global): unless `--no-confirm`, require attended TTY (`You must run with --no-confirm if unattended`), ask `Do you want q to modify your shell config (you will have to manually do this otherwise)?` [Yes, No]. Yes/no-confirm → `fig_install::install(SHELL_INTEGRATIONS)` = install integrations for **all** shells incl. nu (none) (`fig_install/src/common.rs:142-163`; on multiple errors only the last is returned). On failure print `Could not automatically install:` (bold) + error and switch to manual. Manual (and not no-confirm): print instructions:
   ```
   To install the integrations manually, you will have to add the following to your rc files
   This step is required for the application to function properly

   At the top of your .bashrc or .zshrc file:
   bash:    . "$HOME/<data>/shell/bashrc.pre.bash"
   zsh:     . "$HOME/<data>/shell/zshrc.pre.zsh"

   At the bottom of your .bashrc or .zshrc file:
   bash:    . "$HOME/<data>/shell/bashrc.post.bash"
   zsh:     . "$HOME/<data>/shell/zshrc.post.zsh"
   ```
   then still install the files (dotfile integration) — errors printed as `Could not install required files:`.
3. `--global` with shell integrations: only a warning log.
4. macOS input method prompt (drop).
5. Login if not logged in (drop).
Exit 0.

### 18.2 `uninstall` (hidden root; `uninstall.rs:14-50`)
- `-y/--no-confirm`. Without it: prints `\nIs Amazon Q not working? Try running <bold magenta q doctor>\n`, then `dialoguer::Select` `Are you sure want to continue uninstalling Amazon Q?` [Yes, No] (default Yes); No/Esc → `Cancelled`, exit 1; Yes → `Uninstalling Amazon Q`.
- macOS: open survey URL, logout, `fig_install::uninstall(all)`. Linux minimal: only if exe is `~/.local/bin/q` (canonicalized) else errors `Uninstall is only supported for binaries installed in …` / `… only supported for "q" …`; components SHELL_INTEGRATIONS|BINARY|SSH. Linux full: package-managed refuse `Managed uninstalls are not supported. Please use your package manager to uninstall Amazon Q`.
- `fig_install::uninstall` (`common.rs:47-140`): SSH (if flag), **all shell integrations for bash/zsh/fish regardless of the SHELL_INTEGRATIONS flag** (upstream quirk), BINARY: remove `~/.local/bin/{q,qchat,qterm,cw,cwterm}` and `~/.local/bin/<shell> (qterm)` (+ old cwterm) for every shell — these are per-shell symlink/copies of the PTY binary named like `zsh (qterm)` so `ps` shows the shell name.
- (rec) tabsmith `uninstall`: confirm prompt identical; remove integrations, data dir optional (`--purge`), print binary-removal instructions per OS (cannot delete running exe on Windows).

---

## 19. `version` (hidden) (`mod.rs:193-200,492-567`; `feed.rs`)

```rust
Version { /// Show the changelog (use --changelog=all for all versions, or --changelog=x.x.x for a specific version)
          #[arg(long, num_args = 0..=1, default_missing_value = "")] changelog: Option<String> }
```
- No flag: `writeln!(stdout, "{}", Cli::command().render_version())` → `q 1.19.7\n` + extra `\n` (render_version already ends with a newline — inferred from clap 4). (Also `q --version`/`-V` from `#[command(version)]`.)
- `--changelog` (value ""): entry for `CARGO_PKG_VERSION`.
- `--changelog=all`: `Changelog for all versions:` then each entry; empty → `No changelog information available.`
- `--changelog=x.y.z`: `Changelog for version x.y.z:` + entry, or `No changelog information available for version x.y.z.` (still exit 0).
- Entry format: `Version <version> (<date>)`, then per change `  - <Added|Fixed|Changed|<raw type>>: <description>` (or `  No changes recorded for this version.`), then blank line.
- Data: `feed.json` embedded at compile time (`include_str!("../../../../feed.json")`); entries filtered `type=="release" && !hidden` (`feed.rs:30-48`). Schema `feed-schema.json`: `{entries:[{type:"release"|"announcement", date, version:"x.y.z", title, description?, link?, changes?:[{type: added|changed|deprecated|removed|fixed|security, description}], hidden?}]}`; file starts with a hidden `0.0.0` placeholder entry for the next release.

---

## 20. `diagnostic` and `issue`

### `diagnostic` (alias `diagnostics`) (`diagnostics.rs:36-94`)
```rust
#[derive(Args)] struct DiagnosticArgs { /// The format of the output
    #[arg(long, short, value_enum, default_value_t)] format: OutputFormat,
    /// Force limited diagnostic output
    #[arg(long)] force: bool }
```
- macOS & desktop not running & !force: prints `\n→ Amazon Q app is not running.\n  Please launch Amazon Q app with q launch or run q diagnostic --force to get limited diagnostics.` exit 1. (rec) tabsmith: no gate.
- Spinner `Generating...` only if stdout is a TTY.
- Plain = **TOML** of `Diagnostics` (`crates/fig_diagnostic/src/lib.rs:251-273`), Json = same struct as JSON. Structure (kebab-case): `[q-details] version, hash, date, variant`; `[system-info] os, chip, total-cores, memory`; `[environment] cwd, cli-path, os, shell-path, shell-version, terminal, install-method, in-cloudshell, in-ssh, in-ci, in-wsl, in-codespaces` (false flags omitted, `is_false`); flattened `[env-vars]` containing all product env vars (`env_var::ALL`) plus SHELL, DISPLAY, PATH, TERM, ZDOTDIR, XDG_CURRENT_DESKTOP, XDG_SESSION_DESKTOP, XDG_SESSION_TYPE, GLFW_IM_MODULE, GTK_IM_MODULE, QT_IM_MODULE, XMODIFIERS, __CFBundleIdentifier, with `/<username>` replaced by `/USER` (`lib.rs:126-168`). Exit 0.

### `issue` (`issue.rs`)
```rust
#[derive(Args)] struct IssueArgs { /// Force issue creation
    #[arg(long, short = 'f')] force: bool, /// Issue description
    description: Vec<String> }
```
- Desktop gate (skip in tabsmith). Title = joined description trimmed, or prompt `Issue Title` (dialoguer input).
- Prints `Heading over to GitHub...`; builds `https://github.com/<repo>/issues/new?template=1_bug_report_template.yml&os=<os>&environment=<warning+TOML diagnostics>&title=<title>` where warning = `<This will be visible to anyone. Do not include personal or sensitive information>\n\n`; opens browser; if remote or open fails prints `Issue Url: <underlined url>`. Exit 0 (`issue.rs:26-131`).

---

## 21. Paths summary (`crates/fig_util/src/directories.rs`)

| item | Linux | macOS | Windows | ref |
|---|---|---|---|---|
| data dir | `$XDG_DATA_HOME/amazon-q` or `~/.local/share/amazon-q` | `~/Library/Application Support/amazon-q` | `%LOCALAPPDATA%\AmazonQ` | `:146-150` |
| settings | `<data>/settings.json` | same | same | `:454-456` |
| state/history DB | `<data>/data.sqlite3` | same | same | `sqlite/mod.rs:74-76` |
| shell scripts | `<data>/shell/<rc>.<when>.<shell>` | same | – | `shell/mod.rs:117-119` |
| per-shell data | `<data>/shell/<shell>.json` | | | `shell.rs:125-127` |
| autocomplete specs | `<data>/autocomplete/specs` | | | `:282-289` |
| cache | `$XDG_CACHE_HOME/amazon-q` | `~/Library/Caches/amazon-q` | `%LOCALAPPDATA%\AmazonQ\cache` (comment) | `:186-190` |
| runtime dir | `$XDG_RUNTIME_DIR` → `$TMPDIR` → `temp_dir()` | `$TMPDIR` → `confstr(_CS_DARWIN_USER_TEMP_DIR)` | `%TEMP%` | `:211-231` |
| sockets dir | `<runtime>/cwrun` | `<runtime>/cwrun` | `%TEMP%\AmazonQ\sockets` | `:238-246` |
| PTY session socket | `<sockets>/t/<QTERM_SESSION_ID>.sock` | same | same (Windows AF_UNIX) | `:382-384` |
| desktop / remote socket | `<sockets>/desktop.sock`, `remote.sock` | | | `:343-374` |
| logs | `<runtime>/qlog` | `<runtime>/qlog` | `%TEMP%\AmazonQ\logs` | `:295-304` |
| backups | `~/.amazon-q.dotfiles.bak/<YYYY-MM-DD_HH-MM-SS>` (UTC) | | `%USERPROFILE%\…` | `:310-323` |
| themes | resources/themes (app bundle / `/usr/share/amazon-q`) | | | `:277-279,403-420` |
| update lock | `<data>/update.lock` | | | `:463-465` |
| install dir checked by doctor | `~/.local/bin` (in PATH) | `~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin` | – | `doctor/mod.rs:412-434,1248-1258` |

---

## 22. Out of scope (do not implement)
chat, mcp, agent, translate (`ai`), login/logout/whoami/profile/user, telemetry, update/upgrade, launch/quit/restart/dashboard, `settings` (no-args) desktop UI, `debug` desktop subcommands, input-method/vscode/intellij/autostart/gnome integrations, `_` desktop/updater subcommands, LoginStatusCheck, MidwayCheck, desktop-app doctor groups (4, 7-9).

## 23. Bug-for-bug notes worth deciding on (rec)
1. `doctor` returns exit 0 even on failures (`doctor/mod.rs:2344`).
2. `theme` default printed `dark` vs dashboard default `system` (`theme.rs:25`, `autocomplete.ts:179`).
3. `theme --list` omits built-ins and errors when dir missing (`theme.rs:59-67`).
4. `integrations status dotfiles <shell>` ignores `<shell>` (`integrations.rs:410-414`).
5. `fig_install::uninstall` always removes shell integrations (`common.rs:54-61`).
6. `ToolboxInstalledCheck` warning is discarded (`doctor/mod.rs:1937`).
7. `fig_install::install` returns only the last error (`common.rs:160-162`).
8. Warnings in doctor print only the message, not the check name (`doctor/mod.rs:326-328`).
