# tabsmith — authoritative implementation spec

`tabsmith` is a single cross-platform Rust binary (macOS, Linux, Windows) that re-implements **only the CLI
autocompletion part of Kiro CLI** (= Amazon Q Developer CLI = Fig): an in-terminal dropdown menu drawn by a PTY
wrapper, inline ghost text, and the CLI commands around them (`settings`, `theme`, `inline`, `integrations`,
`doctor`, `init`, `version`, `setup`, `uninstall`, …), driven by the official Fig completion specs run in an
embedded QuickJS engine.

This document is the contract. Where it disagrees with the seven research reports, this document wins (the
decision log in §0.3 says why). The reports remain the place for verbatim detail; this spec cites them as
`R:<report> §n` (files in the same directory) and cites reference sources as `qac/…`, `ish/…`, `specs/…`
(root `scratchpad/ref/`).

Markers: **MUST** = Kiro parity or required for the product to work. **SHOULD** = worth doing, not in v1's
critical path. **SKIP** = deliberately not built (reason given). **(inferred)** = not read from code.
**(verify)** = check against the pinned crate version before relying on it.

---

## 0. Ground rules

### 0.1 Rebrand map

| Kiro / Q / Fig | tabsmith |
|---|---|
| `kiro-cli` / `q` (CLI) | `tabsmith` |
| `qterm` / figterm (PTY wrapper binary) | `tabsmith _ pty` (same binary, hidden subcommand) |
| `Amazon Q` product name in strings | `tabsmith` |
| data dir `<data_local>/amazon-q` | `~/.tabsmith` (override `TABSMITH_HOME`) |
| `Q_*`, `QTERM_SESSION_ID` env vars | `TABSMITH_*`, `TABSMITH_SESSION_ID` |
| `PROCESS_LAUNCHED_BY_Q` | `PROCESS_LAUNCHED_BY_TABSMITH` |
| `SHOULD_QTERM_LAUNCH` | `SHOULD_TABSMITH_LAUNCH` |
| `# Amazon Q pre block. Keep at the top of this file.` | `# tabsmith pre block. Keep at the top of this file.` |
| `~/.amazon-q.dotfiles.bak/` | `~/.tabsmith/backups/` |
| OSC `ESC ] 697 ;` | **kept** (`697`) |
| `# fig ignore` escape hatch | `# tabsmith ignore` |
| CDN specs `https://specs.q.us-east-1.amazonaws.com` | embedded `@withfig/autocomplete@2.692.3` |
| CodeWhisperer inline model | local sources + optional OpenAI-compatible / Anthropic profiles |

### 0.2 Engineering constraints

- One crate, one binary. Rust 2021, MSRV 1.85 (local toolchain is 1.95).
- No async runtime (std threads + `std::sync::mpsc`), no SQLite, no IPC sockets, no `regex`, `dirs`, `uuid`,
  `chrono`, `dialoguer`, `globset` crates. Each was replaced by a few lines of std code (noted where it happens).
- Logs never go to the user's terminal from inside `_ pty` (they would corrupt the screen).

### 0.3 Decision log (where the references or reports disagree)

| # | Topic | Decision | Why |
|---|---|---|---|
| D1 | How the wrapper starts | Kiro model: dotfile **pre** block `exec`s `tabsmith _ pty` (bash/zsh/fish); the inner shell re-reads its rc, whose **post** block emits markers. pwsh has no `exec`: pre block runs the wrapper as a child then `Stop-Process -Id $PID` (ish pattern, `ish/src/utils/shell.ts:327-376`). | Kiro parity for `integrations install dotfiles`; ish's launch-flag injection (`--init-file`, ZDOTDIR swap) is only used by `debug shell`. |
| D2 | Marker protocol | OSC 697 key names (Kiro, `R:shell-integration §6`). Values are **not escaped**; tabsmith's own splitter takes the whole payload up to BEL/ST, so `;` in paths survives. Markers are **stripped** before output reaches the real terminal. | Fixes Kiro's `;` truncation (`qac/crates/alacritty_terminal/src/ansi.rs:1176`) without ish's per-prompt byte-escaping loop. Values containing BEL/ESC/LF are unsupported (documented). |
| D3 | Edit buffer | Screen-scraped from a headless model (both refs). Cells are **tagged inside the model** with injected OSC 8 hyperlinks (`tabsmith:p` prompt, `tabsmith:c<n>` command cycle n). | Same idea as figterm's per-cell `FigFlags` (`qac/crates/alacritty_terminal/src/term/cell.rs:19-45`) without forking the crate; tags move with scroll/reflow, so no stored coordinates (Kiro's `cmd_cursor` drift bugs disappear). |
| D4 | Model crate | `alacritty_terminal` | Full SGR incl. dim/italic, WRAPLINE, reflow, alt screen, OSC 8 per cell (`R:overlay-rendering §17.1`). |
| D5 | Default keybindings | `fig.json` set (Enter, Tab, Shift-Tab/Up/Ctrl-P, Down/Ctrl-N, Esc, Ctrl-K description, Ctrl-R history) | It is what Kiro actually runs with; `actions.json` is overwritten at runtime (`R:autocomplete-ui §3.1`). |
| D6 | Delete byte for insertions | `0x7f` (DEL) for every shell | PSReadLine treats `0x08` as Ctrl+Backspace (word delete) (`R:overlay-rendering §17.7.8`); bash/zsh/fish bind both. |
| D7 | Drawing/restoring | Absolute cursor positioning (`CSI r;c H`) per box row; restore only the covered cell span by re-emitting model cells (truecolor exact). Every frame inside synchronized output `CSI ?2026h/l`. | Cheaper and more exact than ish's whole-row `CSI 2K` repaint with 256-colour SGR bugs (`R:overlay-rendering §7.3`); avoids DECSC single-slot clobbering. |
| D8 | Not enough room | Never scroll the real terminal. Below if it fits, else above, else shrink rows on the bigger side, else hide. | Scrolling would desync model, terminal and the shell's line editor (`R:overlay-rendering §17.5`). |
| D9 | Where the engine lives | Rust implements tokenizer, parser, suggestions, ranking; QuickJS only loads spec modules and calls spec functions. | "Rust as the main language"; pure parts unit-testable with `cargo test`. |
| D10 | Generator cache | ish semantics: fresh within TTL; past TTL `max-age` blocks, otherwise serve stale + background refresh; failures never cached. | qac's cache blocks on refresh and poisons on failure (`R:parser-runtime §4.8`). |
| D11 | Specs | Embedded solid `specs.tar.zst` (4.2 MB) extracted once to `~/.tabsmith/specs/<ver>/`; user dir overrides. No CDN. | Offline, pinned, reproducible (`R:spec-format §13`). |
| D12 | `doctor` exit code | 1 if any error (and with `--strict`, any warning); Kiro always exits 0 (`qac/crates/q_cli/src/cli/doctor/mod.rs:2344`). | Scriptable. Documented deviation. |
| D13 | Theme default | Unset → `dark` (Kiro code path, `qac/crates/q_cli/src/cli/theme.rs:25`); `system` = query terminal background (OSC 11). | Kiro's dashboard default (`system`) relied on OS appearance; a terminal popup has its own bg. |
| D14 | Menu size units | `autocomplete.width` = columns (default 40), `autocomplete.height` = max list rows (default 7). | Kiro's 320 px × 140 px at 20 px rows = 7 rows ≈ 40 cols (`R:autocomplete-ui §10.2`). |
| D15 | bash hooks | `PS0` (preexec) + `PROMPT_COMMAND` (precmd); requires bash ≥ 4.4. No vendored bash-preexec. | Removes ~380 vendored lines and its DEBUG-trap/HISTCONTROL side effects (`R:shell-integration §5.2.1`). macOS `/bin/bash` 3.2 is unsupported (doctor error). |
| D16 | Inline ghost text | Drawn by the wrapper for every shell. Default source `history+specs` (local). LLM profiles opt-in and consulted only when no local history match exists. | Kiro's ghost text is a zsh-only plugin calling a cloud model (`R:figterm §15`). |
| D17 | IPC | No sockets. `tabsmith _ pre-cmd` writes `~/.tabsmith/run/<session>.json` (env + aliases); the wrapper re-reads it when its mtime changes. Live settings reload by mtime. | Cross-platform with zero code for named pipes/UDS. |
| D18 | History store | `~/.tabsmith/history.jsonl` | No SQLite C build; same fields as Kiro's table (`R:figterm §16`). |
| D19 | Launch decision | Exit code 0 (launch) / 1 (don't). Deny-list of environments only; no terminal allow-list or process-tree walk. SSH and WSL allowed. | The allow-list existed to position the desktop GUI window; an in-terminal overlay works in any VT terminal. tabsmith runs on the remote host itself, so SSH needs no tunnel. |
| D20 | cmd.exe, nushell | SKIP in v1 | cmd has no preexec/exit/cwd hooks and Kiro reports it unsupported (`doctor/mod.rs:591-712`); Kiro's nu scripts are legacy/broken (`R:shell-integration §4.3`) and nu redraws its prompt on every key. |
| D21 | Recency key | `tokens[commandIndex]` for both read and write | Kiro reads `tokens[0]`, writes `tokens[commandIndex]` (bug, `R:autocomplete-ui §8.2`). |
| D22 | Backspace count on insert | Chars of the **raw** on-screen token text | Kiro escapes only the first space and ignores typed quotes (`qac/packages/autocomplete/src/state/insertion.ts:25-90`). |
| D23 | Parser quirks | `FOO= cmd` = complete empty assignment; string `getQueryTerm` slices at `lastIndexOf(s)+s.len()` | Kiro throws / uses `+1` (`R:parser-runtime §1.7, §4.9`); all spec strings are 1 char so ranking is unchanged. |
| D24 | Ghost-colour exclusion | Only cells **at or after the cursor** are tested against the suggestion colour | Prevents syntax-highlighted typed text in `fg=8` from vanishing from the buffer. |
| D25 | Execute byte | `\r` | Kiro's UI inserts `\n`; figterm's execute path uses `\r` (`qac/crates/fig_proto/src/figterm.rs:4-30`); `\r` is Enter on every shell incl. PSReadLine. |

---

## 1. Feature list (Kiro → tabsmith)

| # | Kiro feature | tabsmith | Notes / reason |
|---|---|---|---|
| F1 | Autocomplete dropdown | **MUST** | In-terminal overlay instead of a GUI window (§7). |
| F2 | Fig spec engine: subcommands, options, args, option chains `-abc`, `--opt=val`, `requiresSeparator`, `isRepeatable`, `exclusiveOn`, `dependsOn`, `--`, `optionsMustPrecedeArguments`, `optionsCanBreakVariadicArg`, `flagsArePosixNoncompliant`, persistent options, `loadSpec`, `isCommand/isScript/isModule`, `generateSpec`, arg `alias` directive | **MUST** | §6. |
| F3 | Diff-versioned specs (az, heroku, fig, infracost, shopify, @usermn/sdc) | **MUST** | §6.3; `@usermn/sdc` skips version detection (would run `npx`). |
| F4 | Generators: `script`, `postProcess`, `splitOn`, `custom`, `trigger`, `getQueryTerm`, `cache`, `debounce`, `scriptTimeout` | **MUST** | §6.6. |
| F5 | Templates `filepaths`, `folders`, `help` | **MUST** | §6.6.4. |
| F6 | Template `history` (7 spec uses) | SKIP | Needs a per-spec history index (`R:parser-runtime §4.4`); returns `[]`. |
| F7 | Alias expansion (first token, from shell `alias` output) | **MUST** | §6.1.6. |
| F8 | Filtering (prefix / fuzzy via fuzzysort), `filterStrategy`, exact-match buckets, dedupe | **MUST** | §6.7. |
| F9 | Ranking: priority, recency (`sortMethod`) | **MUST** | §6.7.4. |
| F10 | Auto-execute rows (`↪`, dangerous-command double-Enter) | **MUST** | §6.7.3. |
| F11 | Visibility state machine (Esc hides for the line, hide after insertion, backspace into previous token, …) | **MUST** | §7.1. |
| F12 | Keybinding actions + `autocomplete.keybindings.*` overrides incl. `"ignore"` | **MUST** | §7.2. |
| F13 | `insertCommonPrefix` (Tab) with failure "shake" | **MUST** | Shake = BEL `\x07` to stdout. |
| F14 | Description bar; argument hint when no suggestions | **MUST** | §7.4. |
| F15 | Description pop-out (`toggleDescription`, `alwaysShowDescription`) | SHOULD | §7.4.3. |
| F16 | History rows in dropdown (`beta.history.mode`, `toggleHistoryMode` Ctrl-R) | **MUST** | Uses tabsmith's own history file (§3.5). Kiro's shell-history loading via login shells: SKIP. |
| F17 | First-token completion (`autocomplete.firstTokenCompletion`) | **MUST** | Spec names + aliases + PATH executables instead of `compgen -c` (portable). |
| F18 | Themes dark/light/system/custom JSON | **MUST** | Named community themes (dracula, nord, …) SKIP: files not in the Kiro repo (`R:autocomplete-ui §13.3`). |
| F19 | `increaseSize` / `decreaseSize` | SKIP | Font scaling has no terminal meaning. |
| F20 | `selectSuggestion1..10` | SHOULD | Kiro's is a no-op bug; tabsmith selects the Nth visible row. No default binding (Cmd/Ctrl+digit not deliverable). |
| F21 | Loading indicator | **MUST** | Spinner glyph in the description bar after 200 ms; list stays visible (fixes Kiro's whole-window spinner). |
| F22 | Inline ghost text | **MUST** | All shells, drawn by the wrapper (§8). |
| F23 | Inline LLM completion | **MUST** (opt-in) | Replaces CodeWhisperer: OpenAI-compatible + Anthropic profiles (§8.5). |
| F24 | Shell integration bash, zsh, fish | **MUST** | §5. |
| F25 | Shell integration pwsh (PowerShell 7+) | **MUST** | Windows' primary shell; port of ish's prompt wrapping + Kiro events (§5.4.4). Windows PowerShell 5.1 (`powershell`): SHOULD (same scripts, other profile path). |
| F26 | nushell, cmd.exe, xonsh | SKIP | D20. xonsh was never in Kiro. |
| F27 | Launch guards (`should-figterm-launch`) | **MUST** | §5.2. |
| F28 | OSC lock / session-id guard against nested shells | **MUST** | §5.1. |
| F29 | Insert text into shell with buffer reconciliation + insertion lock | **MUST** | §7.3. |
| F30 | Command history recording | **MUST** | Feeds inline + history rows (§3.5). |
| F31 | Shell env/alias capture for generators (`_ pre-cmd`) | **MUST** | D17. |
| F32 | Spec execution in the user's environment (RunProcess semantics) | **MUST** | §9.3. |
| F33 | Windows ConPTY delayed EndPrompt/NewCmd | **MUST** | §5.1.4. |
| F34 | CSI-u keyboard mode (`qterm.csi-u.enabled`) | SKIP | Only needed for Ctrl+digit bindings; off by default in Kiro. |
| F35 | `# question` → translate hack | SKIP | AI translate is out of scope. |
| F36 | Typeahead capture `Q_START_TEXT` | SKIP | tabsmith enters raw mode with `TCSANOW`, so pending input is not flushed; the inner shell receives it (may echo twice — cosmetic). |
| F37 | Shortcut specs `?` and `+` | SKIP | Legacy Fig shortcuts. |
| F38 | Dev mode spec folder (`autocomplete.developerMode` + `devCompletionsFolder`) | SHOULD | One extra directory in the resolution order. Dev server port: SKIP. |
| F39 | Spec CDN | SKIP | D11. `tabsmith specs update` (download npm tarball): SHOULD, post-v1. |
| F40 | `settings`, `theme`, `inline`, `integrations`, `init`, `doctor`, `setup`, `uninstall`, `version` | **MUST** | §2. |
| F41 | `diagnostic`, `completion`, `debug key-tester/shell/logs` | SHOULD | §2. |
| F42 | `issue`, `hook`, `update`, `login/logout/whoami/profile/user`, `chat/mcp/agent/translate`, `telemetry`, `launch/quit/restart/dashboard`, `_ local-state`, `debug diagnostics/fix-permissions/build/app/…` | SKIP | Out of scope (desktop app, auth, AI chat, telemetry) or need the socket tabsmith does not have. |
| F43 | Integrations `ssh`, `input-method`, `vscode`, `intellij-plugin`, `autostart-entry`, `gnome-shell-extension` | SKIP | Desktop-app plumbing (`R:cli-surface §10.4-10.5`). Not present in the clap tree. |
| F44 | Mouse click on a row | SKIP | Needs mouse reporting, conflicts with terminal selection. |
| F45 | `version --changelog` | SKIP | No release feed yet; add with the first release. |
| F46 | JetBrains login-shell / re-sourcing fixes, input-method prompts, login nags, inline announcement | SKIP | IDE/desktop niceties. |

---

## 2. CLI surface

### 2.1 Global behaviour

```text
tabsmith [-v...] [-V|--version] [-h|--help] [COMMAND]
```

- No command → print help, exit 0 (Kiro launches chat, `R:cli-surface §1.1`).
- `-v` (count, global): log level 1=warn, 2=info, 3=debug, ≥4=trace; also mirrors CLI logs to stderr. Never affects `_ pty`.
- `-V/--version` → `tabsmith <CARGO_PKG_VERSION>` (clap).
- clap parse error → clap's message and exit code (2). Command error → stderr `error: <msg>` (`error:` bold red when
  stderr is a tty), exit 1. Successful commands exit 0 unless stated.
- `OutputFormat` (`-f/--format`): `plain` (default) | `json` (compact) | `json-pretty` (2-space).

### 2.2 Command tree (clap derive; doc comments become help text)

```text
tabsmith settings [KEY] [VALUE] [-d|--delete] [-f|--format FMT]      "Customize appearance & behavior"
         settings open                                                "Open the settings file"
         settings list [--all] [-f FMT]                               "List settings"
         settings all [-f FMT]            (hidden; = list)
tabsmith theme [--list | --folder | THEME]                            "Get or set theme"
tabsmith inline enable|disable|status                                 "Inline shell completions"
         inline set-customization [NAME]      (alias: set-source)     "Select the inline suggestion source"
         inline show-customizations [-f FMT]  (alias: show-sources)   "Show the available inline sources"
tabsmith integrations install   [-s|--silent] <dotfiles [bash|zsh|fish|pwsh] | all>   "Manage system integrations"
         integrations uninstall [-s] <dotfiles [SHELL] | all>
         integrations reinstall [-s] <dotfiles [SHELL] | all>
         integrations status    [-f FMT] <dotfiles [SHELL] | all>
tabsmith init <bash|zsh|fish|pwsh> <pre|post> [--rcfile NAME]         "Generate the dotfiles for the given shell"
tabsmith doctor [-a|--all] [-s|--strict]                              "Fix and diagnose common issues"
tabsmith setup [--no-confirm] [--force]          (alias: install)     "Setup cli components"
tabsmith uninstall [-y|--no-confirm] [--purge]                        "Uninstall tabsmith"
tabsmith version                                                      (hidden) "Show the version"
tabsmith diagnostic [-f FMT]                     (alias: diagnostics) "Run diagnostic tests"          SHOULD
tabsmith completion [bash|zsh|fish|powershell|elvish]  (hidden, default zsh)                         SHOULD
tabsmith debug key-tester | shell [bash|zsh|fish|pwsh] | logs                                       SHOULD
tabsmith _ (alias: internal, hidden)
         _ pty [--shell PATH] [-- CMD [ARGS...]]
         _ should-launch
         _ get-shell
         _ pre-cmd [--alias TEXT]     (allow_hyphen_values on --alias)
```

`-s/--silent` and `-f/--format` belong to the parent subcommand, so they precede the integration name:
`tabsmith integrations uninstall --silent all` (Kiro semantics, `R:cli-surface §10.1`).

### 2.3 `settings` (port of `qac/crates/q_cli/src/cli/settings.rs:42-230`)

clap: `#[command(subcommand_negates_reqs = true, args_conflicts_with_subcommands = true)]`,
`#[command(group(ArgGroup::new("vals").requires("key").args(["value", "format"])))]`.

| Invocation | Behaviour | Exit |
|---|---|---|
| `settings KEY` | Plain: string value printed raw, other JSON pretty (`{:#}`). json/json-pretty: the JSON value. Missing key: plain → `error: No value associated with KEY`; json formats → prints `null`. | 1 / 0 |
| `settings KEY VALUE` | `serde_json::from_str(VALUE).unwrap_or(Value::String(VALUE))`, write file. No output. | 0 |
| `settings KEY VALUE -d` | `error: the argument '--delete' cannot be used with '[VALUE]'\n Usage: tabsmith settings --delete {KEY}` | 1 |
| `settings -d PATTERN` | PATTERN is a wildcard (`*`, `?`; hand-written matcher — globset's `[..]`/`{..}` not supported). 0 matches → `error: No settings found matching PATTERN`; 1 → `Removing "key"`; n → `Removing:` then `  - key` lines. | 0/1 |
| `settings -d` | `error: the argument '--delete' requires a <KEY>\n Usage: tabsmith settings --delete <KEY>` | 1 |
| `settings` | Print `settings --help`. | 0 |
| `settings open` | `$EDITOR <path>` if set (wait); else macOS `open -t <path>`, Windows `notepad <path>`; else `error: The EDITOR environment variable is not set`. | 0/1 |
| `settings list` | Configured keys sorted. Plain: `key = <compact json>` per line. json: the whole object. | 0 |
| `settings list --all` | Every catalog key (§3.3). Plain: `key\ttype\tdefault\tvalue` with a header line. json: `[{key,type,default,description,value}]`. | 0 |

### 2.4 `theme` (port of `theme.rs:51-128`)

- `--folder`: print `~/.tabsmith/themes` (create if missing).
- `--list`: built-ins `dark`, `light`, `system` then `*.json` stems from the folder, sorted, deduped (fixes Kiro's missing built-ins).
- `THEME`: if `<folder>/<THEME>.json` exists → parse `{author?:{name?,twitter?,github?}, version?, theme?, shade0..7?, accent0..7?}`
  (parse error → exit 1); print `\n› Switching to theme '<THEME bold>'` + ` by <name bold>` when `author.name`,
  then optional `  🐦 <twitter>` / `  💻 github.com/<github>` lines, then `\n`; set `autocomplete.theme`. Built-in name →
  `› Switching to theme '<THEME bold>'`, set. Else `error: '<THEME>' does not exist in <folder>`.
- No args: print `autocomplete.theme` (unset → `dark`).

### 2.5 `inline` (port of `inline.rs`, customizations → sources)

| cmd | Behaviour | stdout | exit |
|---|---|---|---|
| `enable` / `disable` | set `inline.enabled` true/false (running wrappers pick it up via mtime reload) | `Inline enabled` / `Inline disabled` (magenta) | 0 |
| `status` | `inline.enabled` (default true) | `Inline is <enabled\|disabled bold>` | 0 |
| `set-customization NAME` | NAME ∈ {`history`, `specs`, `history+specs`} ∪ keys of `inline.profiles.*`. Unknown → `error: Customization not found`. Else set `inline.source`=NAME, print `Customization <NAME bold> selected`. | | 0/1 |
| `set-customization` (no arg, tty) | Numbered list (`1) history+specs - Local history, then completion specs` …, profiles `N) <name> - <provider> <model>`, last `N) None`), prompt `Select a customization: `. `None` → remove `inline.source`, print `Customization unset`. Empty/invalid input or non-tty → `error: No customization selected`. | | 0/1 |
| `show-customizations [-f]` | Plain: for each source `<name bold>[ (selected)]` + newline + description + blank line. json: `[{name, kind:"builtin"\|"profile", description, selected, provider?, model?, base_url?, api_key_env?}]`. Profiles with an invalid object print `<name> (invalid: <reason>)`. | | 0 |

Profiles are created with `settings`, e.g.
`tabsmith settings inline.profiles.claude '{"provider":"anthropic","model":"claude-opus-5-5","api_key_env":"ANTHROPIC_API_KEY","effort":"low"}'`.
`show-customizations` prints that example when no profile exists.

### 2.6 `integrations` (port of `integrations.rs`)

- `install dotfiles [SHELL]`: shells = `[SHELL]` or `[bash, zsh, fish]` (+ `pwsh` when `pwsh` is on PATH). Each
  integration of each shell: if `is_installed()` is Ok → skip, else `install()`. Errors collected as
  `"<describe bold>: <message>"`, reported as one error `"\n\n" + errs.join("\n\n")` (exit 1). Output unless
  `--silent`: `Installed!` if something was installed, else `Already installed`.
- `install all` = `install dotfiles` (all shells).
- `uninstall dotfiles [SHELL]|all`: uninstall each integration whose `is_installed()` is Ok; `Uninstalled!` / `Not installed`.
- `reinstall` = uninstall then install.
- `status dotfiles [SHELL]|all`: **honours SHELL** (Kiro ignores it). Plain: `✔ <describe>` (green) / `✘ <describe>` (red);
  then `<shell>: <error>` lines for enumeration errors. json: `{"integrations":[{"installed":bool,"description":str,"shell":str,"file_name":str}],"errors":[{"shell":str,"error":{"title":str,"message":str|null}}]}`.
- `describe()`: dotfile → `"<shell> pre post into <file_name>"` (e.g. `bash pre post into .bashrc`); fish conf.d file → `"fish <pre|post>"`.

### 2.7 `init`

`tabsmith init <shell> <pre|post> [--rcfile NAME]` prints the integration code + `\n`, always exit 0; on error prints
`# Could not load source: <err>` (a comment, so `eval` never breaks). Output composition in §5.3. `--rcfile` is
accepted and logged (Kiro's Linux-`profile` empty-output special case is dropped: the pre script's own interactive
guard covers it). `TABSMITH_INIT_SNAPSHOT_TEST=1` makes output deterministic (`TABSMITH_SHELL='/bin/zsh'` or
`'pwsh'`, `SHOULD_TABSMITH_LAUNCH=0`, binary path `tabsmith`).

### 2.8 `doctor` (port of `doctor/mod.rs:2079-2345`)

Flags `-a/--all` (run every check, print every result, never fix, never abort), `-s/--strict` (soft checks become
normal; warnings count as failures for the exit code). No `--format`.

Flow:
1. Unix euid 0: stderr `Running doctor as root is not supported.` (red bold); without `--all` also
   `If you know what you're doing, run the command again with --all.` → exit 1.
2. Without `--all`: spinner `Running checks...` on stderr (Braille dots, 80 ms), cursor hidden; restored on exit/Ctrl-C.
3. Groups in order (header printed dim only with `--all`). Per check: `type` = Normal | Soft | NoCheck(skip); result
   Ok | Warning(msg) | Error{reason, info[], fix?}. Without `--strict`, a Soft Error becomes Warning(reason). Print when
   `--all` or not Ok. On Error (no `--all`): if `fix` → `Attempting to fix automatically...`, run; failure →
   `Failed to fix: <err>`; success → `Re-running check...`, re-check, print; still Error → blank line, **abort**
   remaining groups.
4. Symbols: Ok `✔ <name>` (green), Warning `● <msg>` (yellow), Error `✘ <name>: <reason>` (red) + blank + `  <info>` lines.
5. Footer: errors → `\n✘ Doctor found errors. Please fix them and try again.\n`; success → `✔ Everything looks good!`
   (not with `--all`) then `\n  tabsmith still not working? Check ~/.tabsmith/logs/pty.log\n`. If state
   `doctor.prompt-restart-terminal` is true: `  PS. Autocomplete won't work in any existing terminal sessions, only new ones.`
   + `  (You might want to restart your terminal emulator)`; clear the flag.
6. Exit 1 if any Error (or Warning with `--strict`), else 0 (D12).

| Group header | Check name | Type | Logic → message | Fix |
|---|---|---|---|---|
| `Let's check your dotfiles...` | `<shell> <~/path> integration check` (one per integration of bash, zsh, fish, pwsh) | Normal if it is the current shell (parent process name), Soft if that shell is on PATH, else NoCheck | `is_installed()` error text; info `Run tabsmith integrations install dotfiles <shell> to reinstall shell integrations for <shell>` | `install()` |
| `Let's make sure tabsmith is set up correctly...` | `tabsmith data dir exists` | Normal | `~/.tabsmith` missing → `No directory at path <p>` | create it |
| | `Integration scripts point at this binary` | Normal | installed script files reference a path ≠ `current_exe()` → `Integration scripts reference <old>` | reinstall all dotfiles |
| | `tabsmith settings corruption` | Normal | settings.json parse error → `tabsmith settings file is corrupted` | copy to `settings.json.bak-<unix>` then write `{}` (Kiro writes `{}` without backup) |
| | `Completion specs installed` | Normal | `~/.tabsmith/specs/<ver>/index.json` missing → `Completion specs are not extracted` | extract |
| | `Windows Console Check` (Windows) | Normal | `GetConsoleMode` fails on stdin/stdout → `Windows Console APIs are not supported in this terminal` + Kiro's MinTTY/winpty info lines | – |
| | `Windows version` (Windows) | Normal | build < 17763 → `Windows 10 1809 or newer is required (ConPTY)` | – |
| `Let's check if your environment is supported...` | `tabsmith terminal integrations` | Normal | in order: `TERM_PROGRAM=WarpTerminal` → `WarpTerminal is not supported`; parent `cmd.exe` → `CMD isn't supported, use PowerShell 7 (pwsh) instead`; `INSIDE_EMACS` → `Emacs is not supported`; SecureCRT → `SecureCRT is not supported`; `PROCESS_LAUNCHED_BY_TABSMITH` → `tabsmith can not run in a process it launched`; CI → `Doctor can not run in CI`; then `TABSMITH_TERM`: == version → Ok; unset → `tabsmith is not running in this terminal, please try restarting your terminal`; other → `This terminal is not running with the latest integration, please restart your terminal` (info `TABSMITH_TERM=<v>`) | – |
| | `Session check` | Normal | `TABSMITH_SESSION_ID` unset → skip (NoCheck); set but `~/.tabsmith/run/<sid>.json` missing → Warning `Shell has not reported its environment yet (tabsmith _ pre-cmd did not run)` | – |
| | `Autocomplete is enabled` | Normal | `autocomplete.disable` = true → `Autocomplete disabled.` info `To fix run: tabsmith settings autocomplete.disable false` | – |
| | `Autocomplete dev mode` | Normal | `autocomplete.developerMode` = true → Warning `autocomplete.developerMode is enabled` | – |
| | `Inline` | Normal if inline enabled, else Soft | disabled → Warning `Inline is disabled, to re-enable run: tabsmith inline enable`; `inline.source` names a missing/invalid profile → Error `Inline source '<n>' is not a valid profile`; profile key env var unset in current env (and base_url not localhost) → Error `<API_KEY_ENV> is not set` | – |
| `Let's check if your system is compatible...` (result ignored) | `OS is supported` | Normal | macOS ≥ 11, Linux, Windows ≥ 1809 → Ok; else Warning | – |
| | `Bash is new enough` | Soft if current shell is bash else NoCheck | `bash --version` `GNU bash, version (\d+\.\d+)` < 4.4 → `Bash <v> is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh` | – |
| | `Fish is up to date` | Normal (skip if no `fish`) | `< 3.3.0` → `your fish version is outdated (need at least 3.3.0, found <v>)` | – |

### 2.9 `setup` / `uninstall`

`setup [--no-confirm] [--force]`:
1. Unix euid 0 and not `--force` → stderr `Installing as root is not supported.` + `If you know what you're doing, run the command again with --force.`, exit 1.
2. Without `--no-confirm`: requires a tty (`error: You must run with --no-confirm if unattended`); ask
   `Do you want tabsmith to modify your shell config (you will have to manually do this otherwise)? [Y/n] `.
3. Yes → `integrations install dotfiles` for every supported shell found on PATH; print result; extract specs
   (§9.1); print `Restart your terminal (or open a new tab) to start using tabsmith.`
4. No → print manual instructions (Kiro text, `R:cli-surface §18.1`), with tabsmith paths, then still write the
   script files under `~/.tabsmith/shell/`.

`uninstall [-y] [--purge]`: without `-y` ask `Are you sure want to continue uninstalling tabsmith? [Y/n] `
(No → `Cancelled`, exit 1). Then uninstall all dotfile integrations (all shells, ignoring errors per file, printing
them), `--purge` removes `~/.tabsmith` afterwards; print `Uninstalled tabsmith.` and
`Remove the binary with your package manager, or delete <current_exe>.` (a running exe cannot delete itself on Windows).

### 2.10 Other commands

- `version` → `tabsmith <ver>\n`.
- `diagnostic [-f]` (SHOULD): Plain = hand-written TOML (`[tabsmith] version, os`, `[environment] cwd, cli-path,
  shell-path, shell-version, terminal (TERM_PROGRAM/WT_SESSION/TERM), in-ssh, in-ci, in-wsl`, `[env-vars]` = all
  `TABSMITH_*` plus `SHELL, TERM, COLORTERM, ZDOTDIR, PATH`, with `/<username>` replaced by `/USER`); json = same object.
- `completion [SHELL]` (SHOULD): `clap_complete::generate(shell, Cli::command(), "tabsmith", stdout)`.
- `debug key-tester` (SHOULD): raw mode; each read prints `<n> bytes : "<escape_ascii>" : [0x..,...]`; Ctrl-D (0x04) prints `ctrl-d` and exits (Kiro `debug/mod.rs:597-631`).
- `debug shell [SHELL]` (SHOULD): run `_ pty` with a minimal config (Kiro `debug/mod.rs:816-870`): bash
  `--noprofile --rcfile <tmp>` (tmp = `eval "$(tabsmith init bash post)"`), zsh with `ZDOTDIR=<tmpdir>` whose `.zshrc`
  is `eval "$(tabsmith init zsh post)"`, fish `--no-config -C 'tabsmith init fish post | source'`, pwsh
  `-NoProfile -NoExit -Command ". <tmp.ps1>"`. Env `TABSMITH_DEBUG_SHELL=1`. Prints `Starting tabsmith debug shell` / `Ending tabsmith debug shell`.
- `debug logs` (SHOULD): print the log file paths and the last 50 lines of `pty.log`.

### 2.11 Internal `_` commands

| cmd | Behaviour | Used by |
|---|---|---|
| `_ pty [--shell PATH] [-- CMD...]` | The wrapper (§4.4). | pre scripts, `debug shell` |
| `_ should-launch` | Runs §5.2; prints one reason line (`✅ …`/`❌ …`) to stdout; exit 0 or 1. | fish/pwsh scripts, manual debugging |
| `_ get-shell` | Print parent process exe path (no newline); exit 1 if unknown. Linux `readlink /proc/<ppid>/exe`; macOS `libc::proc_pidpath(ppid)`; Windows: not used (pwsh passes its own path). | `init` (pre), scripts |
| `_ pre-cmd [--alias TEXT]` | If `TABSMITH_SESSION_ID` unset → exit 1. Write `~/.tabsmith/run/<sid>.json` = `{"env":{<all env vars>},"alias":TEXT,"ts":<unix ms>}` atomically (temp + rename), mode 0600, dir 0700. Exit 0. | post scripts, every prompt, backgrounded |

---

## 3. Settings, state, files

### 3.1 Paths

`TABSMITH_HOME` if set, else `$HOME/.tabsmith` (Unix) / `%USERPROFILE%\.tabsmith` (Windows); error if neither
variable exists. Optional `TABSMITH_SETTINGS_PATH` overrides only `settings.json` (tests).

```text
~/.tabsmith/
  settings.json            0600  user settings (flat dotted keys)
  state.json               0600  internal state (recency index, doctor flags)
  history.jsonl            0600  command history
  shell/<rc>.<pre|post>.<shell>  integration script files sourced from dotfiles (§5.5)
  themes/<name>.json             custom themes
  specs/<ver>/…                  extracted embedded specs (read-only)
  specs/user/…                   user spec overrides (same layout as the npm build/ dir)
  run/<session>.json       0600  (dir 0700) env + aliases from `_ pre-cmd`; deleted when the wrapper exits
  logs/cli.log, logs/pty.log     append; one line per record: `<rfc3339-ish unix ms> <level> <sid|-> <msg>`
  backups/<unix-secs>/<file>     dotfile backups before every edit
```

### 3.2 Storage format and semantics (port of `R:cli-surface §4.2`)

- **One flat JSON object, keys are literal dotted strings** (`"autocomplete.theme": "dark"`), values arbitrary JSON.
- Load: missing file → `{}` (created). Parse error → error (CLI exit 1; the wrapper logs and uses defaults).
- Save: read-modify-write; write `serde_json::to_string_pretty` (keys sorted: `serde_json::Map` without
  `preserve_order` is a BTreeMap) to `<file>.tmp-<pid>` then rename; Unix mode 0600. Concurrency: last writer wins
  (ponytail: Kiro used an fd lock; add `fs::File::lock` (std ≥ 1.89) if concurrent CLI writes ever matter).
- Typed getters follow Kiro: `get_bool` = `Value::as_bool` (the string `"true"` is not true), `get_string` = `as_str`,
  `get_i64` = `as_i64`; wrong type → default.
- The wrapper re-reads `settings.json` whenever its mtime changed, checked at every `NewCmd` and every 2 s while idle,
  then forwards the new map to the engine thread.
- `state.json`: same format. Keys: `autocomplete.recency` (`{cmd: {name: unix_ms}}`), `doctor.prompt-restart-terminal` (bool).

### 3.3 Settings catalog (`settings list --all` prints this table)

Autocomplete (Kiro keys, `R:autocomplete-ui §12`, `R:cli-surface §5.1`):

| key | type | default | semantics |
|---|---|---|---|
| `autocomplete.disable` | bool | false | Never show the dropdown (ghost text unaffected). |
| `autocomplete.insertSpaceAutomatically` | bool | true | Append a space after an inserted subcommand/option whose `shouldAddSpace` is true (§6.8). |
| `autocomplete.immediatelyExecuteAfterSpace` | bool | false | With an empty search term, prepend `↪ Immediately execute` row. |
| `autocomplete.scrollWrapAround` | bool | false | Up/Down wrap around the list. |
| `autocomplete.alwaysSuggestCurrentToken` | bool | false | Prepend an auto-execute row with the typed token ("Enter the current argument"); arg `suggestCurrentToken` overrides. |
| `autocomplete.fuzzySearch` | bool | false | Fuzzy instead of prefix filtering when the spec's `filterStrategy` is unset/`default`. Re-read whenever the argument changes. |
| `autocomplete.preferVerboseSuggestions` | bool | false | Insert the longest alias of an option/subcommand (`--message` over `-m`). |
| `autocomplete.onlyShowOnTab` | bool | false | Menu never auto-opens; Tab opens it; hides auto-execute rows. |
| `autocomplete.disableForCommands` | string[] | [] | Spec names never loaded. |
| `autocomplete.sortMethod` | `"most recent"`\|`"alphabetical"` | `"most recent"` | Recency boost unless `alphabetical`. |
| `autocomplete.scriptTimeout` | number ms | 5000 (Windows 20000) | Generator script timeout floor. |
| `autocomplete.immediatelyRunDangerousCommands` | bool | false | Allow the auto-execute row for `isDangerous` exact matches. |
| `autocomplete.firstTokenCompletion` | bool | false | Complete the command name itself (§6.9). |
| `autocomplete.hideAutoExecuteSuggestion` | bool | false | Never show auto-execute rows. |
| `autocomplete.navigateToHistory` | bool | false | Up at index 0 toggles history mode instead of hiding. |
| `autocomplete.alwaysShowDescription` | bool | false | Description pop-out permanently on; `toggleDescription` disabled. |
| `autocomplete.theme` | string | `"dark"` | `dark`, `light`, `system`, or a file stem in `~/.tabsmith/themes`. |
| `autocomplete.width` | number (columns) | 40 | Menu width, clamped to terminal width (D14). |
| `autocomplete.height` | number (rows) | 7 | Max visible list rows (D14). |
| `autocomplete.developerMode` | bool | false | Load specs from `devCompletionsFolder` first; disable spec/parse caches. |
| `autocomplete.devCompletionsFolder` | string\|null | null | Dev spec folder. |
| `autocomplete.keybindings.<keystroke>` | string | – | Action id or `"ignore"` (§7.2). |
| `beta.history.mode` | `"off"`\|`"history_only"`\|`"show"` | `"off"` | History rows in the dropdown (§6.10). |

Dropped Kiro keys (no terminal meaning or unused upstream): `autocomplete.fontFamily`, `fontSize`, `iconTheme`,
`hidePreviewWindow`, `immediatelyRunGitAliases` (never read, `R:autocomplete-ui §12`), `developerModeNPM`,
`developerModeNPMInvalidateCache`, `devCompletionsServerPort`, `personalShortcutsToken`, `history.disableLoading`,
`beta.autocomplete.auto-cache`, `beta.history.customCommand`, `beta.history.allShells`, `developer.*`, `qterm.*`, `ssh.*`.

Inline (new; Kiro had only `inline.enabled` and env vars `Q_INLINE_SHELL_COMPLETION_*`):

| key | type | default | semantics |
|---|---|---|---|
| `inline.enabled` | bool | true | Ghost text on/off. Live (mtime reload). |
| `inline.source` | string | `"history+specs"` | `history` \| `specs` \| `history+specs` \| `<profile name>`. Built-in names are reserved. |
| `inline.profiles.<name>` | object | – | `{provider:"openai"\|"anthropic", model:string (required), base_url?:string, api_key_env?:string, max_tokens?:int, effort?:string}`. Defaults: openai `base_url=https://api.openai.com/v1`, `api_key_env=OPENAI_API_KEY`, `max_tokens=64`; anthropic `base_url=https://api.anthropic.com`, `api_key_env=ANTHROPIC_API_KEY`, `max_tokens=1024`. `effort` (anthropic only) is sent as `output_config.effort`. **API keys are never stored in settings.** |
| `inline.debounceMs` | number | 300 | Quiet time before an LLM request (Kiro `Q_INLINE_SHELL_COMPLETION_DEBOUNCE_MS`). |
| `inline.historyCount` | number | 49 | History lines sent to the LLM (Kiro default). |

### 3.4 Environment variables

| var | meaning |
|---|---|
| `TABSMITH_HOME` | Data/config dir override. |
| `TABSMITH_SETTINGS_PATH` | settings.json override (tests). |
| `TABSMITH_TERM` | Set by the wrapper for the child = wrapper version. "Already inside tabsmith". |
| `TABSMITH_TERM_TMUX` | Set for the child (= version) iff `TMUX` was set; lets each tmux pane get its own wrapper. |
| `TABSMITH_SESSION_ID` | 32 lowercase hex chars, generated by the wrapper (`format!("{:016x}{:016x}", nanos_since_epoch, pid ^ nanos.rotate_left(17))`; no uuid crate). |
| `TABSMITH_SHELL` | Shell exe the wrapper spawns; removed from the child env. |
| `TABSMITH_IS_LOGIN_SHELL` | `1` → child gets `--login` (bash, zsh and fish all accept it); removed from child env. |
| `TABSMITH_FORCE_LAUNCH` / `TABSMITH_TERM_DISABLED` | Force / forbid launching the wrapper. |
| `PROCESS_LAUNCHED_BY_TABSMITH` | Set on every process tabsmith spawns (generators, fallback shell). |
| `TABSMITH_LOG_LEVEL` | `error\|warn\|info\|debug\|trace` (default `warn`). |
| `TABSMITH_INIT_SNAPSHOT_TEST` | Deterministic `init` output. |
| `TABSMITH_DEBUG_SHELL` | Set by `debug shell`. |
| read: `TMUX`, `INSIDE_EMACS`, `TERM_PROGRAM`, `__CFBundleIdentifier`, `__PWSH_LOGIN_CHECKED`, `CI`, `VSCODE_RESOLVING_ENVIRONMENT`, `TERM`, `COLORTERM`, `WT_SESSION`, `EDITOR`, `ZDOTDIR`, `XDG_CONFIG_HOME`, `SHELL`, `HOME`, `USERPROFILE`, `PATH`, `PATHEXT` | see §5.2, §7.5, §5.5 |

### 3.5 History file

`~/.tabsmith/history.jsonl`, one JSON object per line:
`{"command":str,"shell":str,"pid":int|null,"session_id":str,"cwd":str|null,"start_time":unix_s,"end_time":unix_s|null,"exit_code":int|null,"hostname":str|null}`
(Kiro's `history` table fields, `R:figterm §16`). Appended by the engine thread on `CommandFinished` (§4.3). Only
non-empty commands. Loaded at engine start (last 5000 lines kept in memory); when the file exceeds 10 000 lines it
is rewritten with the last 5000 (temp + rename).

---

## 4. Architecture

### 4.1 Modules (single crate, `src/`)

| file | responsibility |
|---|---|
| `main.rs` | clap tree, dispatch, small commands (`settings`, `theme`, `inline`, `version`, `completion`, `debug`, `_ get-shell`, `_ pre-cmd`, `_ should-launch`), logger (≈20 lines, no crate). |
| `settings.rs` | Paths, flat-JSON store (settings/state), typed getters, catalog table, wildcard matcher. |
| `integrations.rs` | `init` generation, should-launch decision, dotfile install/uninstall/status, embedded scripts (`include_str!("../shell/*")`), setup/uninstall flows. |
| `doctor.rs` | Checks and the run loop. |
| `pty.rs` | Wrapper: spawn, raw mode / console modes, threads, main loop, OSC splitter, marker tracker + cell tagging, edit-buffer extraction, key decoding, insertion/reconciliation, resize, Windows deferral, fallback. |
| `menu.rs` | Pure UI state: visibility machine, actions, keybinding table + parser, selection/scroll, common prefix. |
| `render.rs` | Themes, colour conversion, layout, frame bytes (menu, description, ghost), restore spans. |
| `shell_parser.rs` | Port of qac `shell-parser` (§6.1). |
| `parser.rs` | Spec model + normalisation, spec resolution/loading/versioning, argv state machine (§6.2–6.5). |
| `suggest.rs` | Suggestion assembly, generators/triggers/cache, filtering, fuzzysort, ranking, insertion text (§6.6–6.10). |
| `js.rs` | QuickJS host: prelude, module wrapping, function table, exec host, event pump (§9). |
| `inline.rs` | History store, ghost sources, LLM clients, completion cache, validation (§8). |
| `shell/` | `pre.sh`, `post.zsh`, `post.bash`, `pre.fish`, `post.fish`, `pre.ps1`, `post.ps1`. |
| `assets/` | `specs.tar.zst`, `SPECS_LICENSE` (MIT, Hercules Labs) + `THIRD_PARTY` (semver ISC, yaml ISC, strip-json-comments MIT). |

### 4.2 Crates (versions to verify at pin time)

| crate | version | use |
|---|---|---|
| `clap` (derive) | 4.5 | CLI. |
| `clap_complete` | 4.5 | `completion` (SHOULD). |
| `serde` (derive), `serde_json` | 1 | Settings, specs, history, IPC files. |
| `portable-pty` | 0.9 (verify) | PTY: `native_pty_system().openpty(PtySize)`, `slave.spawn_command(CommandBuilder)`, `master.try_clone_reader()`, `master.take_writer()`, `master.resize()`. ConPTY on Windows; verify side-loaded `conpty.dll` support and flags. |
| `alacritty_terminal` | 0.25 (verify) | Headless model: `Term::new(Config{scrolling_history: 2000,..}, &TermSize, VoidListener)`, `vte::ansi::Processor::advance(&mut term, &[u8])`, `term.grid()`, `TermMode::{ALT_SCREEN, SHOW_CURSOR, ORIGIN, INSERT, BRACKETED_PASTE}`, `Flags::{BOLD,DIM,ITALIC,UNDERLINE,INVERSE,HIDDEN,STRIKEOUT,WRAPLINE,WIDE_CHAR,WIDE_CHAR_SPACER}`, `Cell::hyperlink()` (verify OSC 8 is stored per cell; fallback: vendor the crate and add a `tag: u32` to `Cell` exactly like figterm's `FigFlags`). |
| `crossterm` | 0.29 | `terminal::{enable_raw_mode, disable_raw_mode, size}` only. Never its event reader (loses raw bytes). |
| `signal-hook` | 0.3 | Unix SIGWINCH/SIGTERM/SIGHUP thread. |
| `libc` | 0.2 | Unix: `setsid` in generator `pre_exec`, `geteuid`, `getppid`, macOS `proc_pidpath`. |
| `windows-sys` | 0.59 | Windows: console modes, `ReadConsoleInputW`, `GetConsoleScreenBufferInfo`, CP_UTF8, version. |
| `rquickjs` | 0.14 (verified locally: `Ctx::promise()`, `Promise::state()/result()`, `Runtime::{is_job_pending, execute_pending_job, set_interrupt_handler, set_memory_limit}`, `Ctx::eval`, `Persistent`) | Spec host; no `loader`/`futures` features. |
| `zstd` | 0.13 | Spec archive. |
| `tar` | 0.4 | Spec archive. |
| `ureq` | 3 (verify; 2.12 API differs) | LLM HTTPS (rustls). |
| `unicode-width` | same major as alacritty_terminal's | Display width for layout, truncation, ghost clipping. |

### 4.3 Threads and data flow

```text
 real terminal ── stdin bytes ──► [stdin thread] ──Msg::Stdin──┐   unix: blocking read(0)
 real terminal ◄── stdout (pty bytes minus markers + overlay frames) ── [main]
                                                               │   windows: ReadConsoleInputW; KEY_EVENT (bKeyDown)
 [signals thread] (unix SIGWINCH/TERM/HUP) ──Msg::Resize/Term──┤   uChar → UTF-8 bytes (join surrogate pairs);
 [stdin thread, windows] WINDOW_BUFFER_SIZE_EVENT + 250 ms size poll ──Msg::Resize──┤
 [pty reader] master.read(16 KiB) ──Msg::Pty / Msg::PtyEof────►│ main thread: owns Term model, Tracker,
 [child wait]  child.wait() ──Msg::ChildExit(code)────────────►│ MenuState, Renderer, settings snapshot
 [engine] ──Msg::Completion / Msg::Ghost──────────────────────►│
                                                               │──EngineMsg::{Complete, CommandFinished,
 [pty writer] ◄── WriterMsg(bytes) (unbounded) ── main          │   RecordInsert, Llm, Settings, Session}──► [engine]
     └─► master writer (blocking writes never stall main)      
 [engine]: owns QuickJS runtime (!Send), spec cache, generator states/caches, recency, history, inline cache.
     spawns [exec workers] (one per command, timeout) and [llm worker] (one per request);
     they report back on the engine's own channel (EngineMsg::ExecDone / Timer).
```

Messages (all `std::sync::mpsc`, unbounded):

```rust
enum Msg { Stdin(Vec<u8>), Pty(Vec<u8>), PtyEof, Resize(u16,u16), ChildExit(i32), Term,
           Completion(Completion), Ghost{req: u64, text: Option<String>} }
enum EngineMsg {
  Complete(Request), CommandFinished(HistoryEntry), RecordInsert{cmd: String, name: String},
  Llm{req: u64, buffer: String, cwd: PathBuf, shell: Shell}, Settings(Arc<Map>), Session(SessionCtx),
  ToggleFuzzy, ExecDone{id: u64, out: Result<ExecOutput,String>}, Timer{id: u64} }
struct Request { id: u64, buffer: String, cursor: usize /*byte*/, raw_cursor_char_is_space: bool,
                 cwd: PathBuf, shell: Shell, history_mode: bool }
struct Completion {
  req: u64, phase: Phase /* Parse | Generators */, error: bool,
  tokens: Vec<String>, command_text: String, command_index: usize,
  arg_key: (usize, usize),          // identity of currentArg and completionObj (Rc pointer addresses)
  current_arg: Option<(String /*name*/, String /*description*/)>,
  search_term: String, triggered_generators: Vec<GenKey>,
  items: Vec<Item>,                 // filtered, ranked, insertion precomputed (§6.8)
  loading: bool, ghost: Option<String> }
```

Main loop (pseudo-code):

```text
loop {
  timeout = min(resize_deadline, insertion_lock_deadline, llm_debounce_deadline, spinner_tick, settings_poll)
  msg = rx.recv_timeout(timeout)
  match msg:
    Stdin(b)      → handle_input(b)                                  // §7.2
    Pty(b)        → batch = b ++ drain all queued Pty (try_recv); handle_output(batch)   // §4.5
    Resize(c,r)   → master.resize; term.resize; renderer.drop_clear_cache(); resize_deadline = now+150ms
    Completion(c) → menu.on_completion(c); render()                  // §7.1
    Ghost{req,t}  → if req == latest_llm_req && buffer unchanged: ghost = t; render()
    ChildExit(c)  → exit_code = c; (Windows: drop master to unblock reader)
    PtyEof        → if exit_code known or child gone: break
    Term          → break
    Timeout       → fire due timers (resize repaint §7.6, lock expiry §7.3, LLM request §8.4, spinner frame)
}
cleanup(); exit(exit_code)
```

### 4.4 `_ pty` startup, exit, fallback

1. If stdin or stdout is not a tty → fallback.
2. `shell = --shell | $TABSMITH_SHELL | $SHELL (unix) | "pwsh" (windows)`; error if none → fallback.
3. Load settings (errors → defaults). Open `logs/pty.log`.
4. Raw mode: Unix `crossterm::terminal::enable_raw_mode()` (TCSANOW; cfmakeraw ⇒ ISIG off, so Ctrl-C/Z become bytes).
   Windows: stdin mode `(old & !(ENABLE_ECHO_INPUT|ENABLE_LINE_INPUT|ENABLE_PROCESSED_INPUT)) | ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_WINDOW_INPUT`;
   stdout mode `old | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN`; `SetConsoleCP/SetConsoleOutputCP(CP_UTF8)`; all four restored on exit.
5. **Sync model with the real cursor** (ish clears the screen to sync, `ish/src/ui/ui-root.ts:42`): if theme is
   `system`, write `ESC ] 11 ; ? ESC \`; then write `ESC [ 6 n`; read stdin for up to 300 ms collecting replies
   (`ESC]11;rgb:RRRR/GGGG/BBBB` + BEL/ST, and `ESC [ row ; col R`). Any other bytes read are kept and forwarded to the
   PTY after spawn. With a CPR: feed the **model only** `ESC [ row ; col H`. Without: write `ESC[H ESC[2J` to stdout
   (model already at 1;1). Rows above the cursor are blank in the model (content printed before tabsmith started is
   not restorable if the menu ever covers it — cosmetic).
6. Session id; child env = current env + `TABSMITH_TERM=<ver>`, `TABSMITH_TERM_TMUX=<ver>` iff `TMUX`,
   `TABSMITH_SESSION_ID=<sid>`; remove `TABSMITH_SHELL`, `TABSMITH_IS_LOGIN_SHELL`. `TERM` inherited (not set).
7. argv: `[shell] + (["--login"] if TABSMITH_IS_LOGIN_SHELL=1 && shell ∈ {bash,zsh,fish}) + extra args after --`, or the
   explicit `-- CMD` if given. cwd = current dir (fallback `$HOME`). Open PTY at `crossterm::terminal::size()`.
8. Write `ESC ] 0 ; <shell basename> BEL` (tab title, Kiro `figterm/src/main.rs:608-616`).
9. Spawn threads (§4.3). Main loop.
10. Exit: write cached clear frame if a menu is drawn, `ESC[0m ESC[?25h`, restore raw/console modes, delete
    `run/<sid>.json`, exit with the child's code.

**Fallback** (any error before step 9): restore terminal state, log, print `tabsmith had an error: <err>` to stderr,
then run the plain shell with `TABSMITH_TERM_DISABLED=1` (so its pre block does not loop): Unix `execvp` via
`CommandExt::exec`; Windows spawn with inherited stdio, wait, exit with its code. Kiro does the same
(`figterm/src/main.rs:453-476`). **Never** leave the user without a shell (Kiro's `qterm.enabled=false` bug, `R:figterm §19.1`).

Signals (Unix): SIGTERM/SIGHUP → cleanup + exit 129; SIGWINCH → `Msg::Resize`. SIGINT never arrives (raw mode).

### 4.5 Output path (per PTY batch)

```text
handle_output(batch):
  out = Vec::new()
  out += renderer.take_clear()                 // restores every cell span the last frame covered (computed at draw time)
  for piece in splitter.feed(batch):           // §5.1.1, stateful across batches
     Bytes(b)   → processor.advance(&mut term, b); out += strip_kbd_modes(b)   // §7.5
     Marker(p)  → tracker.on_marker(p, &mut term, &mut processor)              // may inject model-only bytes
  tracker.end_of_batch(&mut term)              // Windows deferred markers (§5.1.4)
  if insertion_lock active: check release (§7.3)
  if !locked && tracker.at_prompt(): (buf, cur) = extract_buffer(term) (§5.6);
        if changed since last request → menu.on_buffer(buf, cur) → maybe send EngineMsg::Complete
  out += renderer.frame(&term, &menu, ghost)   // §7.5; empty if nothing to draw
  stdout.write_all(out); flush                  // ONE write per batch
```

Windows: keep a ≤3-byte carry so a UTF-8 char split across PTY reads is never written partially (verify std console
behaviour). Bare LF stays LF because `DISABLE_NEWLINE_AUTO_RETURN` is set; if `SetConsoleMode` fails with that flag,
rewrite bare `\n` (not preceded by `\r`) to `ESC D` in `out` (ish `stdioProxy.ts`).

---

## 5. Shell integration

### 5.1 Protocol: OSC 697

Wire format `ESC ] 6 9 7 ; <payload> (BEL | ESC \)`. Payload = all bytes up to the terminator (no `;` splitting,
no escaping). A payload is either a bare token or `Key=Value` split at the **first** `=`.

| payload | lock-guarded | effect in tracker |
|---|---|---|
| `StartPrompt` | yes | `has_seen_prompt = true`; inject `ESC]8;;tabsmith:p ESC\` into the model (prompt tag on). |
| `EndPrompt` | yes; Windows: deferred | If a command cycle is open (NewCmd seen, no PreExec since) inject `ESC]8;;tabsmith:c<n> ESC\`, else inject `ESC]8;; ESC\`. |
| `NewCmd=<sid>` / `NewCmd` | no (sid check); Windows: deferred | If sid non-empty and ≠ ours → ignore (nested session). Else: `n += 1`; `preexec = false`; inject `ESC]8;;tabsmith:c<n> ESC\`; if a pending command exists → send `CommandFinished` (§3.5) to the engine; reset menu state (empty buffer error-reset §7.1); re-check settings mtime. |
| `PreExec` | no (once per cycle) | Snapshot `extract_buffer().0.trim()` as pending command (`start_time=now`, cwd, shell, pid); `preexec = true`; inject `ESC]8;; ESC\`; hide menu + ghost; cancel insertion lock. |
| `OSCLock=<sid>` | – | If sid == ours → `osc_lock = true`. |
| `OSCUnlock=<sid>` | – | If sid == ours → `osc_lock = false`. |
| `Dir=<path>` | yes | `cwd = path` (used for generators; tabsmith does **not** `chdir` itself). |
| `ExitCode=<i32>` | yes | pending command `exit_code`, `end_time=now`. |
| `Shell=<name>` | yes | `shell`. |
| `ShellPath=<path>`, `PID=<i32>`, `TTY=<path>`, `User=<name>` | yes (`User`: no) | context. |
| `ZshAutosuggestionColor=<zle spec>` | yes | suggestion colour (zsh). |
| `FishSuggestionColor=<fish spec>` | yes | suggestion colour (fish). |
| anything else (`Log=`, `DoneSourcing`, …) | – | ignored. |

Empty values (`Key=`) are ignored (Kiro, `ansi.rs:1175-1272`).

#### 5.1.1 Splitter (stateful, before the VT parser)

States `Ground`, `Esc`, `Osc(buf)`, `OscEsc(buf)`. `ESC ]` starts collecting; BEL or `ESC \` ends; `ESC` + other byte
in `Osc` → abandon (flush collected bytes as `Bytes`) and re-process. A completed OSC whose payload starts with
`697;` → `Piece::Marker(payload[4..])`, consumed (not forwarded, not fed to the model). Any other OSC → `Bytes`
(original bytes). An OSC buffer > 64 KiB is flushed as `Bytes`. Splitting at the exact byte offset matters: the
cursor position at `NewCmd` must be the one at that byte.

#### 5.1.2 Lock

`OSCLock/OSCUnlock` carry our session id so a nested shell (ssh, a manually started `bash`, a nested tabsmith) that
also emits 697 cannot overwrite our state (Kiro `term/mod.rs:1922-1949`). Known gap (inherited): a plain subshell
inherits `TABSMITH_SESSION_ID` and can unlock.

#### 5.1.3 Suggestion-colour specs

- zsh (`fg=8`, `fg=#aaaaaa,bg=…`, `fg=245,bold`, `fg=yellow`): comma list; `fg=`/`bg=` value is `0..255`, a name
  (`black red green yellow blue magenta cyan white`, optional `default`), or `#rrggbb`.
- fish (`555 brblack`, `brblack`, `--bold 666`): every non-option token is a colour: names incl. `br*` variants (8–15),
  hex `rgb`/`rrggbb` (optional `#`). fish renders the first one the terminal supports, so **any** listed colour matches.
- Match: convert both the cell fg (Named n→n, Indexed i→i, Spec rgb→nearest xterm-256) and each spec colour to an
  xterm-256 index; equal ⇒ match; a `bg=` in the spec must match the cell bg the same way.
- pwsh (no report): ghost cell ⇔ `DIM || ITALIC` (PSReadLine predictions, ish `commandManager.ts:106-122`).
- bash: none.

#### 5.1.4 Windows ConPTY deferral (port of `figterm/src/main.rs:583-584, 859-870`, `term/mod.rs:796-881`)

ConPTY may emit the OSC before the prompt text it belongs to. On Windows, `EndPrompt` and `NewCmd` (and their tag
injections) are queued instead of applied. At `end_of_batch`, if the queue is non-empty and **this batch added no
new 697 markers**, apply the queue in FIFO order (Kiro pops LIFO — bug, `R:figterm §19.13`) with the current cursor.

### 5.2 Launch decision (`_ should-launch`, also called in-process by `init … pre`)

First match wins; prints the reason line.

1. `TABSMITH_FORCE_LAUNCH` set → `✅ TABSMITH_FORCE_LAUNCH` → 0
2. `TABSMITH_TERM_DISABLED` → 1
3. `PROCESS_LAUNCHED_BY_TABSMITH` → 1
4. `TABSMITH_TERM` set: `TMUX` set and `TABSMITH_TERM_TMUX` unset → `✅ new tmux pane` → 0; else `❌ already inside tabsmith` → 1
5. `INSIDE_EMACS` → 1; `TERM_PROGRAM=WarpTerminal` → 1; `__CFBundleIdentifier=com.vandyke.SecureCRT` → 1;
   `__PWSH_LOGIN_CHECKED` → 1; `CI` → 1; `VSCODE_RESOLVING_ENVIRONMENT` (VS Code/Kiro/Cursor env probing,
   `ish/src/utils/shell.ts:330-372`) → 1; Unix `TERM` unset or `dumb` → 1
6. Unix: stdin not a tty → 1 (the scripts check stdout and interactivity themselves)
7. Windows: `GetConsoleMode(stdin)` fails → 1
8. → `✅ launching` → 0

### 5.3 `init` output

Lines joined by `\n`, then the script body with `{{BIN}}` replaced by the shell-quoted absolute path of
`current_exe()` (single quotes, `'` → `'\''` for POSIX/fish; `''` for pwsh).

| shell, when | header lines | body |
|---|---|---|
| bash/zsh pre | `TABSMITH_SHELL='<_ get-shell result>'` (omitted if unknown), `SHOULD_TABSMITH_LAUNCH=<0\|1>` | `shell/pre.sh` |
| bash post | – | `shell/post.bash` |
| zsh post | – | `shell/post.zsh` |
| fish pre | `set -g SHOULD_TABSMITH_LAUNCH <0\|1>` | `shell/pre.fish` |
| fish post | – | `shell/post.fish` |
| pwsh pre | `$TABSMITH_SHOULD_LAUNCH = <0\|1>` | `shell/pre.ps1` |
| pwsh post | – | `shell/post.ps1` |

### 5.4 Scripts (normative content; keep these semantics, cosmetics may vary)

All `printf` calls pass values as `%s` arguments (Kiro builds the format string from the value; a `%` in a path breaks it).

#### 5.4.1 `pre.sh` (bash, zsh)

```sh
if [ "${SHOULD_TABSMITH_LAUNCH:-1}" = 0 ] && [ -t 0 ] && [ -t 1 ] \
   && [ -z "${BASH_EXECUTION_STRING:-}${ZSH_EXECUTION_STRING:-}" ]; then
  case $- in *i*)
    if [ -n "${BASH_VERSION:-}" ] && { [ "${BASH_VERSINFO[0]}" -lt 4 ] || { [ "${BASH_VERSINFO[0]}" -eq 4 ] && [ "${BASH_VERSINFO[1]}" -lt 4 ]; }; }; then
      :   # bash < 4.4 unsupported (D15)
    else
      TABSMITH_IS_LOGIN_SHELL=0
      if { [ -n "${BASH_VERSION:-}" ] && shopt -q login_shell; } || [[ -n "${ZSH_NAME:-}" && -o login ]]; then
        TABSMITH_IS_LOGIN_SHELL=1
      fi
      TABSMITH_SHELL="${TABSMITH_SHELL:-$SHELL}" TABSMITH_IS_LOGIN_SHELL="$TABSMITH_IS_LOGIN_SHELL" \
        exec -a "${TABSMITH_SHELL##*/} (tabsmith)" {{BIN}} _ pty
    fi ;;
  esac
fi
```

`exec` replaces the shell (same pid, same tty); argv[0] `zsh (tabsmith)` keeps tab titles sensible (Kiro `pre.sh:1-77`).

#### 5.4.2 `post.zsh`

```zsh
if [[ -n "${ZSH_NAME:-}" && -n "${TABSMITH_SESSION_ID:-}" && -z "${_TS_POST_LOADED:-}" ]]; then
  _TS_POST_LOADED=1
  _ts_osc() { builtin printf '\e]697;%s\a' "$1"; }
  _ts_restore() {
    if [[ ${_ts_wrapped:-0} == 1 ]]; then
      PS1=$_ts_u1; PS2=$_ts_u2; RPS1=$_ts_ur1; RPS2=$_ts_ur2; _ts_wrapped=0
    fi
  }
  _ts_preexec() { _ts_restore; _ts_osc "OSCLock=$TABSMITH_SESSION_ID"; _ts_osc PreExec; }
  _ts_precmd() {
    local st=$?
    _ts_restore                                   # Ctrl-C at the prompt: preexec never ran
    _ts_osc "OSCUnlock=$TABSMITH_SESSION_ID"; _ts_osc "Dir=$PWD"; _ts_osc "Shell=zsh"
    _ts_osc "ShellPath=${TABSMITH_SHELL:-$SHELL}"; _ts_osc "PID=$$"; _ts_osc "ExitCode=$st"
    _ts_osc "TTY=$TTY"; _ts_osc "User=${USER:-root}"
    (( ${+functions[_zsh_autosuggest_accept]} )) && _ts_osc "ZshAutosuggestionColor=${ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE:-fg=8}"
    _ts_u1=$PS1 _ts_u2=$PS2 _ts_ur1=$RPS1 _ts_ur2=$RPS2
    local sp=$'%{\e]697;StartPrompt\a%}' ep=$'%{\e]697;EndPrompt\a%}'
    local nc=$'%{\e]697;NewCmd='"$TABSMITH_SESSION_ID"$'\a%}'
    PS1="$sp$PS1$ep$nc"; PS2="$sp$PS2$ep"
    [[ -n $RPS1 ]] && RPS1="$sp$RPS1$ep"; [[ -n $RPS2 ]] && RPS2="$sp$RPS2$ep"
    _ts_wrapped=1
    _ts_reset_hooks
    ( {{BIN}} _ pre-cmd --alias "$(builtin alias)" >/dev/null 2>&1 & )
  }
  _ts_reset_hooks() {   # precmd LAST (wraps the final PS1), preexec FIRST — re-asserted every prompt
    [[ ${precmd_functions[-1]} == _ts_precmd ]] || precmd_functions=(${precmd_functions:#_ts_precmd} _ts_precmd)
    [[ ${preexec_functions[1]} == _ts_preexec ]] || preexec_functions=(_ts_preexec ${preexec_functions:#_ts_preexec})
  }
  _ts_reset_hooks
fi
```

`PROMPT`/`prompt` are aliases of `PS1` in zsh; PS3/PS4 are not wrapped (Kiro wraps them; PS4 wrapping pollutes `set -x`).

#### 5.4.3 `post.bash` (bash ≥ 4.4)

```bash
if [ -n "${BASH_VERSION:-}" ] && [ -n "${TABSMITH_SESSION_ID:-}" ] && [ -z "${_TS_POST_LOADED:-}" ]; then
  _TS_POST_LOADED=1
  _ts_osc() { builtin printf '\e]697;%s\a' "$1"; }
  : "${TTY:=$(tty 2>/dev/null)}"
  _ts_first() {
    _ts_st=$?
    _ts_osc "OSCUnlock=$TABSMITH_SESSION_ID"; _ts_osc "Dir=$PWD"; _ts_osc "Shell=bash"
    _ts_osc "ShellPath=${TABSMITH_SHELL:-$BASH}"; _ts_osc "PID=$$"; _ts_osc "ExitCode=$_ts_st"
    _ts_osc "TTY=$TTY"; _ts_osc "User=${USER:-root}"
    if [ "${_ts_wrapped:-0}" = 1 ]; then
      [ "$PS1" = "$_ts_last1" ] && PS1=$_ts_u1        # keep a PS1 the user changed meanwhile
      [ "$PS2" = "$_ts_last2" ] && PS2=$_ts_u2
      _ts_wrapped=0
    fi
    ( {{BIN}} _ pre-cmd --alias "$(builtin alias)" >/dev/null 2>&1 & )
    return $_ts_st                                     # user PROMPT_COMMANDs still see $?
  }
  _ts_last() {
    local st=$?
    _ts_u1=$PS1; _ts_u2=$PS2
    PS1="\[\e]697;StartPrompt\a\]$PS1\[\e]697;EndPrompt\a\e]697;NewCmd=$TABSMITH_SESSION_ID\a\]"
    PS2="\[\e]697;StartPrompt\a\]$PS2\[\e]697;EndPrompt\a\]"
    _ts_last1=$PS1; _ts_last2=$PS2; _ts_wrapped=1
    _ts_install
    return $st
  }
  _ts_install() {   # PROMPT_COMMAND = _ts_first ; <user commands> ; _ts_last   (string or bash 5.1 array)
    if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
      [[ ${PROMPT_COMMAND[0]} == _ts_first && ${PROMPT_COMMAND[-1]} == _ts_last ]] && return
      local pc=() c; for c in "${PROMPT_COMMAND[@]}"; do [[ $c == _ts_first || $c == _ts_last ]] || pc+=("$c"); done
      PROMPT_COMMAND=(_ts_first "${pc[@]}" _ts_last)
    else
      [[ $PROMPT_COMMAND == _ts_first\;*_ts_last ]] && return
      local pc=${PROMPT_COMMAND:-}; pc=${pc#_ts_first;}; pc=${pc%_ts_last}; pc=${pc%;}
      PROMPT_COMMAND="_ts_first;${pc:+$pc;}_ts_last"
    fi
  }
  PS0="\e]697;OSCLock=$TABSMITH_SESSION_ID\a\e]697;PreExec\a${PS0:-}"   # bash 4.4+ prints PS0 before running a command
  _ts_install
fi
```

#### 5.4.4 `pre.fish` / `post.fish` (installed as whole files in `conf.d`, §5.5)

`pre.fish`:

```fish
if test "$SHOULD_TABSMITH_LAUNCH" = 0; and status is-interactive; and isatty stdin; and isatty stdout
    set -l login 0; status is-login; and set login 1
    set -gx TABSMITH_SHELL (status fish-path)
    set -gx TABSMITH_IS_LOGIN_SHELL $login
    exec {{BIN}} _ pty          # fish's exec has no -a; argv0 is cosmetic (Kiro goes through bash for it)
end
```

`post.fish` (port of Kiro `post.fish`, renamed): `_ts_osc` = `builtin printf '\e]697;%s\a' $argv[1]`;
`function _ts_preexec --on-event fish_preexec` → `OSCLock=<sid>`, `PreExec`, restore `fish_prompt`,
`fish_right_prompt`, `fish_mode_prompt` from their `_ts_user_*` copies. `function _ts_precmd --on-event fish_prompt`
→ `OSCUnlock`, `Dir=$PWD`, `Shell=fish`, `ShellPath=$TABSMITH_SHELL`, `PID=$fish_pid`, `ExitCode=$status` (captured
first), `TTY`, `User`, `FishSuggestionColor=$fish_color_autosuggestion`; then copy `fish_prompt` → `_ts_user_prompt`
(`functions --erase dst; functions --copy src dst`) and define `fish_prompt` = `StartPrompt` + output of
`_ts_user_prompt` (`printf '%b' (string join "\n" …)`) + `EndPrompt` + `NewCmd=<sid>`; `fish_right_prompt` and
`fish_mode_prompt` get `StartPrompt … EndPrompt` without `NewCmd`; finally
`{{BIN}} _ pre-cmd --alias (alias | string collect) &>/dev/null &`. Because wrapping happens on every `fish_prompt`
event, a `fish_prompt` defined later in `config.fish` is still captured. Whole file guarded by
`set -q TABSMITH_SESSION_ID; and not set -q _TS_POST_LOADED`.

#### 5.4.5 `pre.ps1` / `post.ps1` (pwsh)

`pre.ps1` (after the `$TABSMITH_SHOULD_LAUNCH = n` header):

```powershell
$__tsArgs = [Environment]::GetCommandLineArgs()
$__tsCmd = ($__tsArgs | Where-Object { $_ -imatch '^-(c|command|f|file)$' }).Count -gt 0
$__tsNoExit = ($__tsArgs | Where-Object { $_ -imatch '^-noe(xit)?$' }).Count -gt 0
if ($TABSMITH_SHOULD_LAUNCH -eq 0 -and [Environment]::UserInteractive -and
    -not [Console]::IsInputRedirected -and -not [Console]::IsOutputRedirected -and
    (-not $__tsCmd -or $__tsNoExit)) {
  $env:TABSMITH_SHELL = (Get-Process -Id $PID).Path
  & {{BIN}} _ pty --shell $env:TABSMITH_SHELL
  Stop-Process -Id $PID          # the outer shell ends with the wrapped one (ish pattern)
}
```

`post.ps1`:

```powershell
if ($env:TABSMITH_SESSION_ID -and -not $Global:__TsLoaded) {
  $Global:__TsLoaded = $true
  $Global:__TsUserPrompt = $function:prompt
  function Global:__TsOsc([string]$p) { "$([char]27)]697;$p$([char]7)" }
  function Global:prompt {
    $ok = $?; $code = $global:LASTEXITCODE
    $exit = if ($ok) { 0 } elseif ($code) { $code } else { 1 }
    $sid = $env:TABSMITH_SESSION_ID
    $o = (__TsOsc "OSCUnlock=$sid") + (__TsOsc "ExitCode=$exit") + (__TsOsc 'Shell=pwsh') +
         (__TsOsc "PID=$PID") + (__TsOsc "User=$([Environment]::UserName)")
    if ($PWD.Provider.Name -eq 'FileSystem') { $o += __TsOsc "Dir=$($PWD.ProviderPath)" }
    $o += (__TsOsc 'StartPrompt') + (& $Global:__TsUserPrompt) + (__TsOsc 'EndPrompt') + (__TsOsc "NewCmd=$sid")
    $global:LASTEXITCODE = $code
    return $o
  }
  if (Get-Module PSReadLine) {   # PreExec: PSReadLine calls this when a line is accepted (inferred; verify ordering)
    $Global:__TsPrevHist = (Get-PSReadLineOption).AddToHistoryHandler
    Set-PSReadLineOption -AddToHistoryHandler {
      param([string]$line)
      [Console]::Write((__TsOsc "OSCLock=$env:TABSMITH_SESSION_ID") + (__TsOsc 'PreExec'))
      if ($Global:__TsPrevHist) { return & $Global:__TsPrevHist $line } else { return $true }
    }
  }
}
```

pwsh does not run `_ pre-cmd` (process spawn per prompt is slow on Windows); generators use the wrapper's env plus `Dir=`.

### 5.5 Dotfile installation (port of `qac/crates/fig_integrations/src/shell/mod.rs`, with exact markers)

Integrations per shell:

| shell | config dir | integration objects |
|---|---|---|
| bash | `$HOME` | dotfile `.bashrc` always + each of `.profile`, `.bash_login`, `.bash_profile` that exists; if none exist, `.profile` (created) |
| zsh | `$ZDOTDIR` or `$HOME` | dotfiles `.zshrc`, `.zprofile` |
| fish | `$XDG_CONFIG_HOME/fish` or `~/.config/fish` | whole files `conf.d/00_tabsmith_pre.fish`, `conf.d/99_tabsmith_post.fish` |
| pwsh | `$PROFILE` (CurrentUserCurrentHost) from `pwsh -NoLogo -NoProfile -Command '$PROFILE'` (5 s timeout) | dotfile `<profile path>` |

Script files `~/.tabsmith/shell/<dotfile name without leading dot and extension>.<pre|post>.<shell>`
(`bashrc.pre.bash`, `zprofile.post.zsh`, `profile.pre.ps1`, …). Their contents (exact bytes, used by `is_installed`):

```text
bash:  [ -n "$BASH_VERSION" ] && [ -x '<BIN>' ] && eval "$('<BIN>' init bash <when> --rcfile <rc>)"\n
zsh:   [ -x '<BIN>' ] && eval "$('<BIN>' init zsh <when> --rcfile <rc>)"\n
pwsh:  if (Test-Path -LiteralPath '<BIN>') { Invoke-Expression ((& '<BIN>' init pwsh <when> --rcfile profile) -join "`n") }\n
fish conf.d file:  test -x '<BIN>'; and '<BIN>' init fish <when> --rcfile <file stem> | source\n
                   (stems: 00_tabsmith_pre, 99_tabsmith_post)
```

`<BIN>` is the absolute exe path at install time, so the rc works before the user's PATH is set up (rustup/brew
PATH lines usually come later in the rc). `doctor` detects a moved binary (script bytes differ) and reinstalls.

Dotfile blocks (`P` = script path with a leading `$HOME` written as `${HOME}`, else absolute):

```text
# tabsmith pre block. Keep at the top of this file.
[ -f "P" ] && . "P"                                         (bash, zsh — POSIX so .profile works under dash)
if (Test-Path "P") { . "P" }                                (pwsh)
...user content...
# tabsmith post block. Keep at the bottom of this file.
[ -f "P" ] && . "P"
```

Install (idempotent):

```text
install(): if is_installed().is_ok() { return Ok }
  if dotfile exists: copy to ~/.tabsmith/backups/<unix-secs>/<file name>; uninstall(); contents = read  else contents = ""
  original = contents
  write pre script file;  (shebang, rest) = split_shebang(contents)   // first line kept on top iff it starts with "#!"
  contents = shebang + PRE_MARKER + "\n" + PRE_SOURCE + "\n" + rest
  write post script file; contents = contents.trim_end_matches('\n') + "\n" + POST_MARKER + "\n" + POST_SOURCE + "\n"
  if contents != original: write dotfile (through symlinks; PermissionDenied → error "Permission denied: <path>")
```

`is_installed()` (first failing reason):
1. dotfile missing → `File does not exist: <path>`.
2. any line matching `^\s*#\s*tabsmith ignore` → Ok (user opt-out).
3. `lines` = non-blank lines not starting with `#` (after trim).
4. `lines.first() != PRE_SOURCE` → `<dotfile> does not source pre integration first` (or `… does not source pre integration` if absent anywhere); same for post with `lines.last()`.
5. each script file must exist with exact expected bytes → `File does not exist: <p>` / `<p> should contain:\n<contents>`.

Uninstall: remove every line equal to `PRE_MARKER`, `PRE_SOURCE`, `POST_MARKER`, `POST_SOURCE` (exact string compare
after `trim_end`); then `trim()` + `"\n"`; write; delete both script files (missing = ok). No regexes (Kiro's
`^#.*fig.*var.*` deletes unrelated comments, `R:shell-integration §7.6`). fish/whole-file integrations: delete the files.

Error display strings: `Improper integration installation: <msg>`, `Integration not installed: <msg>`,
`File does not exist: <path>`, `Permission denied: <path>`.

### 5.6 Edit buffer and cursor extraction (screen-scraped, cell tags)

Inputs: model grid, cursor `(cl, cc)` (viewport line, column), current cycle `n` (None while `preexec` or before the
first `NewCmd` → no buffer), shell, suggestion colour.

```text
is_cmd(cell, pos)   = cell.hyperlink_uri == "tabsmith:c<n>" && !is_ghost(cell, pos)
is_prompt(cell)     = cell.hyperlink_uri == "tabsmith:p"
is_ghost(cell, pos) = pos >= (cl, cc) && colour_matches(cell, suggestion_colour)   // D24; pwsh: DIM||ITALIC
wraps(r)            = last cell of row r has WRAPLINE

rows: start = cl; while start-1 >= topmost && cl-start < 100 &&
                  (wraps(start-1) || row(start-1) contains a tag-n cell): start -= 1
      end = cl;   while end+1 <= bottom && (wraps(end) ||
                                            (first cell of row(end+1) is_prompt && row(end+1) contains a tag-n cell)): end += 1
      // below the cursor only wrap continuations and PS2 continuation rows count; zsh completion listings do not
buf = "", cursor = None
for r in start..=end:
   pad = 0; row_has_cmd = false
   for c in 0..cols:
      cell = grid[r][c]; if cell has WIDE_CHAR_SPACER: continue
      if (r, c) == (cl, cc): cursor = Some(buf.len())          // padding before the cursor is not flushed
      if is_cmd(cell, (r,c)): buf += " " * pad; pad = 0; buf.push(cell.c); buf += cell.zerowidth; row_has_cmd = true
      else: pad += 1                                            // prompt, untagged blank, ghost, other cycles
   if (cl, cc) is past the last column of row r (pending wrap): cursor = Some(buf.len())
   if r < end && !wraps(r) && (row_has_cmd || buf non-empty): buf.push('\n')
return (buf, cursor.unwrap_or(buf.len()))
```

Properties (vs Kiro `R:figterm §10.2` quirks): cursor is a **byte offset into `buf`** but every count derived from it
uses chars; multi-line commands contain real `\n`; RPROMPT/PS2/ghost text excluded; text printed below the command
line excluded; scroll and reflow need no coordinate tracking.

---

## 6. Completion engine (engine thread)

Per request: `buffer[..cursor]` → tokenize → select command → expand aliases → parse against specs → assemble
suggestions → trigger generators → filter + rank → precompute insertions → reply (phase `Parse`); then await
triggered generators and reply again (phase `Generators`). Only the latest request is served: before starting and
whenever the event pump runs, a newer `Complete` aborts the current one.

### 6.1 Tokenizer (port of qac `packages/shell-parser`, ≈900 TS lines; full detail `R:parser-runtime §1`)

#### 6.1.1 Grammar (`SP/parser.ts:1-23`)

```text
terminator  = ";" | "&" | "&;"            literal = string | ansi_c_string | raw_string | expansion | simple_expansion | word
command     = (concatenation | literal)+   assignment = (word | subscript)("=" | "+=") literal
statement   = "{" (statement terminator)+ "}" | "(" statements ")" | statement ("||"|"&&"|"|"|"|&") statement
            | command | assignment_list
program     = statements
```

Node = `{type, text (= src[start..end]), inner_text (unquoted), start, end, complete, children}`.

#### 6.1.2 Rules (exact)

- Operators `; & | && || |& &;` always end a word, even without spaces (`a&&b` = two commands).
- Literal dispatch by lookahead: `$((`→arithmetic (no children, `))`), `$(`→command_substitution (recursive
  statements until `)`), `${`→expansion (until `}`), `$'`→ansi_c_string (not inside `"`), `$`+name→simple_expansion
  (`$*@?-$0_` → special, length 2; lone `$` → part of a word), `` ` ``→command_substitution, `'`→raw_string,
  `"`→string (children allowed). Unterminated literal → `complete=false`, extends to EOF.
- Inside a word, `\x` takes `x` literally (so `a\ b` is one word `a b`); inside `"…"` only `` \$ \` \" \\ \newline ``
  drop the backslash; raw/ansi-c strings keep text verbatim. End delimiter stripped from `inner_text` only if `complete`.
- Adjacent literals form one `concatenation` argument: `--foo="a b"c` is one token, inner text `--foo=a bc`.
- Whitespace (space/tab) ends an argument; newline does not split words (only terminates `$var`).
- Assignments: while the rest matches `^[\w\[\]]+\+?=`, parse `name(=|+=)value` (value ends at a space).
  **Fix:** empty value (`FOO= cmd`) is a complete empty assignment (Kiro throws).
- `{`/`(` start compound/subshell; `&&`/`||` → flattened `list`; `|`/`|&` → `pipeline` (binds tighter than lists).

#### 6.1.3 Current command and tokens (`SP/command.ts`)

- `descendant_at(tree, index=cursor, "command")`: deepest `command` node with `start <= index <= end`, children in
  order, first hit wins (commands inside `$( )`, backticks, subshells, pipelines, lists).
- `tokens = children.map(c → {text: c.inner_text, raw: c.text, start, end})`.
- Trailing empty token: if `(children.is_empty() ? 1 : last.end) < cmd.end && cmd.text.ends_with(' ')` push `{text:"", raw:"", start:end, end}`.
- Worked examples (tests):

| buffer | tokens |
|---|---|
| `git co ` | `git`, `co`, `""` |
| `ls \| gr` | `gr` |
| `ls \| ` | `""` |
| `ls \|` | none → no suggestions |
| `echo $(git ch` | `git`, `ch` |
| `cd my\ di` | `cd`, `my di` |
| `git commit -m "fix bu` | `git`, `commit`, `-m`, `fix bu` |
| `FOO=1 BAR=2 npm ` | `npm`, `""` |
| `(cd x && git ` | `git`, `""` |
| `FOO= git ` | `git`, `""` (fix) |
| ` ` (one space) | none |

#### 6.1.4 pwsh

Same parser with escape char `` ` `` instead of `\` and no `$'…'`; quoting rules for insertion differ (§6.8). (PowerShell
syntax is otherwise close enough for completion: `;`, `|`, `&&`, `||`, `'…'`, `"…"`.)

#### 6.1.5 Mid-word guard

If the char at `cursor` exists and is not whitespace → reply `error=true` (Kiro "Cursor is in the middle of a word",
`AC/state/index.ts:486-489`).

#### 6.1.6 Aliases (`SP/command.ts:100-187`)

Alias map from `run/<sid>.json` `alias` text: strip `^alias\s` per line, split lines; separator `" "` for fish, `"="`
otherwise; `[key, ...rest] = line.split(sep)`; value = `rest.join(sep)` with one leading and one trailing `'` stripped.
Expansion: while `tokens.len() > 1` and `aliases[tokens[0].text]` exists and was not used yet: replace the first
token's text range by the alias value in the command text, re-parse, require exactly one top-level `command`
(aliases containing `|`, `&&`, `;` are not expanded), map new tokens inside the alias span to the alias token's
original raw node. Tests (`qac/packages/shell-parser/tests/command.test.ts:24-33`): `woman` → `woman`; `woman ` →
`man ""`; `man woman ` unchanged; `quote ` (alias `'q'`) → `q ""`.

### 6.2 Spec model and normalisation (replaces `@fig/autocomplete-shared` `convertSubcommand`, inferred `R:parser-runtime §2.7`)

Spec JSON arrives from JS (§9.2) with functions as `{"$fn": id}`.

```rust
struct Subcommand { names: Vec<String>, description, display_name, insert_value, icon, priority: Option<f64>,
  hidden, is_dangerous, subcommands: Vec<Rc<Subcommand>>, sub_index: HashMap<String, usize> /* every alias */,
  options: Vec<Rc<Opt>>, persistent: Vec<Rc<Opt>>, opt_index: HashMap<String,(bool /*persistent*/, usize)>,
  args: Vec<Rc<Arg>>, load_spec: Option<LoadSpec>, generate_spec: Option<FnId>, generate_spec_cache_key: Option<StrOrFn>,
  directives: Directives, additional_suggestions: Vec<Item>, filter_strategy: Option<Strategy>, requires_subcommand: Option<bool> }
struct Opt { names, description, display_name, insert_value, priority, hidden, is_dangerous, args: Vec<Rc<Arg>>,
  is_persistent, is_repeatable: Repeat /* No | Times(n) | Unlimited */, requires_equals, requires_separator: Option<String|bool>,
  exclusive_on: Vec<String>, depends_on: Vec<String> }
struct Arg { name, description, is_optional, is_variadic, is_dangerous, is_command, is_script, is_module: Option<String>,
  suggest_current_token: Option<bool>, debounce: Option<u64>, options_can_break_variadic: Option<bool>,
  suggestions: Vec<Item>, generators: Vec<Generator>, load_spec: Option<LoadSpec>, filter_strategy, alias_directive: Option<StrOrFn> }
struct Directives { flags_posix_noncompliant: bool, options_must_precede_args: bool, option_arg_separators: Vec<String> /* default ["="] */ }
enum LoadSpec { Locations(Vec<SpecLocation>), Inline(Rc<Subcommand>), Fn(FnId) }
```

Normalisation rules: `name` string → `[name]`; `args`/`generators`/`options` single → array; `template` on an arg →
generator `{template}` prepended (generators whose template includes `filepaths` get `trigger:"/"`,
`getQueryTerm:"/"`); options with `isPersistent` go only to `persistent`; string `loadSpec` →
`Locations([{global, name}])`; object → `Inline(normalise)`; name collisions in an index: later wins; `isRepeatable`:
absent/false → `No` (=1), `true` → `Unlimited`, number → `Times(n)`.

### 6.3 Spec resolution and loading (`R:parser-runtime §2`, `R:spec-format §5-6`)

`spec_location(name, cwd, is_script: Option<bool>)` (`AP/loadSpec.ts:61-106`, shortcuts `?`/`+` dropped):
- first token (`None`): `name == "bin/console"` or ends with `/bin/console` → global `php/bin-console`; no `/` → global `name`; else local.
- `isCommand` arg (`Some(false)`): path not starting with `/`, `./`, `~/` → global `name` (`npx @capgo/cli` → `@capgo/cli`).
- local: path starting `/` or `~/` → `{local, dir, basename}`; else relative to cwd (strip `./`). Local specs load
  `<dir>/.fig/autocomplete/build/<basename>.js`.
- `isModule:"python/"` + token `http.server` → global `python/http.server`.

`resolve(global name)` → file: search roots in order (dev folder when `autocomplete.developerMode`, then
`~/.tabsmith/specs/user/`, then `~/.tabsmith/specs/<ver>/`): `<name>.js`, else `<name>/index.js`. Diff-versioned names
(`index.json.diffVersionedCompletions`) go straight to `<name>/index.js`. Missing → None (silently; e.g.
`gcloud/alpha`, `gcloud/beta` are missing upstream).

`load(location)`: name in `autocomplete.disableForCommands` → None. Cache key `type,path,name` (no cache in dev mode).
5 s timeout (interrupt handler + pump deadline). Module evaluation in §9.2. Then:
- `default` is an object → normalise.
- `default` is a function (6 diff-versioned `index.js`): version = memo `cliVersion-<name>`:
  `getVersionCommand(exec)` if exported, else `semver_clean(exec({command:name, args:["--version"]}).stdout)`;
  errors → `None`; `@usermn/sdc` → `None` (no `npx`). `r = await default(version)`. If `r.versionedSpecPath` →
  load `<dir>/<base>.js` (`heroku/8.6.0`), take its `default` (base) and `versions`, apply diffs; else `r` is the spec.

Version file pick (decoded from `specs/heroku/index.js`, runs inside the spec's own JS): sort versions with semver;
no target → newest; else newest `<= target`, else newest.

Diff application (Rust; the helper `getVersionFromVersionedSpec` is not on disk, inferred order): sort `versions`
keys by semver; for each key `<= target` (all if `target` is None), `base = merge_subcommand(base, versions[key])`.

```text
merge_subcommand(base, diff): diff null → base; out = {...base, ...diff};
   if diff.subcommands: out.subcommands = merge_named(base.subcommands, diff.subcommands, merge_subcommand)
   if diff.options:     out.options     = merge_named(base.options,     diff.options,     merge_option)
   if diff.args:        out.args        = merge_args(base.args, diff.args)
merge_option(base, diff) = {...base, ...diff} with only args merged by merge_args
merge_named(bl, dl, f): used = {}; out = []
   for d in dl: j = first unused index in bl whose names ∩ d.names ≠ ∅
                if none: out.push(d) else: used += j; if !("remove" in d): out.push(f(bl[j], d))
   out.extend(unused bl items in order)          // diff items first, then untouched base items
merge_args(b, d) = d.filter(!remove).enumerate().map((i, x) → {...b[i], ...x})   // index after filtering (quirk kept)
```

Spec-load errors are logged; the request replies with no suggestions (menu error-reset).

### 6.4 Argv state machine (port of `AP/parseArguments.ts`; full detail `R:parser-runtime §3`)

```text
ArgState { args: Option<Vec<Rc<Arg>>>, index, variadic_count }
State { completion: Rc<Subcommand>, opt_args: ArgState, sub_args: ArgState, annotations: Vec<Ann>,
        passed: Vec<Rc<Opt>>, command_index, entered_sub_args: bool, end_of_options: bool, persistent: Vec<Rc<Opt>> }

cur(s)                 = s.args?[s.index]
mandatory_or_var(a)    = a.is_some() && (a.is_variadic || !a.is_optional)
prefer_opt_arg(st)     = mandatory_or_var(cur(opt_args)) || cur(sub_args).is_none()
arg_state(st)          = prefer_opt_arg ? opt_args : sub_args
advance(s)             = cur.is_variadic ? variadic_count += 1 : (index+1 < len ? index += 1 : s = empty)
can_subcommands(st)    = !mandatory_or_var(cur(opt_args)) && !entered_sub_args
can_options(st)        = false if entered_sub_args && directives.options_must_precede_args
                       | false if end_of_options
                       | if mandatory_or_var(cur(opt_args)): cur.is_variadic && opt_args.variadic_count > 0
                                                             && cur.options_can_break_variadic != Some(false)
                       | false if cur(sub_args).is_variadic && sub_args.variadic_count > 0
                                && cur(sub_args).options_can_break_variadic == Some(false)
                       | true

parse(cmd):
  tokens = cmd.tokens; if tokens.is_empty() → error
  if tokens.len() == 1: spec = first_token_spec (§6.9) or dotslash/php-bin-console if token contains "/"
                        → result(initial(spec, token))
  st = parse_prefix(tokens[..len-1]) (cached by key [texts joined " ", location, cwd]; no cache in dev mode)
  try update(st, last_token, final=true) else push Ann{none, last}
  → result(st)

parse_prefix(tokens, locations = [spec_location(tokens[0], cwd, None)], start = 0):
  spec = first location that loads, else error; st = initial(spec)
  apply_load_spec(spec.load_spec, 0)            // function loadSpecs skipped at index 0
  for i in 1..tokens.len():
     if st.completion.generate_spec: merge generated (key `${tokens[0]}:${cacheKey}`; subcommands/options/persistent
         object-merged, generated wins; args kept unless empty)          // runs for every i incl. the final one
     if i == tokens.len()-1 && called from parse(): break   // final token never loads specs
     last_arg = cur(arg_state(st)); last_kind = prefer_opt_arg ? option_arg : subcommand_arg; last = st.clone()
     st = update(st, tokens[i], final=false)?               // error aborts the whole parse (no suggestions)
     last_type = type of last annotation (or last subtoken of a composite)
     if last_type == last_kind && last_arg.alias_directive && token not yet substituted:     // git aliases
         value = string | await fn(token, exec); re-tokenize command with token → value (§6.1.6 substitute);
         st = last; i -= 1; continue        (first failure propagates if nothing was substituted yet)
     ls = last_type == subcommand ? st.completion.load_spec : None
     if last_type == last_kind && last_arg: ls = last_arg.load_spec
            | is_command/is_script → [spec_location(token, cwd, Some(is_script))]
            | is_module(p) → [global p+token]
     if ls: if Locations(l) → return parse_prefix(tokens, l, start+i) with command_index += i
            if Inline/Fn-returned subcommand → st.completion = {..sub, directives: parent ∪ sub};
                 opt_args = empty; passed = []; sub_args = ArgState(sub.args); entered_sub_args = false
  return st

update(st, token, final): try in order, first success wins
  1. if can_subcommands: subcommand(token)
  2. if can_options: option_or_chain(token)
  3. if prefer_opt_arg: option_arg(token)  else: subcommand_arg(token)   // "Cannot consume subcommand arg." if none

subcommand(token): err if entered_sub_args; child = completion.sub_index[token] (exact, case-sensitive) else err
  push Ann{subcommand}; if final: return (siblings stay suggestable)
  child' = child with directives inherited if empty; persistent = parent.persistent ∪ child.persistent (parent wins;
           computed, not mutated)
  st.completion = child'; passed = []; opt_args = empty; sub_args = ArgState(child.args)

option(flag, final): o = opt_index[flag] (options or inherited persistent) else err
  Times(n) and count(passed with overlapping names) >= n → err
  push Ann{option}; if final: return; passed.push(o); opt_args = ArgState(o.args)

option_or_chain(token):
  final && token ∈ {"-","--"} → err
  token == "--" → end_of_options = true; Ann{option}; opt_args = empty
  long = directives.flags_posix_noncompliant || token starts "--" || !token starts "-"
  if long:
     seps = directives.option_arg_separators (default ["="]); find first sep occurrence in token
     with sep: (flag, val) = split; option(flag,false); err if o.args.len() > 1; option_arg(val, final) (err if no args
               → whole token falls back to positional); Ann composite[{option, flag+sep}, {option_arg, val}]
     without:  option(token, final); if o.requires_equals || o.requires_separator: opt_args = empty
  else (POSIX chain "-abc"): for i in 1..len: f = "-"+token[i]
        try option(f,false) else { if i > 1: opt_val = token[i..]; break } else err
        subtoken {option, i==1 ? "-a" : "b"}; if mandatory_or_var(cur(o.args)): opt_val = token[i+1..]; break
     if opt_val non-empty: err if args > 1; option_arg(opt_val, final); subtoken option_arg
     Ann composite; when final, the last letter's option is not added to passed (stays suggestable)

option_arg / subcommand_arg(token, final): err if no current arg; Ann; if !final: advance(); sub-arg sets entered_sub_args

result(st):
  search_term = last annotation text ("" after a trailing space); arg_st = arg_state(st)
  if last annotation is composite: arg_st = opt_args; if its last subtoken is option_arg: search_term = subtoken text; only_args = true
  current_arg = cur(arg_st)
  flags = ARGS | (!only_args && !end_of_options && can_subcommands ? SUBCOMMANDS) | (… && can_options ? OPTIONS)
```

Expected outcomes (qac test table `qac/packages/autocomplete-parser/tests/parseArguments.test.ts:291-530`; mock spec:
`-m` mandatory arg, `-o` optional arg, `-v` variadic, `-n` none, `--req-eq` requiresEquals, `--req-sep` requiresSeparator
`":"`, `man` two mandatory positionals, `oas` optionArgSeparators `["=",":"]`, `pnf` posix-noncompliant, `ompa`
optionsMustPrecedeArguments) — copy as Rust tests:

| buffer | flags | currentArg | searchTerm |
|---|---|---|---|
| `cmd man -oarg` | Args | `-o`.arg | `arg` |
| `cmd man -o=arg` | Args | `-o`.arg | `=arg` |
| `cmd man -omarg` | Args | `-m`.arg | `arg` |
| `cmd man -moarg` | Args | `-m`.arg | `oarg` |
| `cmd man -nb` (no `-b`) | Any | man.args[0] | `-nb` |
| `cmd man --opt=` | Args | `--opt`.arg | `` |
| `cmd man --none=arg ` | Any−Subcommands | man.args[1] | `` |
| `cmd man --req-eq ` | Any | man.args[0] | `` |
| `cmd oas --req-sep:arg ` | Any | oas.args[0] | `` |
| `cmd pnf -pnfo=arg` | Args | `-pnfo`.arg | `arg` |
| `cmd man -- -o ` | Args | man.args[1] | `` |
| `cmd -o arg ` | Any | null | `` |
| `cmd ompa argument ` | Args | null | `` |
| `cmd sudo sudo sudo sudo ` | — | loads spec 4×, 2 annotations | `` |

### 6.5 Generator context

```text
ctx = { currentWorkingDirectory: tracker.cwd or "", currentProcess: shell name, sshPrefix: "",
        environmentVariables: session env (run/<sid>.json, else wrapper env),
        searchTerm: result.search_term,            // MUST be real: the bundled filepaths() relies on it (ish bug)
        isDangerous: current_arg.is_dangerous }
tokenArray = token texts from command_index (includes the partial last token)
```

Empty cwd → script/custom generators return `[]` (`AC/generators/helpers.ts:25-26`).

### 6.6 Generators

#### 6.6.1 State and triggers (`AC/state/generators.ts:118-198`)

The engine keeps `GenSlot { arg: Weak<Arg> identity, index, last_search_term, result, loading }` per generator of the
current arg. Per request, for each generator `g` at index `i`:
- trigger if no slot / arg identity changed;
- else `g.trigger` undefined → trigger only if `arg.debounce`; string `t` → `a.rfind(t) != b.rfind(t)`;
  function → `call(trigger, [a, b])` (sync; exception ⇒ trigger); `{on:"change"}` → `a != b`;
  `{on:"threshold", length:n}` → `a.len() > n && !(b.len() > n)`; `{on:"match", string}` → index of `a` in the list ≠ index of `b`.
- A triggered slot keeps its previous result (`loading = true`); with `arg.debounce` wait `debounce ms` (number > 0) or
  200 ms before running (abandon if superseded).
- `triggered_generators` in the reply = keys of slots whose run was started (used by the visibility machine, §7.1).
- While a slot is loading, its stale results are shown only if the generator has no `getQueryTerm` (`AC/suggestions/index.ts:216-223`).

#### 6.6.2 Run (`AC/state/generators.ts:83-116`)

- `template` → §6.6.4. Else `script` → §6.6.3. Else `custom(tokenArray, "$exec", ctx)` → awaited; `undefined` → `[]`;
  items get `type: item.type || "arg"`; if `filterTemplateSuggestions` and first item has `context` → apply it.
- Every result item is tagged with the generator key (for `getQueryTerm` and §7.1). Any error → `[]` + log.

#### 6.6.3 Script generators (`scriptSuggestionsGenerator.ts:10-92`)

```text
cmd = script is fn ? call(script, [tokenArray]) : script;  falsy → []
input = array ? {command: cmd[0], args: cmd[1..], cwd} : string ? {command:"sh"|"cmd", args:["-c"|"/C", s], cwd}   // legacy (none in 2.692.3)
                                                          : {cwd, ...cmd}
timeout = max(autocomplete.scriptTimeout, g.scriptTimeout, input.timeout)
out = cached_run(key = g.cache.cacheKey ?? json(input)) → exec(input, timeout)
items = splitOn ? (out.stdout.trim()=="" ? [] : out.stdout.trim().split(splitOn))
      : postProcess ? call(postProcess, [stdout, tokenArray]).filter(x → x && (string || x.name))
      : []
strings → {type:"arg", name:s, insertValue:s, isDangerous}; objects → type default "arg"
```

#### 6.6.4 Templates

- `filepaths` / `folders` (generator objects created by `template` and the bundled `filepaths()` helper both end up
  calling `exec({command:"ls", args:["-1ApL"], cwd: dir})`). tabsmith answers that call **natively** (§9.3), so the
  bundled helper keeps its exact semantics: `dir` = `getCurrentInsertedDirectory(cwd, searchTerm)` (expand `~`,
  `$VAR`, `${VAR:-d}` from env; take up to the last `/`; relative → `cwd/` + it); names sorted non-dot by
  `localeCompare`, then dot names, `.DS_Store` skipped, `"../"` always appended; trailing `/` → `{type:"folder"}`
  else `{type:"file"}`; `context.templateType`; trigger = last-`/` change; queryTerm = after last `/`
  (`R:parser-runtime §4.3`). Template generators (`template:"filepaths"`) are executed by the same Rust code path
  (no JS).
- `help`: walk annotations from the root spec through each subcommand annotation except the last; return that level's
  subcommand names excluding the triggering subcommand's aliases as `{type:"special", name, insertValue:name, context:{templateType:"help"}}`.
- `history`: `[]` (F6).

#### 6.6.5 Cache (ish semantics, D10)

```text
key = [g.cache.cacheByDirectory ? cwd : "", g.cache.cacheKey ?? tokenArray.join(" ")].join(",")
entry {value, fetched_at, inflight}
g.cache present: age <= ttl (default 0) → value
                 strategy == "max-age" → blocking refetch
                 else ("stale-while-revalidate" default) → return stale value, start background refetch
                     (result arrives as a phase-Generators reply)
g.cache absent: results live in the GenSlot until the trigger fires; cleared on CommandFinished
failures are never stored; in-flight identical keys are shared
```

### 6.7 Assembly, filtering, ranking

#### 6.7.1 Assembly (`AC/suggestions/index.ts:96-290`), in this order

1. `SUBCOMMANDS` flag: unique subcommands (dedupe aliases), `type:"subcommand"`, `should_add_space`, sorted by first name (lowercase compare, ties by exact compare — approximates ICU `localeCompare`; the same ordering is used for the native `filepaths` template listing).
2. `ARGS` flag: `current_arg.suggestions` (strings → `{type:"arg", name, isDangerous: arg.is_dangerous}`), then each generator's results (§6.6.1 stale rule).
3. `completion.additional_suggestions` (always), alphabetical.
4. `OPTIONS` flag: options ∪ persistent (unique), minus names in any passed option's `exclusive_on`, minus options
   used `>=` repeat limit (`No` = 1, `Unlimited` never), `priority = 75` for names in any passed option's
   `depends_on` not yet passed; `should_add_space`, `separator_to_add` (args[0] optional → none; `requires_separator`
   string → it, `true` → first `option_arg_separators` or `"="`; else `requires_equals` → `"="`); alphabetical.
5. Option chain (last annotation composite ending in an option, e.g. typing `-la`): keep only `option`/`arg` rows; if
   OPTIONS flag is off still add the option `-<last char>`; arg rows get query term `""`; the option named
   `-<last char>` is renamed to the whole chain; other short options `-x` become `{name: chain+"x", insertValue: chain+"x"}`.

`should_add_space(item)` = args[0] exists and is not optional → true; else `requires_subcommand` if set; else has subcommands.

#### 6.7.2 History rows (`AC/state/index.ts:124-160`, §6.10)

#### 6.7.3 Filter (`filterSuggestions`, `AC/suggestions/index.ts:393-519`)

```text
query(s) = s.get_query_term or generator.get_query_term:
             string g → search_term[ search_term.rfind(g).map(+g.len()).unwrap_or(0) .. ]   (D23)
             fn g     → call(g, [search_term]) (sync; error → search_term)
           else search_term
auto_exec_hidden = hideAutoExecuteSuggestion || onlyShowOnTab

if search_term == "":
   out = s.filter(!hidden)  (priority order)
   if immediatelyExecuteAfterSpace && !out.empty && !auto_exec_hidden:
        prepend {name:"↪", type:auto-execute, insertValue:"\n", description:"Immediately execute"}
   return out
A = B = C = []; added_auto = false
for s in suggestions (already priority-sorted):
   q = query(s); qx = s.type == folder ? q+"/" : q
   exact = first name n with n.lowercase == qx.lowercase          (hidden items included)
   if exact:
      if !added_auto && !auto_exec_hidden && (names.empty || insert_value.none || names[0] == insert_value)
         && (!s.is_dangerous || immediatelyRunDangerousCommands) && (s.args.empty || s.args[0].is_optional):
           push {..s, type:auto-execute, original_type:s.type, name: folder ? exact minus "/" : s.names,
                 description: folder ? "folder" : s.description, insertValue:"\n"} to A; added_auto = true
      (s.type == folder ? B : A).push(s)
   else if !s.hidden: if let Some(score) = partial(s.names, q): C.push((s, score))
   if s.display_name: if let Some(score) = partial([display_name], q): C.push((s, score))   // dedupe removes doubles
C.stable_sort_by(score desc, then priority desc)
out = A ++ B ++ C
if !out.empty && !added_auto && !auto_exec_hidden:
   if search_term == "." || (search_term ends "/" && any item.template_type == folders):
        prepend {name: search_term=="." ? "." : "↪", type:auto-execute, original_type:folder, insertValue:"\n", description:"Enter the current directory"}
   elif current_arg.suggest_current_token ?? alwaysSuggestCurrentToken:
        prepend {name: search_term, type:auto-execute, insertValue:"\n", description:"Enter the current argument"}

partial(names, q):  prefix(n) = 0 if n==q, -1 if eq ignore case, -2 if n.starts_with(q), -3 if starts_with ignore case, else None
   prefix mode: max over names of prefix(n)
   fuzzy mode:  f = max over names of fuzzysort(q, n).score; None if no fuzzy match;
                if any prefix(n) is Some(p): p / 10.0 (so prefix hits rank above pure fuzzy hits)
```

Fuzzy mode = `current_arg ? arg.filter_strategy : completion.filter_strategy`: `prefix` → off, `fuzzy` → on, else the
user flag (setting `autocomplete.fuzzySearch`, toggled by `toggleFuzzySearch`, reset to the setting when `arg_key` changes).

#### 6.7.4 fuzzysort (`qac/packages/fuzzysort/fuzzysort.js:42-252`, fresh struct per call)

```text
single(q, t): None if q or t empty; compare lowercase chars (per char, not UTF-16 unit)
  simple: greedily find q chars in order in t → matches_simple; None if not all found
  beginning(i) = (is_upper(t[i]) && !is_upper(t[i-1])) || !is_alnum(t[i-1]) || !is_alnum(t[i])   (original case; i==0 → true)
  next_beginning[i] = smallest beginning index > i, else len(t)
  strict: ti = matches_simple[0]==0 ? 0 : next_beginning[matches_simple[0]-1]; skip if ti == len
          loop: if q[qi]==t[ti]: record ti; qi++; ti++; done if qi==len(q)
                else ti = next_beginning[ti]
                if ti >= len(t): if no strict matches: fail; m = pop; ti = next_beginning[m]; qi -= 1
  best = strict ok ? strict : simple
  score = 0; last = -2; for idx in best: if idx != last+1: score -= idx; last = idx
  if !strict ok: score *= 1000
  score -= len(t) - len(q)
  → Some(score, indexes)          tests: ("fs","Fuzzy Search") → -16 ; ("test","test") → 0
```

#### 6.7.5 Ranking pipeline (`AC/state/index.ts:98-187`)

```text
s = assembly (+ history per beta.history.mode / history_mode)
s = update_priorities(s, cmd = tokens[command_index])          // D21
s.stable_sort_by(priority desc)
s = filter(s, …)
if 0 < s.len <= 50: dedupe by (names, insert_value, display_name, args) keeping first

update_priorities: use_recency = sortMethod != "alphabetical"
  for x: name = first name; rec = use_recency && name != "../" ? recency[cmd][name] : None
         boost = rec.map(ms / 1e13).unwrap_or(0)
         p = x.priority.filter(|p| p != 0).unwrap_or(50)        // Kiro treats 0 as 50 (kept)
         if x.type != auto-execute: p = clamp(p, 0, 100)
         p = rec.is_some() && 50 <= p <= 75 ? 75 + boost : p + boost
```

`RecordInsert{cmd, name}` (sent by main on every full insertion, skipped if name contains `↪`) sets
`recency[cmd][name] = now_ms` and persists `state.json` (debounced 1 s).

### 6.8 Insertion text (precomputed per item; `AC/state/insertion.ts:25-224`, with D22/D25 fixes)

```text
q = query(item); raw = raw text of the current token as typed (from token.raw / on-screen chars)
text = if item.insert_value && item.type ∉ {file, folder}: item.insert_value     // verbatim, may contain {cursor}, \n
       else: m = name_match(item, q) else ""; escape(m + (sep ? sep + "{cursor}" : ""), is_folder, shell)
if text ends with one or more "\n": text = text.trim_end_matches('\n') + "\n"
if !text.ends_with('\n') && item.should_add_space && insertSpaceAutomatically: text += " "
delete = item.type ∈ {auto-execute, special} ? 0 :
         q == search_term ? raw.chars().count()
         : string getQueryTerm g ? raw[raw.rfind(g)+g.len()..].chars().count()
         : q.replace(whitespace, "\\ ").chars().count()               // function getQueryTerm: Kiro approximation
if raw_suffix_equals_q (no quotes/escapes in the deleted part) && text.starts_with(q) && delete >= q.chars().count():
     delete -= q.chars().count(); text = text[q.len()..]                // don't retype what is already typed
cursor_back = if text contains "{cursor}" at char i: remove it; text.chars().count() - i else 0
execute = text.ends_with('\n') → strip it, send "\r" after the text (D25)

name_match(item, q): candidates = names matching q (prefix: ==, eq-ignore-case, starts_with, starts_with-ignore-case;
   fuzzy: fuzzysort match), original order; preferVerboseSuggestions && type ∈ {option, subcommand} → longest, else first;
   none but display_name exists → pick from all names the same way
escape POSIX (Kiro): special = \ ? * ' " # | < > ( ) [ ] ! &
   none special: replace each whitespace with "\ "
   else folder: "'" + s[..-1].replace("'", "'\"'\"'") + "'/"      else: "'" + s.replace("'", "'\"'\"'") + "'"
escape pwsh: if s contains whitespace or any of $ ` ' " ( ) { } ; , & | @ # < >:
   "'" + s.replace("'", "''") + "'"; folders: "'" + s + "{cursor}'" so typing continues inside the quotes
```

Examples (tests): buffer `git st`, item `status` → delete 0, text `atus `; buffer `git ch` + `checkout` → `eckout `;
option `--out` requiresSeparator + mandatory arg → text `--out=` + cursor_back 1 then auto-space (Kiro quirk: `--out= `
with the cursor right after `=`); `cd my\ di` + folder `my dir/` → delete 6, text `my\ dir/`; `cat 'my fi` + file
`my file` → delete 6 (raw `'my fi`), text `'my file'`.

Common prefix (`insertCommonPrefix`, `insertion.ts:226-304`) is computed in main from the item list (§7.2).

### 6.9 First-token completion (`autocomplete.firstTokenCompletion`, default false)

Single token without `/`: when the setting is off → spec with no args (no suggestions). When on: items =
(root spec names from the index, `type:"subcommand"`, priority 40) ∪ (alias names, `type:"shortcut"`, priority 100,
description = alias value) ∪ (executables in the session PATH, `type:"subcommand"`, priority 40; Windows: files with
a `PATHEXT` extension, shown without it), deduped by name, cached 10 s. Token containing `/` → `dotslash` spec
(`{args:{template:["filepaths","folders"]}}`) or `php/bin-console`.

### 6.10 History rows

`prefix = buffer[..start of last token]`. For history entries newest first with `command.starts_with(prefix)`:
`name = command[prefix.len()..].trim_end()`, unique, `{type:"history", description:"past command", name,
insertValue:name, icon:'h', priority: freq(first word of name) > 1 ? 75 + min(freq,10)/10 : 50}`.
`beta.history.mode == "show"`: append rows whose name is not already a spec suggestion name; then each spec item's
priority = max(own, priority of the history row whose first word equals one of its names).
`"history_only"` or the `toggleHistoryMode` flag: only history rows.

### 6.11 Inline ghost (local) in the same reply

`ghost` = per `inline.source`:
- `history` / `history+specs` first: newest history entry with `exit_code == 0` (fallback: any) such that
  `command.starts_with(buffer) && command != buffer` → `command[buffer.len()..]`.
- `specs` / `history+specs` second: first item that is not auto-execute/history, whose insertion has `delete == 0`
  and non-empty text after trimming a trailing space and that came from a prefix (not fuzzy) match → that text.
- LLM profile: history first (as above); if none, main schedules an LLM request (§8.4).
Only when `inline.enabled`, buffer non-empty, cursor at the end of the buffer, and not `error`.

---

## 7. Dropdown UI (main thread)

### 7.1 Visibility state machine (port of `AC/state/index.ts:297-392`, `R:autocomplete-ui §2`)

```rust
enum Vis { Visible, HiddenUntilKeypress, HiddenUntilShown, HiddenByInsertion }
struct Menu { vis: Vis /* init HiddenUntilKeypress */, items, selected: usize, sticky: bool /* hasChangedIndex */,
  scroll_top, last_inserted: Option<ItemKey + GenKey>, just_inserted: bool, history_mode: bool,
  popout: bool, last: Option<Completion>, loading_since: Option<Instant> }
```

- **Error reset** (`error(msg)`): items = [], vis = HiddenUntilKeypress, history_mode = false, selection reset. Fired by:
  `Completion.error`, empty buffer, mid-word cursor, new prompt (`NewCmd`).
- **On a phase-Parse reply** `c` (only if `c.req` is the latest request):

```text
has_new_arg = last.arg_key != c.arg_key
backspaced  = c.tokens.len < last.tokens.len && last.tokens[c.tokens.len-1] == c.tokens[c.tokens.len-1]
large       = !(one of command_text/last.command_text is a prefix of the other && |len diff| < 2)
orig = vis
match vis: HiddenUntilKeypress → Visible
           HiddenByInsertion → (has_new_arg || last_inserted.gen ∈ c.triggered_generators) ? Visible : HiddenUntilKeypress
           _ → unchanged
if onlyShowOnTab: vis = orig; if has_new_arg: vis = HiddenUntilShown
if backspaced: vis = HiddenUntilKeypress
if large && !just_inserted: vis = HiddenUntilKeypress
just_inserted = false
set_items(c.items)
```

- **On a phase-Generators reply**: `set_items(c.items)` only.
- `set_items`: if `vis == HiddenUntilKeypress` keep stale items (Kiro does not recompute); else if `sticky` and the
  previously selected item (by `names, type, insert_value, description`) is in the new list → keep its index; else
  `selected = 0, sticky = false, scroll_top = 0`.
- "Shown" ⇔ `vis == Visible && (items non-empty || current_arg has name/description)` and not `autocomplete.disable`.

Behavioural summary to test: typing keeps the menu; a space shows the next level immediately; after a full insertion
the menu reopens only for a new argument or a re-triggered generator (folder `src/`); Esc hides for the rest of the
line (no timer) and only show/toggle, Tab+onlyShowOnTab, backspace into the previous token, a large change, or an
error reset bring it back; Up at index 0 hides (or toggles history with `navigateToHistory`).

### 7.2 Keys, bindings, actions

#### 7.2.1 Decoding stdin (whole chunk, sequentially)

A chunk is split into keys; bracketed paste `ESC[200~ … ESC[201~` is one opaque unit (never intercepted).

| bytes | key |
|---|---|
| `\r`, `\n` | `enter` (Ctrl-J/Ctrl-M are Enter, Kiro `input/mod.rs:1247-1257`) |
| `\t` | `tab` |
| `ESC [ Z` | `shift+tab` |
| `ESC [ A/B/C/D`, `ESC O A/B/C/D` | `up/down/right/left` (DECCKM variants included) |
| `ESC [ 1 ; m A..D` | arrows with modifiers (`m-1` bits: 1 shift, 2 alt, 4 ctrl) |
| `ESC [ H`, `ESC O H`, `ESC [ 1 ~`, `ESC [ 7 ~` / `ESC [ F`, `ESC O F`, `ESC [ 4 ~`, `ESC [ 8 ~` | `home` / `end` |
| `ESC [ 3 ~`, `ESC [ 2 ~`, `ESC [ 5 ~`, `ESC [ 6 ~` | `delete`, `insert`, `pageup`, `pagedown` |
| `0x7f`, `0x08` | `backspace` |
| `0x01..0x1a` (others) | `control+<a..z>` (case-insensitive) |
| `ESC` alone (last byte of the chunk) | `esc` |
| `ESC <utf8 char>` | `alt+<char>` |
| `ESC [ code ; mods u` | CSI-u key (rare: tabsmith strips kitty enable requests, §7.5) |
| other UTF-8 char | `<char>` |
| CPR reply `ESC [ r ; c R` | forwarded to the PTY untouched (the shell asked) |

#### 7.2.2 Binding grammar (port of `qac/crates/figterm/src/interceptor.rs:26-187`)

Split on `+` from the left; all but the last segment are modifiers (`ctrl|control`, `shift`, `alt|option`,
`meta|command`; unknown → ignored); `"+"` alone and `"ctrl++"` work. Key names: `backspace enter left|arrowleft
right|arrowright up|arrowup down|arrowdown home end pageup pagedown tab delete insert esc f<N>` or exactly one char.
`shift+<lowercase letter>` → uppercase letter, no shift. Ctrl/Alt + letter match both cases. Arrows also match their
`ESC O x` forms.

#### 7.2.3 Default bindings (fig.json, D5) and overrides

| action | default keys |
|---|---|
| `insertSelected` | `enter` |
| `insertCommonPrefix` | `tab` |
| `navigateUp` | `shift+tab`, `up`, `control+p` |
| `navigateDown` | `down`, `control+n` |
| `hideAutocomplete` | `esc` |
| `toggleDescription` | `control+k` |
| `toggleHistoryMode` | `control+r` |
| `insertCommonPrefixOrNavigateDown`, `insertCommonPrefixOrInsertSelected`, `insertSelectedAndExecute`, `execute`, `showAutocomplete`, `toggleAutocomplete`, `showDescription`, `hideDescription`, `toggleFuzzySearch`, `selectSuggestion1..10` | none |

Settings `autocomplete.keybindings.<keystroke>` = `"<action>"` (also accepted with an `autocomplete.` prefix) override
the default for that key; `"ignore"` disables interception for that key; an unknown action string swallows the key
and does nothing (Kiro). Table rebuilt on settings reload.

#### 7.2.4 Interception (`interceptor.rs:195-226`, `R:autocomplete-ui §4`)

```text
intercepting = !preexec && !alt_screen && menu shown && items non-empty
global       = !preexec && !alt_screen && items non-empty
for key in chunk:
   if intercepting && binding(key) not "ignore" → handle action; swallow
   elif global && !intercepting && ((key == tab && onlyShowOnTab) → showAutocomplete
                                    | binding(key) ∈ {toggleAutocomplete, showAutocomplete}) → handle; swallow
   elif ghost shown && cursor at buffer end && key ∈ ghost-accept keys (§8.3) → accept ghost; swallow
   else forward raw bytes (accumulated, one write per chunk);
        if key ∈ {control+c, control+d}: menu error-reset (Kiro resets interception)
        if key == backspace: backspace_echo_pending = true (keeps a visible menu up until the echo arrives)
```

Esc is swallowed and stops interception immediately (the next Esc reaches the shell).

#### 7.2.5 Actions (`AC/hooks/keypress.ts:59-196`)

Preamble: with `onlyShowOnTab` and Tab while not Visible → exactly 1 item: insert it; else `vis = Visible`. All other
actions are no-ops when there is no selected item.

| action | effect |
|---|---|
| `navigateUp` | `navigate(-1, hide_on_negative=true)` |
| `navigateDown` | `navigate(+1)` |
| `insertSelected` | full insertion of the selected item |
| `insertCommonPrefix` | common prefix; failure → BEL |
| `insertCommonPrefixOrNavigateDown` / `…OrInsertSelected` | common prefix; failure → navigate(+1) / insert selected |
| `insertSelectedAndExecute` | full insertion + `\r` (unless auto-execute) |
| `execute` | write `\r` |
| `hideAutocomplete` | `vis = HiddenUntilShown` |
| `showAutocomplete` | `vis = Visible` |
| `toggleAutocomplete` | Visible → HiddenUntilShown, else Visible |
| `toggleHistoryMode` | flip `history_mode`, re-request |
| `toggleDescription` / `show…` / `hide…` | pop-out on/off (no-op with `alwaysShowDescription`) |
| `toggleFuzzySearch` | `EngineMsg::ToggleFuzzy`, re-request |
| `selectSuggestionN` | select the Nth visible row (no insert) |

```text
navigate(d, hide_on_negative): i = selected + d
  if scrollWrapAround: i = (i + len) % len
  elif i == -1 && hide_on_negative: if navigateToHistory { toggle history_mode; return } vis = HiddenUntilKeypress; return
  selected = clamp(i, 0, len-1); sticky = selected changed; scroll window: if selected < top: top = selected
                                                          if selected >= top+rows: top = selected-rows+1
```

Common prefix (`insertion.ts:226-304`): 1 item → full insertion; selected type (or `original_type` for auto-execute)
∈ {special, auto-execute} → fail; candidates = items of a matching type (`../` only matches `../`; file/folder match
each other) whose lowercase first name starts with `q.lowercase`; 1 candidate → full insertion (auto-execute → fail);
`lcp` = longest common prefix of the sorted lowercase names; `shared = first_name(selected)[..lcp.chars]` (selected
item's casing); empty or `== q` → fail; text = shared with whitespace → `\ `; equal to the full insertion text → full
insertion; else partial insertion (delete = query chars, `just_inserted = true`, menu stays visible).

### 7.3 Writing insertions to the PTY

```text
full insertion of item (precomputed delete, text, cursor_back, execute):
  bytes = "\x7f" * delete + text + "\x1b[D" * cursor_back + (execute ? "\r" : "")
  reconcile: let ib = buffer the reply was computed from, cur = current extracted buffer
     if cur != ib: if cur.starts_with(ib): bytes = "\x7f" * (cur.chars - ib.chars) + bytes
                   elif ib.starts_with(cur): bytes = ib[cur.len()..] + bytes          // shell has not echoed yet
  writer.send(bytes)
  expected = (ib minus last `delete` chars before the cursor) + text (+ the rest after the cursor); cursor moved back
  lock until extracted buffer == expected or 100 ms passed (buffer changes during the lock do not reach the state machine)
  vis = HiddenByInsertion; last_inserted = item; just_inserted = true
  EngineMsg::RecordInsert{cmd: tokens[command_index], name: first name}
partial (common prefix): same bytes path; only just_inserted = true
```

Insertions are dropped while `preexec`.

### 7.4 Layout and content

#### 7.4.1 Geometry

```text
rows R, cols C, cursor (cy, cx) from the model (0-based viewport)
w      = min(autocomplete.width, C)                 // default 40
n_max  = autocomplete.height                        // default 7
n      = min(n_max, items.len())
desc   = items non-empty ? 1 : 0                     // description bar under the list
h      = n + desc;   arg-hint mode (no items, current_arg has name/description): n = 0, h = 1
below  = R - 1 - cy;  above = cy
side   = below >= h ? Below : above >= h ? Above : (below >= above ? Below : Above)
room   = side == Below ? below : above
if room < h: n = room - desc; if n < 1 { hide; } h = n + desc
x0     = clamp(cx - width(query of selected item) - 2, 0, C - w)       // align suggestion text under the typed token
block order is always list rows then the description row:
  Below → list on cy+1 ..= cy+n, description on cy+n+1
  Above → list on cy-h ..= cy-h+n-1, description on cy-1 (block bottom-aligned to the cursor, like Kiro's isAboveCursor)
```

#### 7.4.2 Rows

List row for item `s` (selected row uses selected colours):
`' '` + icon (1 col) + `' '` + title + (`' '` + args hint in dim when room) + pad to `w`.
- Icon (ASCII, width 1): subcommand `>`, option `-`, arg/special/generator `*`, file `f`, folder `d`, history `h`,
  shortcut `@`, auto-execute `↪`. Spec icons/emoji are ignored (terminal width safety).
- Title = `display_name` or names joined `", "`. Highlight: prefix mode → for each name whose lowercase starts with
  the lowercase query, the first `q.chars().count()` chars get `matching-bg`; fuzzy mode → fuzzysort indexes of the best
  name. (No regex; fixes `R:autocomplete-ui §15.4`.)
- Args hint: for each arg with a name: `name` + (`...` if variadic); optional → `[x]`, mandatory → `<x>`; joined `" "`.
- Truncation by display width (`unicode-width`), overflow replaced by `…`.

Description row: `' '` + (`name: ` bold when the text comes from `current_arg`) + italic description
(= selected.description, or `file`/`folder` for those types, else `current_arg.description`); `No description` in
`#8c8c8c` when empty; right-aligned spinner frame (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`, 80 ms) while any generator has been loading > 200 ms.

Arg-hint mode: one row `name: description`. No key interception (no items).

#### 7.4.3 Pop-out (SHOULD)

`toggleDescription` / `alwaysShowDescription`: a 30-column panel beside the list (right if `x0+w+30 <= C`, else left
if `x0 >= 30`, else not shown), height `h`, description wrapped by display width; the description bar row is then
not drawn.

### 7.5 Rendering

#### 7.5.1 Guards (draw nothing, keep only clears)

`preexec`; `ALT_SCREEN`; model modes `ORIGIN` or `INSERT` set or a non-full scroll region (verify accessor); cursor in
pending-wrap state (`grid.cursor.input_needs_wrap`, verify); resize repaint pending; terminal smaller than 10×3.

#### 7.5.2 Frame bytes

```text
frame = "\x1b[?2026h\x1b[?25l"
for each covered rect (menu rows, pop-out, ghost span):
    span = [x_start, x_end) extended by 1 on the left if cell x_start is WIDE_CHAR_SPACER,
           and by 1 on the right if cell x_end-1 is WIDE_CHAR (a covered half of a wide char is blanked by the terminal)
    frame += CUP(row, x_start) + SGR + content (extra extended cells filled with spaces in the box bg)
    clear += CUP(row, x_start) + restore_cells(model row, span)
frame += CUP(cy, cx) + "\x1b[0m" + sgr(template of grid.cursor) + (SHOW_CURSOR ? "\x1b[?25h" : "") + "\x1b[?2026l"
clear wrapped the same way, also ending with the cursor/pen restore
CUP(r, c) = "\x1b[{r+1};{c+1}H"
restore_cells: for x in span: cell = grid[row][x]; skip WIDE_CHAR_SPACER; if style changed: "\x1b[0m" + sgr(cell);
               emit cell.c (+ zerowidth); blank/NUL cells → ' ' with the cell's bg
sgr(cell): fg Named(Black..White)=30..37, Bright*=90..97, Foreground → 39, Dim* → normal + 2; Indexed(i) → 38;5;i;
           Spec(rgb) → 38;2;r;g;b (exact, no 256 downgrade — fixes ish); bg likewise 40..47/100..107/49/48;5/48;2;
           flags: BOLD 1, DIM 2, ITALIC 3, UNDERLINE 4, INVERSE 7, HIDDEN 8, STRIKEOUT 9
```

Hyperlinks (incl. tabsmith's model-only tags) are never emitted. Real hyperlinks under a restored span lose their link
(ish has the same limit).

#### 7.5.3 Frame scheduling

- Every PTY batch: `take_clear()` first (restores whatever the last frame covered, computed from the model state at
  draw time, which equals the real screen at that moment), then the PTY bytes, then a fresh frame (§4.5).
- State-only changes (navigation, completion reply, ghost, spinner): `clear + frame` in one write.
- A render key (`side, rows, items window, selected, ghost, cursor, colours`) skips identical frames.

#### 7.5.4 Output filtering (port of ish `stdioProxy.ts:15-43`)

Strip from the bytes written to the real terminal (the model still sees them): `CSI ?9001h/l` (win32-input-mode),
kitty keyboard `CSI > … u`, `CSI < … u`, `CSI = … u`, `CSI ? u`, modifyOtherKeys `CSI > … m`; partial sequences carried
across batches. Only outside the alternate screen (inside, apps such as neovim keep their keyboard protocol; tabsmith
does not intercept keys there).

### 7.6 Resize, alternate screen

- Resize: `master.resize`, `term.resize` (alacritty reflows), drop the cached clear (the terminal reflowed the old
  frame unpredictably), menu/ghost hidden, arm 150 ms debounce (re-armed by PTY data meanwhile). When it fires and
  not on the alternate screen: full repaint `?2026h ?25l CSI H CSI 2J` + every model row (CUP + cells up to the last
  non-blank) + cursor/pen restore + `?2026l`, then a normal frame. On the alternate screen: postpone until it is left.
  Unix: SIGWINCH → `crossterm::terminal::size()`; Windows: `WINDOW_BUFFER_SIZE_EVENT` + 250 ms polling.
- Alternate screen (`CSI ?1049h/?1047h/?47h`): the clear runs before the batch that switches (always), no frames while
  `ALT_SCREEN`, keys never intercepted, menu state error-reset on leave.

### 7.7 Themes (port of `qac/packages/autocomplete/src/fig/themes.ts`)

Eight colours: `main-bg`, `main-text`, `matching-bg`, `selected-bg`, `selected-text`, `selected-matching-bg`,
`description-text`, `description-border` (unused in v1).

- Built-in dark: bg `rgb(48,48,48)`, text `rgb(180,180,180)`, match bg `rgb(95,89,56)`, selected text
  `rgb(253,253,253)`, selected bg `rgb(30,90,199)`, selected match bg `rgb(106,142,218)`, description text
  `rgb(180,180,180)`, border `rgb(65,65,65)`.
- Built-in light: bg `rgb(254,254,254)`, text `rgb(7,7,7)`, match bg `rgb(255,239,152)`, selected text
  `rgb(253,253,253)`, selected bg `rgb(41,105,218)`, selected match bg `rgb(106,142,218)`, description text
  `rgb(7,7,7)`, border `rgb(199,199,199)`.
- `system`: startup OSC 11 reply luminance `0.299r+0.587g+0.114b < 0.5` → dark, else light; no reply → dark.
- Custom file: `version:"1.0"` + `theme{}` → explicit colours (missing ones from dark); else base16: main-bg=shade0,
  matching-bg=shade1, selected-bg=shade2, description-text=shade5, main-text=shade6, selected-text=shade7,
  selected-matching-bg=accent6; missing shades/accents from the dark filler (`#181818 #282828 #383838 #585858
  #b8b8b8 #d8d8d8 #e8e8e8 #f8f8f8`; accents `#ab4642 #dc9656 #f7ca88 #a1b56c #86c1b9 #7cafc2 #ba8baf #a16946`). Any
  error → dark.
- Colour parsing: `#rgb`, `#rrggbb`, `#rrggbbaa` (alpha premultiplied against black), `rgb(r, g, b)` with optional
  spaces (fixes Kiro's no-space regex).
- Output: truecolor SGR if `COLORTERM ∈ {truecolor, 24bit}` or `WT_SESSION` set; else nearest xterm-256 index (6×6×6
  cube + 24 greys).

---

## 8. Inline ghost text

### 8.1 When shown

`inline.enabled`; at a prompt (not `preexec`, not alt screen); buffer non-empty; cursor at the end of the buffer;
ghost text non-empty and only one line; **the shell is not already showing its own suggestion** (any ghost-coloured
cell at/after the cursor, §5.1.3 — covers zsh-autosuggestions, fish autosuggestions, PSReadLine predictions). The
dropdown and ghost text can be visible together.

### 8.2 Rendering

Drawn in the frame (§7.5) at `(cy, cx)` with `ESC[0m ESC[90m` (bright black, the zsh-autosuggestions `fg=8`
convention), clipped at the first non-blank model cell to the right (an RPROMPT) and at the right edge (never wraps).
Restored like any other span. The shell's echo of the next typed char overwrites the first ghost cell; the next frame
redraws the shortened ghost.

### 8.3 Accept

| key | effect (only when the ghost is shown and the cursor is at the buffer end, so the shell would no-op anyway) |
|---|---|
| `right`, `control+f`, `end`, `control+e` | write the whole ghost text to the PTY; clear ghost |
| `alt+f` (`ESC f`), `control+right` | partial accept: leading whitespace + the next run of non-whitespace chars |
| `tab` | **not** a ghost key (dropdown `insertCommonPrefix` when the menu intercepts, else the shell's own completion) |
| anything else | forwarded; ghost recomputed from the next reply |

Accepting does not count as an insertion for the visibility machine (it is ordinary typing to the shell).

### 8.4 Request flow

Local sources arrive in every `Completion` (§6.11). LLM (when `inline.source` is a profile and the local reply has no
history match): main sets `llm_deadline = last buffer change + inline.debounceMs`; on expiry, if the buffer is
unchanged, send `EngineMsg::Llm{req, buffer, cwd, shell}`. Engine: cache lookup (below) → immediate `Msg::Ghost`; else
spawn an LLM worker (5 s timeout) with a history snapshot. The worker replies `Msg::Ghost{req, text}`; main shows it
only if `req` is the latest and the buffer is unchanged.

Cache (port of `qac/crates/figterm/src/inline/completion_cache.rs`): `Vec<(full_text, priority)>`, cleared on
`CommandFinished` (ponytail: linear scan of ≤ 50 entries instead of a radix trie). Lookup: entries where
`full.starts_with(buffer) && full != buffer`, minimum priority (first inserted wins ties) → `full[buffer.len()..]`. The
first completion of a response is stored with priority 0.0, others 1.0. This makes a suggestion survive the user typing
its first characters.

### 8.5 LLM providers

Prompt (Kiro format, `qac/crates/figterm/src/inline/mod.rs:381-403`): last `inline.historyCount` (49) history
commands with non-null exit code, oldest → newest, then the buffer, numbered like `history`:
`format!("{:>5}  {cmd}", n)` joined by `\n`; drop oldest lines until ≤ 10 240 bytes. **Privacy filter**: history lines
containing (case-insensitive) `password`, `passwd`, `secret`, `token`, `api_key`, `apikey`, `authorization:`,
`bearer ` are dropped; if the buffer itself matches, no request is made. Requests happen only when the user selected a
profile.

```text
SYSTEM = "You complete shell commands. The user message shows the user's recent shell history in `history` format;
the last numbered line is the command being typed. Reply with ONLY the characters that should be appended to that
last line to complete it: one line, no explanation, no quotes, no code fences. If unsure, reply with nothing."
USER   = "OS: <macos|linux|windows>\nShell: <shell>\nCWD: <cwd>\n\n<history prompt>"
```

OpenAI-compatible (`provider:"openai"`; works with OpenAI, Ollama `http://localhost:11434/v1`, vLLM, LM Studio, OpenRouter):

```http
POST {base_url}/chat/completions
Authorization: Bearer <key>          (omitted when the key env var is unset/empty — local servers)
Content-Type: application/json

{"model": M, "max_tokens": 64, "messages": [{"role":"system","content":SYSTEM},{"role":"user","content":USER}]}
→ text = choices[0].message.content
```

Anthropic (`provider:"anthropic"`; current API version, so **no** `temperature`/`top_p`/`top_k` — they return 400 on
`2026-10-21`):

```http
POST {base_url}/v1/messages
x-api-key: <key>
anthropic-version: 2026-10-21
content-type: application/json

{"model": M, "max_tokens": 1024, "system": SYSTEM,
 "messages": [{"role":"user","content":USER}],
 "output_config": {"effort": E}}          ← only when the profile sets "effort" (e.g. "low" for latency)
→ if stop_reason == "refusal" → no suggestion
  text = concatenation of content[i].text where content[i].type == "text"   (thinking blocks may precede it)
```

Example profile: `{"provider":"anthropic","model":"claude-opus-5-5","api_key_env":"ANTHROPIC_API_KEY","effort":"low"}`.
`max_tokens` defaults to 1024 for Anthropic because current models think before answering; the profile may override.

The key is read from the **session env** (`run/<sid>.json`, which sees variables exported later in the rc) then the
wrapper env. HTTP errors, timeouts, non-2xx → no suggestion, logged at `warn` (never on screen).

Cleaning: take the first non-empty line; strip surrounding backticks/quotes; if it starts with the buffer (model echoed
the whole command) strip that prefix; strip a leading `\d+\s{2}` history number; `trim_end`; empty → none.

Validation (port of `inline/validate.rs:3-59`): `full = buffer + completion`; if `full.trim()` splits (shell words) into
exactly `["cd", arg]`: expand `~`/`$VAR` from the session env (unknown var → accept); absolute non-directory → reject;
else `cwd.join(path)` must canonicalize to a directory. Everything else is valid.

---

## 9. Specs and the JS host

### 9.1 Bundling (D11)

- `scripts/update-specs.sh` (repo tooling, run by maintainers): download
  `https://registry.npmjs.org/@withfig/autocomplete/-/autocomplete-<ver>.tgz` (inferred URL pattern), extract
  `package/build/` + `package/LICENSE`, `tar -C build -cf - . | zstd -19 > assets/specs.tar.zst`, write `<ver>` to
  `assets/SPECS_VERSION`.
- `include_bytes!` (cargo feature `embed-specs`, default on; ~4.2 MB). First engine start (or `setup`, or doctor fix):
  if `~/.tabsmith/specs/<ver>/index.json` is missing, decompress (`zstd::Decoder` + `tar::Archive`) into
  `specs/<ver>.tmp-<pid>/`, then rename to `specs/<ver>/` (atomic; concurrent extractors: the loser deletes its temp dir).
  77 MB on disk. Old `specs/<other ver>/` dirs are removed after a successful extraction.
- Index: `index.json` (`completions` 1 484 entries, `diffVersionedCompletions` 6) of the embedded set ∪ `*.js` stems
  present in `specs/user/`.
- Licences: ship the MIT text (Hercules Labs/Fig) and THIRD_PARTY notices for inlined semver, yaml,
  strip-json-comments, `@fig/autocomplete-*` (`R:spec-format §1`).
- SHOULD (post-v1): `tabsmith specs update` downloads a newer tarball into `specs/user/`.

### 9.2 JS host (engine thread, rquickjs 0.14)

Runtime setup: `Runtime::new()`, `set_memory_limit(512 MiB)`, `set_interrupt_handler(Some(Box::new(move || now_ms() >
deadline.load())))` (deadline set before every eval/call/pump slice), `Context::full(&rt)`. One long-lived context:
modules evaluated once and their namespaces cached by path (specs keep module-level caches, e.g. `man.js`).

Module loading without a module loader (every file ends with exactly one `export{…};`, no imports, `R:spec-format §3`):

```text
src = read file; if src.trim().is_empty() → namespace {}
tail = src[src.rfind("export{")..]; body = src[..that]; parse "A as default, B as versions, C" → pairs (local, exported)
script = "(function(){" + body + "\n;return {" + pairs.map(|(l,e)| format!("{e}:{l}")).join(",") + "};})()"
ns = ctx.eval::<Object, _>(script)   (5 s deadline)
spec_json = __ts.ser(ns.default)   → serde_json::Value → normalise (§6.2)
```

Prelude (evaluated once; Rust provides `__ts_log(level, msg)`, `__ts_exec(id, inputJson) → Promise<string>`,
`__ts_sleep(ms) → Promise`):

```js
globalThis.console = Object.fromEntries(["log","info","debug","warn","error"].map(l => [l, (...a) => __ts_log(l, a.map(String).join(" "))]));
let __tid = 0; const __live = new Set();
globalThis.setTimeout = (fn, ms = 0, ...args) => { const id = ++__tid; __live.add(id);
  __ts_sleep(ms).then(() => { if (__live.delete(id)) fn(...args); }); return id; };
globalThis.clearTimeout = id => { __live.delete(id); };
globalThis.Intl ??= { NumberFormat: class { constructor(_l, o = {}) { this.o = o; }
  format(n) { if (this.o.notation !== "compact") return String(n);
    const u = [[1e12,"T"],[1e9,"B"],[1e6,"M"],[1e3,"K"]].find(([v]) => Math.abs(n) >= v);
    return u ? +(n / u[0]).toPrecision(this.o.maximumSignificantDigits ?? 3) + u[1] : String(n); } } };
globalThis.__ts = {
  fns: [],
  ser(v, seen = new WeakSet(), d = 0) {                 // functions → {"$fn": id}; drops undefined/symbol; cycle-safe
    if (typeof v === "function") return { $fn: this.fns.push(v) - 1 };
    if (v === null || typeof v !== "object") return typeof v === "bigint" ? String(v) : v;
    if (v instanceof RegExp) return String(v);
    if (seen.has(v) || d > 200) return null; seen.add(v);
    const out = Array.isArray(v) ? v.map(x => this.ser(x, seen, d + 1))
      : Object.fromEntries(Object.entries(v).filter(([, x]) => x !== undefined && typeof x !== "symbol")
                                            .map(([k, x]) => [k, this.ser(x, seen, d + 1)]));
    seen.delete(v); return out;                         // shared (non-cyclic) refs are duplicated, cycles → null
  },
  mkExec(ctx) { return input => typeof input === "string"
      ? __ts_exec(ctx.id, JSON.stringify({command: "sh", args: ["-c", input], cwd: ctx.cwd, legacy: true})).then(r => JSON.parse(r).stdout)
      : __ts_exec(ctx.id, JSON.stringify({cwd: ctx.cwd, ...input})).then(r => JSON.parse(r)); },
  call(id, argsJson, ctxJson) {                          // every spec callback goes through here
    const ctx = ctxJson && JSON.parse(ctxJson);
    const args = JSON.parse(argsJson).map(a => a === "$exec" ? this.mkExec(ctx) : a);
    return Promise.resolve().then(() => this.fns[id](...args)).then(r => JSON.stringify(this.ser(r) ?? null));
  },
};
```

Callback signatures the engine uses (`R:spec-format §8`): `custom(tokens, "$exec", generatorContext)`,
`postProcess(stdout, tokens)`, `script(tokens)`, `trigger(newSearchTerm, oldSearchTerm)`, `getQueryTerm(token)`,
`generateSpec(tokens, "$exec")`, `loadSpec(token, "$exec")`, `parserDirectives.alias(token, "$exec")`,
`getVersionCommand("$exec")`, diff-versioned `default(version)`, `filterTemplateSuggestions(suggestions)`,
`generateSpecCacheKey({tokens})`. Results returned by `call` are JSON (functions inside generated specs get new ids).
History parsing never runs JS (`exec` would be a throwing stub; not needed since F6 is SKIP).

Event pump (manual Promise resolution, no async runtime):

```text
await_all(promises, deadline):
  loop {
    while rt.is_job_pending() { rt.execute_pending_job() (exceptions logged) }
    if all promises are not Pending → return results (Promise::result / state)
    if now >= deadline → return (pending ones = timeout → [])
    msg = engine_rx.recv_timeout(min(deadline, next timer) - now)
    ExecDone{id,out} → take Persistent (resolve, reject) for id; call resolve(json) or reject(Error(msg))
    Timer{id}        → resolve the sleep promise
    Complete(newer)  → stash it; return Aborted            (dangling promises resolve later harmlessly)
    other            → handle inline (Settings, CommandFinished, RecordInsert, Llm, Session, ToggleFuzzy)
  }
__ts_exec(id, input): (promise, resolve, reject) = ctx.promise(); store Persistent(resolve, reject) under a new exec id;
   run §9.3 on a worker thread which sends EngineMsg::ExecDone; return promise
```

Function-id table growth is bounded by distinct spec loads + `generateSpec` results (cached by key); not collected (ponytail).

### 9.3 `exec` semantics (port of figterm RunProcess, `qac/crates/figterm/src/message.rs:61-151,459-515`)

```text
input {command, args[], cwd?, env?: {k: v|null}, timeout?}
short-circuits:
  command == "ls" && args.map(trim) == ["-1ApL"] → native: read_dir(cwd) following symlinks, include dotfiles
       except . and .., dirs get "/", one per line, no sorting → {stdout, stderr:"", status:0}; error → status 2
  command ∈ {fig, q, kiro, kiro-cli, tabsmith} → {stdout:"", stderr:"", status:1}   (bundled ai() helper)
cwd   = input.cwd if it is a directory, else tracker cwd, else wrapper cwd, else "/" / "C:\"
env   = session env if known else wrapper env; remove LS_COLORS CLICOLOR_FORCE CLICOLOR COLORTERM;
        set PROCESS_LAUNCHED_BY_TABSMITH=1 HISTFILE="" HISTCONTROL=ignoreboth TERM=xterm-256color NO_COLOR=1;
        apply input.env (null → remove)
exe   = absolute → as is; else search env PATH (Windows: also each PATHEXT ext; .cmd/.bat run via std's cmd handling);
        not found → reject "failed running executable"
spawn = no shell; stdin null; Unix pre_exec libc::setsid() (no controlling tty); Windows CREATE_NO_WINDOW
timeout = max(autocomplete.scriptTimeout (5000; Windows 20000), generator.scriptTimeout, input.timeout);
          poll try_wait every 5 ms; on expiry kill → reject "timed out after N ms"
output  = lossy UTF-8; clean: \r\n→\n, remove "\x1b[?25h", strip leading and trailing \n+ (qac cleanOutput)
status  = exit code; killed by signal → 1;  non-zero status resolves normally (specs check it)
legacy string form → ["sh","-c",s] (Windows: Git Bash `bash.exe -c` if found, else cmd /C) resolving to stdout string
```

`bash -c …` appears 138× in 72 spec files: on Windows these work only when Git Bash's `bash` is on PATH; otherwise
those generators yield `[]` (risk, §11).

---

## 10. Test plan

`cargo test` (pure, fast, all OSes):

| area | tests |
|---|---|
| `shell_parser` | Every row of the §6.1.3 table; operators without spaces; unterminated quotes (`complete=false`, inner text); escapes in word vs string vs raw string; concatenation `--foo="a b"c`; `$(…)` nesting; `FOO= cmd` (fix); alias cases from §6.1.6; alias with `\|` not expanded. |
| `parser` | Mock spec (the §6.4 table, as JSON fixtures in `tests/fixtures/mock_spec.json`); `isRepeatable` limits; `exclusiveOn`/`dependsOn`; persistent option inheritance without mutation; `--` end of options; `loadSpec` locations recursion with `command_index` (`sudo git ` → git spec, generators see `["git",…]`); `spec_location` table (`R:parser-runtime §2.1`); diff merge (`merge_named` order, `remove`, `merge_args` index quirk) on hand-made fixtures; version pick (newest ≤ target, none → newest). |
| `suggest` | fuzzysort examples (`fs`/`Fuzzy Search` → −16, `test`/`test` → 0, non-match → None); prefix score ladder 0/−1/−2/−3; bucket order A ++ B ++ C; auto-execute rules (dangerous, optional args, hidden setting, onlyShowOnTab); `.` and `/`-folder auto-execute; dedupe ≤ 50; recency boost bands (`75 + ms/1e13`), `priority: 0` → 50, alphabetical setting; triggers (string/fn/threshold/match/change); cache TTL/SWR/max-age; option-chain rewriting (`-la`); history rows priority; insertion table from §6.8 (incl. POSIX and pwsh escaping, `{cursor}`, auto-space, raw-token deletion counts with quotes and escaped spaces). |
| `menu` | Visibility transitions for: typing, space/new arg, full insertion with and without new arg, generator re-trigger, Esc → stays hidden across tokens, backspace into previous token, large change (paste) vs `just_inserted`, `onlyShowOnTab`, `navigateToHistory`, Up at 0, wrap-around, sticky selection across recompute; common prefix (single item, mixed types, `../`, no prefix → BEL); keybinding parser (`ctrl+k`, `control+K`, `shift+tab`, `shift+a`→`A`, `+`, `ctrl++`, `f12`, unknown modifier ignored, multi-char unknown → dropped); `"ignore"`; stdin decoding table §7.2.1 incl. a chunk with several keys and a bracketed paste. |
| `render` | Layout: below/above/shrink/hide decisions at various `cy`, `R`; `x0` clamping; truncation with wide chars; colour parsing (`#rgb`, `#rrggbbaa`, `rgb( 1, 2, 3 )`); 256-colour mapping; **round-trip invariant**: model A ← `output`; model B ← `output + frame + clear`; visible grids (chars, fg, bg, flags) equal — for below, above, ghost, wide chars at both span edges, truecolor cells. |
| `pty` (no real PTY) | OSC splitter across chunk boundaries (split inside `ESC ]`, inside payload, BEL vs ST, non-697 OSC passthrough, 64 KiB flush); markers stripped from output; lock/unlock with foreign sid; `NewCmd=<other sid>` ignored; edit-buffer extraction by feeding a real `alacritty_terminal::Term` with byte transcripts: zsh prompt + command, RPROMPT, PS2 multi-line (real `\n`), wrapped long command, zsh completion listing below (excluded), ghost text in `fg=8` after cursor (excluded) vs typed `fg=8` before cursor (kept), prompt scrolled by output, resize reflow, Windows deferral FIFO; reconciliation byte strings (`cur` ahead / behind `ib`). |
| `integrations` | `init` snapshots for every (shell, when) under `TABSMITH_INIT_SNAPSHOT_TEST=1` (`insta`-free: compare to files in `tests/snapshots/`); syntax-check snapshots when tools exist: `bash -n`, `zsh -n`, `fish --no-execute`, `pwsh -NoProfile -Command [ScriptBlock]::Create(...)`; dotfile install into a temp HOME: idempotent (install twice → same bytes), shebang kept first, `# tabsmith ignore`, pre-not-first / post-not-last detection, uninstall restores original content (modulo trailing newline), backup created; should-launch decision table from env maps. |
| `settings` | get/set JSON-or-string parsing, missing key plain vs json, delete wildcard (`autocomplete.*`, `?`), sorted pretty output, corruption handling, mtime reload. |
| `inline` | Kiro cache test vector (`{"echo hello, world":0.5,…}` → `"e"`→`echo hello, world`, `"ls -a"`→`ls -al`, `"other"`→None, `R:figterm §15.2`); history prompt numbering + 10 240-byte truncation + privacy filter; cleaning (fences, echoed buffer, numbering); `cd` validation; request bodies for both providers serialized exactly as §8.5 (no `temperature`; `output_config` only with effort); response parsing incl. refusal and thinking-before-text. |
| `js` (needs specs; `#[ignore]` by default, run in CI) | Evaluate all 1 484 modules: each yields a namespace or the documented empties; all 727 root specs normalise; `git` alias directive via a fake exec; `filepaths()` through the native `ls -1ApL` path; `setTimeout` ordering; `Intl.NumberFormat` compact; interrupt handler stops `while(true){}`; legacy string exec (`aws/ecs.js`). |

Smoke / end-to-end (`tests/e2e.rs`, `#[ignore]`, Linux + macOS CI; manual on Windows):
1. Harness opens its own PTY (portable-pty), runs `tabsmith debug shell bash` (and zsh, fish, pwsh where installed)
   with `TABSMITH_HOME=<tmp>`, feeds the PTY output into an alacritty `Term` of 80×24, writes keystrokes.
2. Type `git ch` → assert a row containing `checkout` is drawn below the prompt and the model buffer is `git ch`.
3. Press Tab/Enter → shell line becomes `git checkout ` and the menu reappears with branch candidates (generator).
4. Esc → menu gone; type ` x` → still hidden; Ctrl-U → buffer empty → next keystroke shows menu again.
5. Run `echo hi` + Enter → history file gains the entry with exit code 0; typing `ec` shows ghost `ho hi`; Right
   accepts it.
6. Fill the screen so the prompt is on the last row → menu drawn above.
7. Resize 80→40→80 → after 200 ms the screen equals a fresh model render (no artefacts).
8. Run `vim`/`less` (alt screen) → no overlay bytes emitted; quit → prompt usable.
9. `exit` → harness sees the child exit code propagated.
Manual checklist per OS/terminal: macOS Terminal.app + iTerm2 (zsh, bash 5), Linux GNOME Terminal + kitty (bash, zsh,
fish), tmux pane, SSH into a host with tabsmith installed, Windows Terminal + pwsh 7 (with and without side-loaded
`conpty.dll`), VS Code integrated terminal (no hang during env resolution).

---

## 11. Risks and known gaps per OS

All OSes:
- **Model/terminal width divergence** (emoji, ambiguous-width CJK, VS16): `unicode-width` may disagree with the real
  terminal → misplaced restore spans. Mitigation: ASCII icons, exact wide-char span extension, full repaint on resize.
- **Content printed before the wrapper started** is blank in the model; if the menu covers it, restoring shows blanks.
- **Programs that change origin mode / scroll regions / charsets at the prompt**: overlay suppressed (guards §7.5.1);
  charset (DEC graphics) not detected → garbled box (rare).
- **Prompts that bypass the wrappers** (e.g. a framework that rewrites `PS1` after `_ts_last`, `zle reset-prompt`
  with a prompt computed outside `PS1`): no `NewCmd` → no buffer → no menu (fails safe).
- `alacritty_terminal` OSC 8 per-cell storage, `input_needs_wrap`, scroll-region accessors: **verify**; fallback is a
  vendored copy with a `tag` field (figterm precedent).
- Typeahead before the wrapper starts may be echoed twice (F36).
- Env/alias capture lags one prompt (`_ pre-cmd` runs in the background at precmd); generators use the previous env.
- Generator commands are third-party spec code run on keystrokes (as in Kiro); mitigations: no shell, detached, env
  sanitised, timeouts, `fig`/`q` stubbed, QuickJS memory/interrupt limits.
- LLM mode sends recent history + cwd to a third party: opt-in only, privacy filter, keys only from env.

macOS:
- `/bin/bash` 3.2 unsupported (D15); doctor tells the user to use zsh or Homebrew bash.
- Terminal.app has no truecolor → 256-colour themes; no synchronized output (`?2026` ignored) → possible flicker on slow redraws.
- `exec -a` does not change the process name shown by some tools on macOS (cosmetic).

Linux:
- `TERM=dumb`, linux console without UTF-8 → launch refused or glyph issues (spinner falls back to `-\|/` when
  `LANG`/`LC_ALL` lacks UTF-8 — SHOULD).
- Flatpak/Snap-sandboxed terminals: `pwsh`/`fish` paths and `/proc/<ppid>/exe` may be hidden → `_ get-shell` fails,
  wrapper falls back to `$SHELL`.

Windows:
- **ConPTY re-rendering**: ConPTY repaints from its own buffer, may reorder OSC relative to text and repaint whole
  regions after resize. Mitigations: deferred `EndPrompt`/`NewCmd` (§5.1.4), redraw after every batch, 150 ms full
  repaint after resize. **Verify** whether portable-pty enables passthrough mode / loads a side-loaded `conpty.dll` +
  `OpenConsole.exe` (shipping them next to `tabsmith.exe`, as ish does with `useConptyDll`, is the fallback plan).
- Requires Windows 10 1809+ (ConPTY); legacy conhost without VT processing → doctor error, launch refused.
- `win32-input-mode` (`CSI ?9001h`) from ConPTY must be stripped or Windows Terminal switches key encoding (§7.5.4).
- No SIGWINCH: resize via `WINDOW_BUFFER_SIZE_EVENT` + 250 ms polling; legacy conhost may not send the event.
- Child exit does not EOF the ConPTY pipe: wait on the child, then drop the master to unblock the reader.
- UTF-8 split across PTY reads must be carried before writing to the console (verify std behaviour).
- PSReadLine: `AddToHistoryHandler` timing for `PreExec` is inferred (verify); PSReadLine's ListView predictions draw
  below the prompt and can visually collide with the menu (users can set `-PredictionViewStyle InlineView`).
- cmd.exe and nushell unsupported (D20); Git Bash supported best-effort only in Windows Terminal (mintty is not a
  Windows console → launch refused); specs that shell out to `bash -c` (138 uses) or macOS tools give no results
  without Git Bash.
- Windows PowerShell 5.1 (`powershell.exe`) SHOULD: same scripts, different `$PROFILE`; `[char]27` used instead of `` `e ``.
- A running `tabsmith.exe` cannot be replaced/deleted (upgrades need all sessions closed).
