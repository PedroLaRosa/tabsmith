# tabsmith — authoritative implementation spec

`tabsmith` is a single cross-platform Rust binary (macOS, Linux, Windows) that provides terminal autocompletion: an
in-terminal dropdown menu drawn by a PTY wrapper, inline ghost text, and the CLI commands around them (`settings`,
`theme`, `inline`, `integrations`, `doctor`, `init`, `version`, `setup`, `uninstall`, …), driven by the Fig
completion specs run in an embedded QuickJS engine.

This document is the contract. It was written before the implementation and synced with it after four review
rounds; §0.4 lists the deliberate decisions those rounds introduced. Citations of the form `ish/…` point into the
inshellisense reference sources. Where this text and the code disagree, the code wins and this text is the bug.

Markers: **MUST** = required for the product to work. **SHOULD** = worth doing, not in v1's
critical path. **SKIP** = deliberately not built (reason given). **(inferred)** = not read from code.
**(verify)** = check against the pinned crate version before relying on it.

---

## 0. Ground rules

### 0.1 Names

| What | tabsmith |
|---|---|
| CLI binary | `tabsmith` |
| PTY wrapper | `tabsmith _ pty` (same binary, hidden subcommand) |
| Product name in strings | `tabsmith` |
| Data dir | `~/.tabsmith` (override `TABSMITH_HOME`) |
| Env vars | `TABSMITH_*`, `TABSMITH_SESSION_ID` |
| Launch-guard env vars | `PROCESS_LAUNCHED_BY_TABSMITH`, `SHOULD_TABSMITH_LAUNCH` |
| Dotfile block marker | `# tabsmith pre block. Keep at the top of this file.` |
| Dotfile backups dir | `~/.tabsmith/backups/` |
| Marker protocol | OSC `ESC ] 697 ;` |
| Escape hatch | `# tabsmith ignore` |
| Completion specs | embedded `@tabsmith/autocomplete-specs@1.0.0` |
| Inline model | local sources + optional OpenAI-compatible / Anthropic profiles |

### 0.2 Engineering constraints

- One crate, one binary. Rust 2021, MSRV 1.85 (local toolchain is 1.95).
- No async runtime (std threads + `std::sync::mpsc`), no SQLite, no IPC sockets, no `regex`, `dirs`, `uuid`,
  `chrono`, `dialoguer`, `globset` crates. Each was replaced by a few lines of std code (noted where it happens).
- Logs never go to the user's terminal from inside `_ pty` (they would corrupt the screen).

### 0.3 Decision log

| # | Topic | Decision | Why |
|---|---|---|---|
| D1 | How the wrapper starts | Dotfile **pre** block `exec`s `tabsmith _ pty` (bash/zsh/fish); the inner shell re-reads its rc, whose **post** block emits markers. pwsh has no `exec`: for an interactive invocation only, the pre block runs the wrapper as a child, then ends the outer shell with `[Environment]::Exit($LASTEXITCODE)`; when the wrapper could not start at all the outer shell carries on (§5.4.5). | `integrations install dotfiles` writes these blocks; ish's launch-flag injection (`--init-file`, ZDOTDIR swap) is only used by `debug shell`. ish's `Stop-Process -Id $PID` lost the exit code and killed every pwsh whose wrapper failed to start. |
| D2 | Marker protocol | OSC 697 key names. Values are **not escaped**; tabsmith's own splitter takes the whole payload up to BEL/ST, so `;` in paths survives. Markers are **stripped** before output reaches the real terminal. | Keeps `;` in values intact without ish's per-prompt byte-escaping loop. Values containing BEL/ESC/LF are unsupported (documented). |
| D3 | Edit buffer | Screen-scraped from a headless terminal model. Cells are **tagged inside the model** with injected OSC 8 hyperlinks (`tabsmith:p` prompt, `tabsmith:c<n>` command cycle n). | Per-cell tags without forking the crate; tags move with scroll/reflow, so no stored coordinates (no cursor-drift bugs). |
| D4 | Model crate | `alacritty_terminal` | Full SGR incl. dim/italic, WRAPLINE, reflow, alt screen, OSC 8 per cell. |
| D5 | Default keybindings | The default keybinding set (Enter, Tab, Shift-Tab/Up/Ctrl-P, Down/Ctrl-N, Esc, Ctrl-K description, Ctrl-R history) | — |
| D6 | Delete byte for insertions | `0x7f` (DEL) for every shell | PSReadLine treats `0x08` as Ctrl+Backspace (word delete); bash/zsh/fish bind both. |
| D7 | Drawing/restoring | Absolute cursor positioning (`CSI r;c H`) per box row; restore only the covered cell span by re-emitting model cells (truecolor exact). Every frame inside synchronized output `CSI ?2026h/l`. | Cheaper and more exact than ish's whole-row `CSI 2K` repaint with 256-colour SGR bugs; avoids DECSC single-slot clobbering. |
| D8 | Not enough room | Never scroll the real terminal. Below if it fits, else above, else shrink rows on the bigger side, else hide. | Scrolling would desync model, terminal and the shell's line editor. |
| D9 | Where the engine lives | Rust implements tokenizer, parser, suggestions, ranking; QuickJS only loads spec modules and calls spec functions. | "Rust as the main language"; pure parts unit-testable with `cargo test`. |
| D10 | Generator cache | ish semantics: fresh within TTL; past TTL `max-age` blocks, otherwise serve stale + background refresh; failures never cached. | Stale results are served while a refresh runs in the background; a failed run never poisons the cache. |
| D11 | Specs | Embedded solid `specs.tar.zst` (4.2 MB) extracted once to `~/.tabsmith/specs/<ver>/`; user dir overrides. No CDN. | Offline, pinned, reproducible. |
| D12 | `doctor` exit code | 1 if any error (and with `--strict`, any warning). | Scriptable. |
| D13 | Theme default | Unset → `dark`; `system` = query terminal background (OSC 11). | A terminal popup has its own background; OS appearance does not apply. |
| D14 | Menu size units | `autocomplete.width` = columns (default 40), `autocomplete.height` = max list rows (default 7). | A terminal popup is measured in cells, not pixels. |
| D15 | bash hooks | `PS0` (preexec) + `PROMPT_COMMAND` (precmd); requires bash ≥ 4.4. No vendored bash-preexec; a bash-preexec the user loads keeps working (D47). | Removes ~380 vendored lines and its DEBUG-trap/HISTCONTROL side effects. macOS `/bin/bash` 3.2 is unsupported (doctor error). |
| D16 | Inline ghost text | Drawn by the wrapper for every shell. Default source `history+specs` (local). LLM profiles opt-in and consulted only when no local history match exists. | The wrapper draws it, so no shell plugin is needed; the default sources are local. |
| D17 | IPC | No sockets. `tabsmith _ pre-cmd` writes `~/.tabsmith/run/<session>.json` (env + aliases); the wrapper re-reads it when its mtime changes. Live settings reload by mtime. | Cross-platform with zero code for named pipes/UDS. |
| D18 | History store | `~/.tabsmith/history.jsonl` | No SQLite C build. |
| D19 | Launch decision | Exit code 0 (launch) / 1 (don't). Deny-list of environments only; no terminal allow-list or process-tree walk. SSH and WSL allowed. | An in-terminal overlay works in any VT terminal, so no terminal allow-list is needed. tabsmith runs on the remote host itself, so SSH needs no tunnel. |
| D20 | cmd.exe, nushell | SKIP in v1 | cmd has no preexec/exit/cwd hooks, and nu redraws its prompt on every key. |
| D21 | Recency key | `tokens[commandIndex]` for both read and write | Read and write use the same key, so a recorded selection is found again. |
| D22 | Backspace count on insert | Chars of the **raw** on-screen token text; for a value typed inside an option token (`--opt=val`, `-oval`) only the value part (§6.8) | The deletion must cover exactly what is on screen, including typed escapes and quotes. |
| D23 | Parser quirks | `FOO= cmd` = complete empty assignment; string `getQueryTerm` slices at `lastIndexOf(s)+s.len()` | The slice offset is the string's own length; all spec strings are 1 char. |
| D24 | Ghost-colour exclusion | Only cells **at or after the cursor** are tested against the suggestion colour | Prevents syntax-highlighted typed text in `fg=8` from vanishing from the buffer. |
| D25 | Execute byte | `\r` | `\r` is Enter on every shell incl. PSReadLine. |

### 0.4 Decision log addendum (deliberate decisions introduced during review)

| # | Topic | Decision | Why |
|---|---|---|---|
| D26 | Insertion escaping | Allow-list: a name stays bare only if every char is alphanumeric or one of `-_./+,:@%=~` (only spaces besides → `\ `); anything else goes through `quote(shell)` (§5.3). Only the name is escaped, never `<sep>{cursor}` (§6.8). | A file or branch name such as `zz;id` (or one with `` $ ` { } `` or a leading `~`/`=`) must not run commands or expand. |
| D27 | Variable references | A non-path row that is a bare `$NAME` (pwsh: `$env:NAME` too) is inserted unquoted; file/folder rows are always escaped. | `echo $HO` → `$HOME` must stay a reference; a file named `$foo` must not expand. |
| D28 | Quote opened before the replaced text | Escape for inside that quote, add no quotes; a file closes the quote, a folder leaves it open; bash/zsh `!` becomes `"'!'"`. | Escaping as if unquoted inside a quote gives a wrong path (`"my dir/x\ y.txt"`); `!` history-expands inside `"…"`. |
| D29 | pwsh smart quotes | U+2018–U+201B count as single and U+201C–U+201E as double quotes: they force quoting and are doubled / backtick-escaped. | PowerShell ends strings on them: a crafted file name injected commands. |
| D30 | pwsh folders | Inserted as a backtick-escaped bare word (``dir` with` space/``), never `'…{cursor}'`. | A cursor left before the closing `'` hit the mid-word guard: no completion for the next path segment. |
| D31 | `special` rows (`help` template) | Delete the typed prefix like any other row. | Deleting nothing would double the typed text (`help bubuild`). |
| D32 | Control characters | Stripped from every insertion; directory entries with control chars and multi-line history rows are not offered; ghost text with control chars is never shown or accepted. | A CR in a file name, generator output or LLM reply ran the line without Enter. |
| D33 | pwsh path separator | Path templates, their trigger and query term also split on `\` (pwsh only). | Windows paths; POSIX shells keep `\` as the escape char. |
| D34 | Native `ls -1ApL` | Lists the requested directory (relative to the shell cwd); a missing one lists nothing. | A missing directory must not fall back to the cwd (`cd nonexist/` would offer the cwd's folders). |
| D35 | Bare `NewCmd` | Ignored; only `NewCmd=<own sid>` opens a cycle. | Program output (`cat` of a file) could fake a prompt and redirect keystrokes; tabsmith's scripts always send the sid. |
| D36 | `NoHistory` marker | A command the shell keeps out of its own history (leading space, fish private mode, PSReadLine sensitive/handler-rejected line, bash `HISTCONTROL`) is not written to `history.jsonl`; it still ends its cycle (`CommandEnded`). | Recording everything would let secrets reach ghost text, history rows and LLM context. |
| D37 | Recorded command text | The line submitted with Enter (or by an execute insertion) wins over the screen scrape when the scrape shows it as a whole word run followed only by shell output (§5.1 PreExec). | The scrape alone records `!!` echoes, zsh correction prompts and transient-prompt glyphs. |
| D38 | Nested integrated shells | Running commands form a stack keyed by `PID=`; a prompt finishes only its own shell's command and those nested above it. | Nested shells share the sid: the inner prompt closed the outer command with exit 0. |
| D39 | Paste without bracketed paste | From the 2nd plain printable key forwarded in one read, the rest of that read reaches the shell raw. | pwsh never enables `?2004h`: a pasted line ending in Enter was replaced by the selection. |
| D40 | Keys ahead of their echo | Printable typeahead is modelled (`unechoed`); after a key that cannot be modelled (Backspace, arrows, Ctrl-…) a key that would act on the menu/ghost is held until the reply for a later request (≤ 100 ms) (§7.3). | A menu computed for a buffer the shell no longer has inserted text instead of running the typed line. |
| D41 | Insertion lock | Re-armed while nothing of the insertion has been echoed, capped at 1 s. | On a slow shell a second accept wrote the insertion twice. |
| D42 | Inline TUIs | No menu, ghost or interception while the cursor is off the command line (fzf `--height`, fish's pager search field): the at-prompt rule of §5.6. | Their rows carry the open cycle's tag; the dropdown was drawn over fzf and took its Enter. |
| D43 | Dotfile writes | Temp file + rename (permissions kept, through symlinks incl. dangling ones) after a write-permission probe. | Truncating in place would leave the rc empty or torn after an interrupted or concurrent write. |
| D44 | `integrations uninstall` | Acts on every integration with traces on disk, not only those `is_installed()` accepts. | The scripts embed the exe path: after a binary move nothing counted as installed and nothing was removed. |
| D45 | Manual setup text | Prints the exact guarded source lines install writes. | Unguarded `. "$HOME/…"` lines would not be recognised by doctor/uninstall and would break shell start after `--purge`. |
| D46 | Terminal-injected integrations | The post blocks load Ghostty (only when the outer shell had it injected), kitty and VS Code shell integration again in the wrapped shell. | The pre block's `exec` dropped them: no OSC 133/7/633, new tabs opened in `$HOME`. |
| D47 | bash `PROMPT_COMMAND` layout | `_ts_first` / `_ts_last` are stripped from any element and re-added around the user's commands; bash-preexec's `__bp_interactive_mode` stays after `_ts_last`; the string form is kept until bash-preexec installed itself; array form on bash ≥ 5.1; inside VS Code's integration its prompt command runs ours. | DEBUG-trap preexec hooks (bash-preexec, VS Code) never fired or fired at prompt time; user prompt commands saw `_ts_first`'s `PIPESTATUS`. |
| D48 | zsh prompt themes | powerlevel10k's `_p9k_set_prompt` is wrapped; zle `line-init` and `line-pre-redraw` re-wrap a PS1 replaced after precmd. | A precmd-last wrap loses to p10k and to async/streaming themes: no markers at all. |
| D49 | fish prompt wrappers | `printf '%s'` (not `%b`), `$argv` forwarded to the user's functions, erased user prompts stay erased, `--private` forwarded to the wrapped fish, `_ pre-cmd` backgrounded inside `sh -c`. | Naive wrappers interpret backslashes (`\c` cut the prompt), break fish 4 transient prompts, resurrect erased prompts, drop private mode and clobber `$last_pid`. |
| D50 | Native install location | One fixed file, `~/.local/bin/tabsmith` (`%USERPROFILE%\.local\bin\tabsmith.exe`), replaced by renaming a checked copy over it (Windows: the running exe steps aside as `tabsmith.exe.<pid>.<nanos>.old` first); no `versions/` folder. The copy is checked against `SHA256SUMS` and must print `tabsmith <version>` before the rename. The running binary and `~/.local/bin` are compared, and the binary replaced, with symlinks resolved; staged copies an interrupted update left behind are removed once ten minutes old. | The rc scripts embed the running binary's absolute path (§5.5) and Linux resolves symlinks, so a symlink into `versions/` would pin every rc to one version. A rename keeps the old inode for running sessions (macOS SIGKILLs a signed binary overwritten in place), and a truncated download never becomes the file every new terminal execs. |
| D51 | Background update | Started by `bootstrap` just before the `_ pty` session, on its own `self-update` thread, which builds its own module (modules hold `Rc`). At most once a day (`update-check` marker, recorded before the release channel is asked), only for the native install, never with `app.disableAutoupdates`. Results go to `pty.log`, never the terminal; the new version runs from the next tab. | A module may not call another module's use case, so `terminal_session` cannot start it. cargo installs, `target/` builds and package managers update the way they were installed. |
| D52 | Signed checksums | Every release carries `SHA256SUMS.sig`, an ed25519 signature over `SHA256SUMS` by the release signing key; its public half (`scripts/release-signing-key.pub`) is compiled in. The updater refuses a release whose signature does not verify, before downloading the binary. The installers do not check it. | `SHA256SUMS` sits in the same release as the binaries: whoever can publish a release can publish matching checksums. The private key lives in the gated `release` environment. A first install trusts the release over HTTPS, as Claude Code's first install trusts its own domain. |

---

## 1. Feature list

| # | Feature | tabsmith | Notes / reason |
|---|---|---|---|
| F1 | Autocomplete dropdown | **MUST** | In-terminal overlay (§7). |
| F2 | Fig spec engine: subcommands, options, args, option chains `-abc`, `--opt=val`, `requiresSeparator`, `isRepeatable`, `exclusiveOn`, `dependsOn`, `--`, `optionsMustPrecedeArguments`, `optionsCanBreakVariadicArg`, `flagsArePosixNoncompliant`, persistent options, `loadSpec`, `isCommand/isScript/isModule`, `generateSpec`, arg `alias` directive | **MUST** | §6. |
| F3 | Diff-versioned specs (az, heroku, fig, infracost, shopify, @usermn/sdc) | **MUST** | §6.3; `@usermn/sdc` skips version detection (would run `npx`). |
| F4 | Generators: `script`, `postProcess`, `splitOn`, `custom`, `trigger`, `getQueryTerm`, `cache`, `debounce`, `scriptTimeout` | **MUST** | §6.6. |
| F5 | Templates `filepaths`, `folders`, `help` | **MUST** | §6.6.4. |
| F6 | Template `history` (7 spec uses) | SKIP | Needs a per-spec history index; returns `[]`. |
| F7 | Alias expansion (first token, from shell `alias` output) | **MUST** | §6.1.6. |
| F8 | Filtering (prefix / fuzzy via fuzzysort), `filterStrategy`, exact-match buckets, dedupe | **MUST** | §6.7. |
| F9 | Ranking: priority, recency (`sortMethod`) | **MUST** | §6.7.4. |
| F10 | Auto-execute rows (`↪`, dangerous-command double-Enter) | **MUST** | §6.7.3. |
| F11 | Visibility state machine (Esc hides for the line, hide after insertion, backspace into previous token, …) | **MUST** | §7.1. |
| F12 | Keybinding actions + `autocomplete.keybindings.*` overrides incl. `"ignore"` | **MUST** | §7.2. |
| F13 | `insertCommonPrefix` (Tab) with failure "shake" | **MUST** | Shake = BEL `\x07` to stdout. |
| F14 | Description bar; argument hint when no suggestions | **MUST** | §7.4. |
| F15 | Description pop-out (`toggleDescription`, `alwaysShowDescription`) | SHOULD | §7.4.3. |
| F16 | History rows in dropdown (`beta.history.mode`, `toggleHistoryMode` Ctrl-R) | **MUST** | Uses tabsmith's own history file (§3.5). Loading shell history through login shells: SKIP. |
| F17 | First-token completion (`autocomplete.firstTokenCompletion`) | **MUST** | Spec names + aliases + PATH executables instead of `compgen -c` (portable). |
| F18 | Themes dark/light/system/custom JSON | **MUST** | Named community themes (dracula, nord, …) SKIP: their theme files are not available. |
| F19 | `increaseSize` / `decreaseSize` | SKIP | Font scaling has no terminal meaning. |
| F20 | `selectSuggestion1..10` | SHOULD | tabsmith selects the Nth visible row. No default binding (Cmd/Ctrl+digit not deliverable). |
| F21 | Loading indicator | **MUST** | Spinner glyph in the description bar after 200 ms; list stays visible. Not drawn while the pop-out replaces the bar (§7.4.3). |
| F22 | Inline ghost text | **MUST** | All shells, drawn by the wrapper (§8). |
| F23 | Inline LLM completion | **MUST** (opt-in) | OpenAI-compatible + Anthropic profiles (§8.5). |
| F24 | Shell integration bash, zsh, fish | **MUST** | §5. |
| F25 | Shell integration pwsh (PowerShell 7+) | **MUST** | Windows' primary shell; prompt wrapping + a `PSConsoleHostReadLine` wrapper for PreExec (§5.4.5). Windows PowerShell 5.1 (`powershell`): SHOULD (same scripts, other profile path). |
| F26 | nushell, cmd.exe, xonsh | SKIP | D20. |
| F27 | Launch guards (`_ should-launch`) | **MUST** | §5.2. |
| F28 | OSC lock / session-id guard against nested shells | **MUST** | §5.1. |
| F29 | Insert text into shell with buffer reconciliation + insertion lock | **MUST** | §7.3. |
| F30 | Command history recording | **MUST** | Feeds inline + history rows (§3.5). |
| F31 | Shell env/alias capture for generators (`_ pre-cmd`) | **MUST** | D17. |
| F32 | Spec execution in the user's environment (RunProcess semantics) | **MUST** | §9.3. |
| F33 | Windows ConPTY delayed EndPrompt/NewCmd | **MUST** | §5.1.4. |
| F34 | CSI-u keyboard mode | SKIP | Only needed for Ctrl+digit bindings. |
| F35 | `# question` → translate hack | SKIP | AI translate is out of scope. |
| F36 | Typeahead capture at startup | SKIP | tabsmith enters raw mode with `TCSANOW`, so pending input is not flushed; bytes read during the startup probe are forwarded to the inner shell (may echo twice — cosmetic). |
| F37 | Shortcut specs `?` and `+` | SKIP | Legacy Fig shortcuts. |
| F38 | Dev mode spec folder (`autocomplete.developerMode` + `devCompletionsFolder`) | SHOULD (done) | One extra directory in the resolution order (leading `~` expanded); edited spec files are re-evaluated (§9.2). Dev server port: SKIP. |
| F39 | Spec CDN | SKIP | D11. `tabsmith specs update` (download npm tarball): SHOULD, post-v1. |
| F40 | `settings`, `theme`, `inline`, `integrations`, `init`, `doctor`, `setup`, `uninstall`, `version` | **MUST** | §2. |
| F41 | `diagnostic`, `completion`, `debug key-tester/shell/logs` | SHOULD | §2. |
| F42 | `issue`, `hook`, `login/logout/whoami/profile/user`, `chat/mcp/agent/translate`, `telemetry`, `launch/quit/restart/dashboard`, `_ local-state`, `debug diagnostics/fix-permissions/build/app/…` | SKIP | Out of scope (desktop app, auth, AI chat, telemetry) or need the socket tabsmith does not have. |
| F43 | Integrations `ssh`, `input-method`, `vscode`, `intellij-plugin`, `autostart-entry`, `gnome-shell-extension` | SKIP | Desktop-app plumbing. Not present in the clap tree. |
| F44 | Mouse click on a row | SKIP | Needs mouse reporting, conflicts with terminal selection. |
| F45 | `version --changelog` | SKIP | No release feed yet; add with the first release. |
| F46 | JetBrains login-shell / re-sourcing fixes, input-method prompts, login nags, inline announcement | SKIP | IDE/desktop niceties. |
| F47 | Native installer + background auto-update | **MUST** | `install.sh` / `install.ps1` / `install.cmd` one-liners fetch a release binary into `~/.local/bin`; `tabsmith update` and a daily background check at `_ pty` start keep it current (§2.10, D50–D52). |

---

## 2. CLI surface

### 2.1 Global behaviour

```text
tabsmith [-v...] [-V|--version] [-h|--help] [COMMAND]
```

- No command → print help, exit 0.
- `-v` (count, global): log level 1=warn, 2=info, 3=debug, ≥4=trace (else `TABSMITH_LOG_LEVEL`, default warn); also
  mirrors CLI logs to stderr. `_ pty` never writes logs to the terminal.
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
tabsmith update                                                       "Update tabsmith to the latest release"
tabsmith version                                                      (hidden) "Show the version"
tabsmith diagnostic [-f FMT]                     (alias: diagnostics) "Run diagnostic tests"          SHOULD
tabsmith completion [bash|zsh|fish|powershell|elvish]  (hidden, default zsh)                         SHOULD
tabsmith debug key-tester | shell [bash|zsh|fish|pwsh] | logs                                       SHOULD
tabsmith _ (alias: internal, hidden)
         _ pty [--shell PATH] [-- ARGS...]    (ARGS starting with `-`: extra shell args; else a command)
         _ should-launch
         _ get-shell
         _ pre-cmd [--alias TEXT]     (allow_hyphen_values on --alias)
```

`-s/--silent` and `-f/--format` belong to the parent subcommand, so they precede the integration name:
`tabsmith integrations uninstall --silent all`.

### 2.3 `settings`

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
| `settings open` | Create the file if missing; `$EDITOR <path>` if set (split on whitespace, so `code --wait` works; ponytail: no quoted paths) and wait; else macOS `open -t <path>`, Windows `notepad <path>`; else `error: The EDITOR environment variable is not set`. Exit 1 when the editor fails. | 0/1 |
| `settings list` | Configured keys sorted. Plain: `key = <compact json>` per line. json: the whole object. | 0 |
| `settings list --all` | Every catalog key (§3.3). Plain: `key\ttype\tdefault\tvalue` with a header line. json: `[{key,type,default,description,value}]`. | 0 |

### 2.4 `theme`

- `--folder`: print `~/.tabsmith/themes` (create if missing).
- `--list`: built-ins `dark`, `light`, `system` then `*.json` stems from the folder, sorted, deduped.
- `THEME`: if `<folder>/<THEME>.json` exists → parse `{author?:{name?,twitter?,github?}, version?, theme?, shade0..7?, accent0..7?}`
  (parse error → exit 1); print `\n› Switching to theme '<THEME bold>'` + ` by <name bold>` when `author.name`,
  then optional `  🐦 <twitter>` / `  💻 github.com/<github>` lines, then `\n`; set `autocomplete.theme`. Built-in name →
  `› Switching to theme '<THEME bold>'`, set. Else `error: '<THEME>' does not exist in <folder>`.
- No args: print `autocomplete.theme` (unset → `dark`).

### 2.5 `inline`

| cmd | Behaviour | stdout | exit |
|---|---|---|---|
| `enable` / `disable` | set `inline.enabled` true/false (running wrappers pick it up via mtime reload) | `Inline enabled` / `Inline disabled` (magenta) | 0 |
| `status` | `inline.enabled` (default true) | `Inline is <enabled\|disabled bold>` | 0 |
| `set-customization NAME` | NAME ∈ {`history`, `specs`, `history+specs`} ∪ names of the **valid** `inline.profiles.*` objects. Unknown or invalid → `error: Customization not found`. Else set `inline.source`=NAME, print `Customization <NAME bold> selected`. | | 0/1 |
| `set-customization` (no arg, tty) | Numbered list (`1) history+specs - Local history, then completion specs` …, valid profiles `N) <name> - <provider> <model>`, last `N) None`), prompt `Select a customization: `. `None` → remove `inline.source`, print `Customization unset`. Empty/invalid input or non-tty → `error: No customization selected`. | | 0/1 |
| `show-customizations [-f]` | Plain: for each source `<name bold>[ (selected)]` + newline + description + blank line; an invalid profile prints `<name> (invalid: <reason>)` + blank line. json: `[{name, kind:"builtin"\|"profile", description, selected, provider?, model?, base_url?, api_key_env?}]` (invalid profile: `description: "invalid: <reason>"`, no provider fields). | | 0 |

Profiles are created with `settings`, e.g.
`tabsmith settings inline.profiles.claude '{"provider":"anthropic","model":"claude-opus-5-5","api_key_env":"ANTHROPIC_API_KEY","effort":"low"}'`.
`show-customizations` prints that example when no profile exists.

### 2.6 `integrations`

- `install dotfiles [SHELL]`: shells = `[SHELL]` or `[bash, zsh, fish]` (+ `pwsh` when `pwsh` is on PATH). Each
  integration of each shell: if `is_installed()` is Ok → skip, else `install()`. Errors collected as
  `"<describe bold>: <message>"` (enumeration errors as `"<shell>: <error>"`), reported as one error
  `"\n\n" + errs.join("\n\n")` (exit 1). Output unless `--silent`: `Installed!` if something was installed (also sets
  state `doctor.prompt-restart-terminal`), else `Already installed`.
- `install all` = `install dotfiles` (all shells).
- `uninstall dotfiles [SHELL]|all`: uninstall each integration with **traces on disk** (D44): a script file, the fish
  `conf.d` file, or one of the four exact dotfile lines of §5.5 — also when `is_installed()` fails (moved binary,
  lines appended after the post block). `Uninstalled!` if any, else `Not installed`; exit 0 unless an error.
- `reinstall` = uninstall (silently) then install.
- `status dotfiles [SHELL]|all`: **honours SHELL**. Plain: `✔ <describe>` (green) / `✘ <describe>` (red);
  then `<shell>: <error>` lines for enumeration errors. json: `{"integrations":[{"installed":bool,"description":str,"shell":str,"file_name":str}],"errors":[{"shell":str,"error":{"title":str,"message":str|null}}]}`.
- `describe()`: dotfile → `"<shell> pre post into <file_name>"` (e.g. `bash pre post into .bashrc`); fish conf.d file → `"fish <pre|post>"`.

### 2.7 `init`

`tabsmith init <shell> <pre|post> [--rcfile NAME]` prints the integration code + `\n`, always exit 0; on error prints
`# Could not load source: <err>` (a comment, so `eval` never breaks). Output composition in §5.3. `--rcfile` is
accepted and logged at debug level (no special case: the pre script's own interactive guard covers the Linux
`profile` case). `TABSMITH_INIT_SNAPSHOT_TEST=1` makes output deterministic (bash/zsh pre
`TABSMITH_SHELL='/bin/zsh'`, launch value 0, binary path `tabsmith`); the snapshots live in `tests/snapshots/` and are
re-blessed with `TABSMITH_BLESS=1 cargo test integrations_init_snapshots`.

### 2.8 `doctor`

Flags `-a/--all` (run every check, print every result, never fix, never abort), `-s/--strict` (soft checks become
normal; warnings count as failures for the exit code). No `--format`.

Flow:
1. Unix euid 0: stderr `Running doctor as root is not supported.` (red bold); without `--all` also
   `If you know what you're doing, run the command again with --all.` → exit 1.
2. Without `--all` and with stderr a tty: spinner `Running checks...` on stderr (Braille dots, 80 ms), cursor hidden;
   restored when the first result prints, on exit, and on Ctrl-C (exit 1).
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
| `Let's check your dotfiles...` | `<shell> <~/path> integration check` (one per integration of bash, zsh, fish, pwsh) | Normal if it is the current shell (parent process name; Windows: Toolhelp snapshot + `QueryFullProcessImageNameW`), Soft if that shell is on PATH, else NoCheck (and no `pwsh` spawn) | `is_installed()` error text; info `Run tabsmith integrations install dotfiles <shell> to reinstall shell integrations for <shell>` | `install()`; none for `Permission denied` |
| `Let's make sure tabsmith is set up correctly...` | `tabsmith data dir exists` | Normal | `~/.tabsmith` missing → `No directory at path <p>` | create it |
| | `Integration scripts point at this binary` | Normal | the script files (and fish `conf.d` files) of the **current** integrations of every shell on PATH reference (text between their first two `'`) a path ≠ `current_exe()` → `Integration scripts reference <old>`. Orphaned files in `shell/` are ignored (nothing sources them, install never rewrites them). | `install dotfiles` for those shells |
| | `tabsmith settings corruption` | Normal | settings.json parse error → `tabsmith settings file is corrupted` | copy to `settings.json.bak-<unix>` then write `{}` |
| | `Completion specs installed` | Normal | `~/.tabsmith/specs/<ver>/index.json` missing → `Completion specs are not extracted` | extract |
| | `Windows Console Check` (Windows) | Normal | `GetConsoleMode` fails on stdin/stdout → `Windows Console APIs are not supported in this terminal` + MinTTY/winpty info lines | – |
| | `Windows version` (Windows) | Normal | build < 17763 → `Windows 10 1809 or newer is required (ConPTY)` | – |
| `Let's check if your environment is supported...` | `tabsmith terminal integrations` | Normal | in order: `TERM_PROGRAM=WarpTerminal` → `WarpTerminal is not supported`; parent `cmd.exe` → `CMD isn't supported, use PowerShell 7 (pwsh) instead`; `INSIDE_EMACS` → `Emacs is not supported`; SecureCRT → `SecureCRT is not supported`; `PROCESS_LAUNCHED_BY_TABSMITH` → `tabsmith can not run in a process it launched`; CI → `Doctor can not run in CI`; then `TABSMITH_TERM`: == version → Ok; unset → `tabsmith is not running in this terminal, please try restarting your terminal`; other → `This terminal is not running with the latest integration, please restart your terminal` (info `TABSMITH_TERM=<v>`) | – |
| | `Session check` | Normal | NoCheck when `TABSMITH_SESSION_ID` is unset or the session shell (parent process, else `TABSMITH_SHELL`) is pwsh, which never runs `_ pre-cmd`; else `~/.tabsmith/run/<sid>.json` missing → Warning `Shell has not reported its environment yet (tabsmith _ pre-cmd did not run)` | – |
| | `Autocomplete is enabled` | Normal | `autocomplete.disable` = true → `Autocomplete disabled.` info `To fix run: tabsmith settings autocomplete.disable false` | – |
| | `Autocomplete dev mode` | Normal | `autocomplete.developerMode` = true → Warning `autocomplete.developerMode is enabled` | – |
| | `Inline` | Normal if inline enabled, else Soft | disabled → Warning `Inline is disabled, to re-enable run: tabsmith inline enable`; `inline.source` names a missing/invalid profile → Error `Inline source '<n>' is not a valid profile`; profile key env var unset in current env (and base_url not localhost) → Error `<API_KEY_ENV> is not set` | – |
| `Let's check if your system is compatible...` (printed, never counted or aborting) | `OS is supported` | Normal | Linux, macOS ≥ 11 (`sw_vers`), Windows build ≥ 17763 (`cmd /C ver`) → Ok; else Warning `macOS <v> is not supported (needs macOS 11 or newer)` / `Windows 10 1809 or newer is required (ConPTY)` / `<os> is not supported` | – |
| | `Bash is new enough` | Soft if current shell is bash else NoCheck | `<parent bash> --version`, first dotted version < 4.4 → `Bash <v> is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh` | – |
| | `Fish is up to date` | Normal (skip if no `fish`) | `< 3.3.0` → `your fish version is outdated (need at least 3.3.0, found <v>)` | – |

### 2.9 `setup` / `uninstall`

`[Y/n]` prompts (both commands): empty, `y`, `yes` (any case) = yes; anything else = no; end of input (Ctrl-D) = no
(a newline is printed). Without a tty → `error: You must run with --no-confirm if unattended`.

`setup [--no-confirm] [--force]`:
1. Unix euid 0 and not `--force` → stderr `Installing as root is not supported.` + `If you know what you're doing, run the command again with --force.`, exit 1.
2. Without `--no-confirm` ask
   `Do you want tabsmith to modify your shell config (you will have to manually do this otherwise)? [Y/n] `.
3. Yes → `integrations install dotfiles` for every shell of bash, zsh, fish, pwsh found on PATH; print
   `Installed!` / `Already installed` (or `Could not automatically install:` + the error, then step 4).
4. No (or step 3 failed) → print the manual instructions: for the top and the bottom of the rc file, the
   **exact** guarded source lines install writes for `bash:` (`.bashrc`), `zsh:` (`.zshrc`) and, with pwsh on PATH,
   `pwsh ($PROFILE):` (D45); then write only the script files under `~/.tabsmith/shell/` (dotfiles untouched).
5. Extract the specs (§9.1; error → exit 1). After an automatic install only:
   `Restart your terminal (or open a new tab) to start using tabsmith.`

`uninstall [-y] [--purge]`: without `-y` ask `Are you sure want to continue uninstalling tabsmith? [Y/n] `
(No → `Cancelled`, exit 1). Then uninstall every integration of bash, zsh, fish (+ pwsh on PATH), continuing past
errors and printing them to stderr; an integration whose dotfile cannot be written keeps its script files. `--purge`
then removes `~/.tabsmith` (missing is fine; a running session no longer recreates it, §3.1). Any error →
`Uninstalled tabsmith with errors (see above).`, exit 1. Else print `Uninstalled tabsmith.`, with `--purge`
`Restart open terminals to stop tabsmith in them.`, then
`Remove the binary with your package manager, or delete <current_exe>.` (a running exe cannot delete itself on
Windows), exit 0.

### 2.10 Other commands

- `version` → `tabsmith <ver>\n`.
- `update`: only the native install (`~/.local/bin/tabsmith[.exe]`) updates itself. Reads the latest release's
  `VERSION`; a version that does not parse is an error and a version not newer than the running one prints
  `tabsmith is up to date (<ver>)`. Otherwise it downloads that release's `SHA256SUMS` and `SHA256SUMS.sig` and
  refuses checksums the release signing key did not sign (D52), then downloads `tabsmith-<os>-<arch>[.exe]`, checks
  it against `SHA256SUMS`, and installs it as D50 says, printing `Updated tabsmith from <old> to <new>. Open a new
  terminal to use it.` Run from anywhere else: `error: tabsmith runs from <path>, not ~/.local/bin; update it the
  way you installed it (e.g. cargo install --path .)`. Every failure exits 1 and leaves the binary as it was.
  `app.disableAutoupdates` does not apply. The same steps run in the background at `_ pty` start (D51), at most
  once a day and silently skipping a non-native install.
- `diagnostic [-f]` (SHOULD): Plain = hand-written TOML (`[tabsmith] version, os`, `[environment] cwd, cli-path,
  shell-path (= $SHELL), shell-version, terminal (TERM_PROGRAM/WT_SESSION/TERM), in-ssh, in-ci, in-wsl`, `[env-vars]` =
  all `TABSMITH_*` plus `SHELL, TERM, COLORTERM, ZDOTDIR, PATH`, with `/<username>` replaced by `/USER`; null values
  omitted); json = same object.
- `completion [SHELL]` (SHOULD): `clap_complete::generate(shell, Cli::command(), "tabsmith", buffer)`, then one write
  (a closed pipe is an error, not a panic).
- `debug key-tester` (SHOULD): raw mode; each read prints `<n> bytes : "<escape_ascii>" : [0x..,...]`; Ctrl-D (0x04) prints `ctrl-d` and exits.
- `debug shell [SHELL]` (SHOULD; default `$SHELL`, Windows pwsh): run `_ pty` with only the post integration. The output of `init <shell> post` is written to a fresh `~/.tabsmith/run/debug-shell-<pid>/`
  (0700; never the shared temp dir) and loaded: bash `--noprofile --rcfile <dir>/bashrc`; zsh `ZDOTDIR=<dir>` with
  `<dir>/.zshrc`; fish `--no-config -C 'set -q fish_color_autosuggestion[1]; or set -g fish_color_autosuggestion
  brblack; source <rc>'` (`--no-config` also skips fish's default theme); pwsh `-NoProfile -NoExit -Command ". <rc>"`;
  paths quoted with `quote(shell)`. The shell binary is `$SHELL` when it is that shell, else the name. Env
  `TABSMITH_DEBUG_SHELL=1`. Prints `Starting tabsmith debug shell` / `Ending tabsmith debug shell` (write errors
  ignored: a closed terminal never panics), removes the dir, exits with the shell's code.
- `debug logs` (SHOULD): print the log file paths and the last 50 lines of `pty.log`.

### 2.11 Internal `_` commands

| cmd | Behaviour | Used by |
|---|---|---|
| `_ pty [--shell PATH] [-- ARGS...]` | The wrapper (§4.4). | pre scripts, `debug shell` |
| `_ should-launch` | Runs §5.2; prints one reason line (`✅ …`/`❌ …`) to stdout; exit 0 or 1. | manual debugging |
| `_ get-shell` | Print parent process exe path (no newline); exit 1 if unknown. Linux `readlink /proc/<ppid>/exe`; macOS `libc::proc_pidpath(ppid)`; Windows Toolhelp snapshot + `QueryFullProcessImageNameW`. | manual debugging (`init` calls the same function in-process) |
| `_ pre-cmd [--alias TEXT]` | If `TABSMITH_SESSION_ID` is unset or not `[A-Za-z0-9_-]+` → exit 1. Write `~/.tabsmith/run/<sid>.json` = `{"env":{<all env vars>},"alias":TEXT,"ts":<unix ms>}` atomically (temp + rename), mode 0600, `run/` 0700; `run/` is created only inside an existing data root (a purged `~/.tabsmith` is not recreated). Then delete run files untouched for 24 h (a shell that exits right after a prompt leaves its file behind: the background `_ pre-cmd` rewrites it after the wrapper's cleanup). Exit 0. | bash/zsh/fish post scripts, every prompt, backgrounded |

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
  update-check                   empty marker; its mtime is the last background update check (D51)
  shell/<rc>.<pre|post>.<shell>  integration script files sourced from dotfiles (§5.5)
  themes/<name>.json             custom themes
  specs/<ver>/…                  extracted embedded specs (read-only); `.last-used` touched at every engine start
  specs/user/…                   user spec overrides (same layout as the npm build/ dir)
  run/<session>.json       0600  (dir 0700) env + aliases from `_ pre-cmd`; deleted when the wrapper exits,
                                 pruned after 24 h untouched
  run/debug-shell-<pid>/         (0700) rc files of `debug shell`, removed when it ends
  logs/cli.log, logs/pty.log 0600 (dir 0700) append, no rotation; one line per record: `<unix ms> <Level> <sid|-> <msg>`
  backups/<unix-secs>/<file>     dotfile backups before every install edit
```

The writers that run inside a live session (history append, `_ pre-cmd`, the log files, the recency save) write only
while the data root exists, so a session still running after `uninstall --purge` does not bring `~/.tabsmith` back
(a new wrapper, settings writes and `setup` do create it).

### 3.2 Storage format and semantics

- **One flat JSON object, keys are literal dotted strings** (`"autocomplete.theme": "dark"`), values arbitrary JSON.
- Load: missing or blank file → `{}` (not created until a save). Parse error or a non-object → `<path> is corrupted`
  (CLI exit 1; the wrapper logs and uses defaults).
- Save: read-modify-write; write `serde_json::to_string_pretty` + `\n` (keys sorted: `serde_json::Map` without
  `preserve_order` is a BTreeMap) to a temp file next to the **symlink target** (`canonicalize`), mode 0600, then
  rename over that target: a symlinked `settings.json` (dotfiles repo) stays a symlink. Concurrency: last writer wins
  (ponytail: no file lock; add `fs::File::lock` (std ≥ 1.89) if concurrent CLI writes ever matter). The
  recency map is merged with the file on save instead (§6.7.5).
- Typed getters: `get_bool` = `Value::as_bool` (the string `"true"` is not true), `get_string` = `as_str`,
  `get_i64` = `as_i64`; wrong type → default.
- The wrapper re-reads `settings.json` whenever its mtime changed, checked at every `NewCmd` and every 2 s, then
  rebuilds the keymap and theme and forwards the new map to the engine thread (which clears its parse caches,
  generator slots and first-token cache). `run/<sid>.json` is polled the same way (`EngineMsg::Session`).
- `state.json`: same format. Keys: `autocomplete.recency` (`{cmd: {name: unix_ms}}`), `doctor.prompt-restart-terminal` (bool).

### 3.3 Settings catalog (`settings list --all` prints this table)

Autocomplete:

| key | type | default | semantics |
|---|---|---|---|
| `autocomplete.disable` | bool | false | Never show the dropdown and never intercept a key for it, Tab/show/toggle included (ghost text unaffected). |
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
| `autocomplete.width` | number (columns) | 40 | Menu width (at least 10), clamped to terminal width (D14). |
| `autocomplete.height` | number (rows) | 7 | Max visible list rows (at least 1) (D14). |
| `autocomplete.developerMode` | bool | false | Load specs from `devCompletionsFolder` first; disable spec/parse/CLI-version caches; edited spec files are re-evaluated. |
| `autocomplete.devCompletionsFolder` | string\|null | null | Dev spec folder (leading `~` = home). |
| `autocomplete.keybindings.<keystroke>` | string | – | Action id or `"ignore"` (§7.2). |
| `beta.history.mode` | `"off"`\|`"history_only"`\|`"show"` | `"off"` | History rows in the dropdown (§6.10). |

Inline:

| key | type | default | semantics |
|---|---|---|---|
| `inline.enabled` | bool | true | Ghost text on/off. Live (mtime reload). |
| `inline.source` | string | `"history+specs"` | `history` \| `specs` \| `history+specs` \| `<profile name>`. Built-in names are reserved. |
| `inline.profiles.<name>` | object | – | `{provider:"openai"\|"anthropic", model:string (required), base_url?:string, api_key_env?:string, max_tokens?:int, effort?:string}`. Defaults: openai `base_url=https://api.openai.com/v1`, `api_key_env=OPENAI_API_KEY`, `max_tokens=64`; anthropic `base_url=https://api.anthropic.com`, `api_key_env=ANTHROPIC_API_KEY`, `max_tokens=1024`. `effort` (anthropic only) is sent as `output_config.effort`. **API keys are never stored in settings.** |
| `inline.debounceMs` | number | 300 | Quiet time before an LLM request. |
| `inline.historyCount` | number | 49 | Max history lines sent to the LLM (0 = none). |

App:

| key | type | default | semantics |
|---|---|---|---|
| `app.disableAutoupdates` | bool | false | Never update in the background (D51); `tabsmith update` still works. |

### 3.4 Environment variables

| var | meaning |
|---|---|
| `TABSMITH_HOME` | Data/config dir override. |
| `TABSMITH_SETTINGS_PATH` | settings.json override (tests). |
| `TABSMITH_TERM` | Set by the wrapper for the child = wrapper version. "Already inside tabsmith". |
| `TABSMITH_TERM_TMUX` | Set for the child (= version) iff `TMUX` was set; lets each tmux pane get its own wrapper. |
| `TABSMITH_SESSION_ID` | 32 lowercase hex chars, generated by the wrapper (`format!("{:016x}{:016x}", nanos_since_epoch, pid ^ nanos.rotate_left(17))`; no uuid crate). |
| `TABSMITH_SHELL` | Shell exe the wrapper spawns (exported by the pre scripts; doctor reads it as the session shell); removed from the child env. |
| `TABSMITH_IS_LOGIN_SHELL` | `1` → child gets `--login` (bash, zsh and fish all accept it); removed from child env. |
| `TABSMITH_FORCE_LAUNCH` / `TABSMITH_TERM_DISABLED` | Force / forbid launching the wrapper. |
| `TABSMITH_GHOSTTY_INJECTED` | `1`, exported by `pre.sh` when the outer shell had Ghostty's integration injected; the post block reloads it, then unsets the variable (§5.4). |
| `PROCESS_LAUNCHED_BY_TABSMITH` | Set on every generator process. |
| `TABSMITH_LOG_LEVEL` | `error\|warn\|info\|debug\|trace` (default `warn`; `-v` flags win). |
| `TABSMITH_INIT_SNAPSHOT_TEST` | Deterministic `init` output. |
| `TABSMITH_BLESS` | Tests: rewrite the `init` snapshots. |
| `TABSMITH_DEBUG_SHELL` | Set by `debug shell`. |
| read: `TMUX`, `TMUX_PANE`, `INSIDE_EMACS`, `TERM_PROGRAM`, `__CFBundleIdentifier`, `__PWSH_LOGIN_CHECKED`, `CI`, `VSCODE_RESOLVING_ENVIRONMENT`, `TERM`, `COLORTERM`, `WT_SESSION`, `EDITOR`, `ZDOTDIR`, `XDG_CONFIG_HOME`, `SHELL`, `HOME`, `USERPROFILE`, `PATH`, `PATHEXT`, `ComSpec`; scripts: `HISTCONTROL`, `GHOSTTY_RESOURCES_DIR`, `KITTY_SHELL_INTEGRATION`, `KITTY_INSTALLATION_DIR`, `VSCODE_INJECTION`, `VSCODE_SHELL_INTEGRATION` | see §5.2, §5.4, §5.5, §7.7, §9.3 |

### 3.5 History file

`~/.tabsmith/history.jsonl`, one JSON object per line:
`{"command":str,"shell":str,"pid":int|null,"session_id":str,"cwd":str|null,"start_time":unix_s,"end_time":unix_s|null,"exit_code":int|null,"hostname":str|null}`
(`hostname` is always null). Appended (mode 0600) by the engine
thread on `CommandFinished` (§4.3), only for non-blank commands and only while the data root exists. A command marked
`NoHistory` (D36) ends with `CommandEnded` instead and is never written. Loaded at engine start (last 5000 lines kept
in memory); when the file exceeds 10 000 lines it is re-read and rewritten with the last 5000 (temp + rename;
ponytail: no lock, a line another session appends in between is lost).

---

## 4. Architecture

### 4.1 Modules (single crate, `src/`)

| path | responsibility |
|---|---|
| `main.rs` | Entry point: parses the CLI, initialises the logger, runs `bootstrap::cli::run`, exits with its code. |
| `bootstrap/` | Composition root. `cli.rs`: clap tree, dispatch, `version` and `completion` (§2.10), the internal `_` commands (§2.11; `_ pty` starts the background update first, D51), `error:` printing. `engine_server.rs`: the engine thread (§4.3, §6), which serves each `EngineMessage` (`engine_message.rs`, `engine_inbox.rs`) with the completion, history and ghost_text use cases and sends the replies. An `Llm` message goes to `RequestLlmGhostTextUseCase`; a deferred answer runs on an `llm` worker thread (§8.4). `engine_inbox.rs` is the engine thread's single channel (its messages and the spec runtime's events) and implements specs' `RuntimeEventSource`. `channel_completion_engine.rs` implements terminal_session's `CompletionEngine` over that channel and starts the engine thread (`start_completion_engine`). `bridges.rs`, `diagnostics_bridges.rs`: ports that one module declares, implemented over another. |
| `modules/` | The eleven hexagonal modules below, each a vertical slice. Its `mod.rs` only declares them, as `main.rs` declares only `bootstrap` and `modules` (plus the test-only `architecture_tests`) besides `fn main`. |
| `modules/shared/` | Kernel every module may use; it imports no module. `Shell`, `DataDirectory` (paths, §3.1), shell quoting, the `Clock` port; in `infrastructure/` (shared keeps its implementations at the top of `infrastructure/`, with no `adapters/`): `SystemClock`, `data_directory_from_env.rs` (resolves the data directory from `TABSMITH_HOME`, `TABSMITH_SETTINGS_PATH`, `HOME` / `USERPROFILE`; §3.1, §3.4), `SharedDependencies` (what bootstrap hands every module's `build_<module>_module`), flat-JSON document store (settings/state, §3.2), atomic file writes, `logger.rs` (file logger, no crate), OS helpers, output format, terminal style. |
| `modules/settings/` | Typed getters (`Settings`), catalog table (`settings_catalog.rs`, §3.3), wildcard matcher (`SettingKeyPattern`), the `settings` command (§2.3). |
| `modules/history/` | History store (§3.5, §8): `history.jsonl` through `JsonlHistoryRepository`; `RecordCommandUseCase` records each executed command. |
| `modules/shell_integration/` | `init` generation (§5.3), should-launch decision (§5.2), `_ get-shell`, dotfile install/uninstall/status (§5.5), embedded scripts (`include_str!` of `shell/*`, in `embedded_shell_scripts.rs`), setup/uninstall flows (§2.9). |
| `modules/specs/` | Spec model + normalisation (§6.2), spec loading/versioning and name → file resolution (§6.3), bundle extraction/cleanup and index (§9.1), QuickJS host: prelude, module wrapping, function table, exec, event pump (`quickjs_spec_runtime.rs`, `quickjs_module_wrapper.rs`, `process_executor.rs`; §9). |
| `modules/completion/` | Tokenizer (§6.1), argv state machine (§6.3–6.4), suggestion assembly, triggers/cache, templates, filtering, fuzzysort, ranking, insertion text, history rows, first-token items (§6.6–6.10), generator runs, recency and the request pipeline (§6) as use cases (`CompleteCommandLineUseCase`). |
| `modules/ghost_text/` | Ghost sources (§6.11), LLM clients, completion cache, validation (§8), the `inline` command (§2.5). |
| `modules/dropdown/` | Pure UI state: visibility machine, actions, selection/scroll, common prefix (`Menu`, §7.1); keystroke grammar, keybinding table (`Keymap`, §7.2.2–7.2.3); themes, colour conversion, layout, row icons (§7.4, §7.7); the `theme` command (§2.4). |
| `modules/terminal_session/` | Wrapper: spawn, raw mode / console modes, threads, main loop, interception, insertion/reconciliation, echo hold, resize, panic guard, fallback (`infrastructure/pty/`: spawn, raw mode, threads, run loop, panic guard, fallback and `SessionMessage`; key routing, echo hold, insertion reconciliation and resize debounce in `domain/`; §4.3–4.4). `SessionUseCases` wires the session's use cases over `SessionPortsParts`, in production and in tests. OSC 697 splitter, marker tracker + cell tagging, Windows deferral, edit-buffer extraction, output filter (§5.1, §5.6, §7.5.4). Stdin key decoding (`KeyDecoder`, §7.2.1). Frame bytes (menu, description, pop-out, ghost), restore spans, full repaint, icon pixels and the kitty graphics protocol (`overlay_renderer.rs`, `icon_assets.rs`, `kitty_graphics.rs`; §7.4.2, §7.5). The `_ pty`, `_ pre-cmd`, `debug shell` and `debug key-tester` commands. |
| `modules/diagnostics/` | Checks and the run loop (`doctor`, §2.8), the `diagnostic` report and `debug logs` (§2.10). |
| `modules/self_update/` | The native install's update (§2.10, D50–D52): `UpdateTabsmithUseCase` with a `Manual` (`update`) or `Scheduled` (background) trigger; release assets, `SHA256SUMS` parsing, the install location and the daily schedule in `domain/`; ports for the release channel (`GithubReleaseChannel`, ureq), SHA-256 and the ed25519 checksum signature (ring), and the executable (`FileInstalledExecutable`); the `update-check` marker repository; the `update` command and `start_background_update`. |
| `shell/` (repo root) | `pre.sh`, `post.zsh`, `post.bash`, `pre.fish`, `post.fish`, `pre.ps1`, `post.ps1`. |
| `assets/` (repo root) | `specs.tar.zst`, `SPECS_VERSION`, `SPECS_LICENSE` (MIT, Hercules Labs). THIRD_PARTY notices (semver ISC, yaml ISC, strip-json-comments MIT): not shipped yet. `icons/`: the row icons (PNGs and `template.rgba`, §7.4.2). |

Each module is a vertical slice; a folder exists only when it has content:

```
src/modules/<module>/
  mod.rs            declares domain, application, infrastructure; nothing else
  domain/           entities/, value_objects/, services/, repositories/ (traits), the module's error
  application/      use cases (*_use_case.rs), application services shared by several use cases (e.g. GeneratorRunner, IntegrationInstaller, CheckEvaluator), DTOs, ports/ (traits)
  infrastructure/   adapters/ (implement the ports, with the two exceptions below), cli/ (clap handlers), <module>_module.rs (builds the use cases)
```

Layers depend inward only: `domain` ← `application` ← `infrastructure`. `domain` and `application` import no `infrastructure`, no external crate (except `unicode_width`) and call no `print!`-style macro; ports (traits in `domain/repositories/` or `application/ports/`) are implemented by `infrastructure/adapters/`, with two exceptions: the kernel's `SystemClock` sits at the top of `shared/infrastructure/`, and history's memory-only `InMemoryHistoryRepository` stays in `domain/repositories/`. A module never uses another module's use cases or `infrastructure` (only `shared` is open to every module) and nothing depends on `bootstrap`: a port that one module declares and another serves is implemented in `bootstrap`, the only place that wires modules together. `src/architecture_tests.rs` is the test that enforces these rules on `shared` and the ten modules above; it does not check where a port is implemented.

### 4.2 Crates (as pinned in `Cargo.toml`)

| crate | version | use |
|---|---|---|
| `clap` (derive) | 4.5 | CLI. |
| `clap_complete` | 4.5 | `completion` (SHOULD). |
| `serde` (derive), `serde_json` | 1 | Settings, specs, history, IPC files. |
| `anyhow` | 1 | Errors. |
| `portable-pty` | 0.9 | PTY: `native_pty_system().openpty(PtySize)`, `slave.spawn_command(CommandBuilder)`, `master.try_clone_reader()`, `master.take_writer()`, `master.resize()`. ConPTY on Windows (no side-loaded `conpty.dll`). |
| `alacritty_terminal` | 0.26 | Headless model: `Term::new(Config{scrolling_history: 2000,..}, &TermSize, VoidListener)`, `vte::ansi::Processor::advance(&mut term, &[u8])` (synchronized updates applied at once), `term.grid()`, `TermMode::{ALT_SCREEN, SHOW_CURSOR, ORIGIN, INSERT}`, `Flags`, `grid.cursor.input_needs_wrap`, `Cell::hyperlink()/set_hyperlink()` (OSC 8 is stored per cell: the D3 tags need no vendored crate). Its scroll region is private: DECSTBM is tracked by the output filter. |
| `crossterm` | 0.29 | `terminal::{enable_raw_mode, disable_raw_mode, size}` only. Never its event reader (loses raw bytes). |
| `signal-hook` | 0.3 | Unix SIGWINCH/SIGTERM/SIGHUP thread; doctor's SIGINT cursor restore. |
| `libc` | 0.2 | Unix: `setsid` in generator `pre_exec`, `kill(-pgid)`, `waitpid`, `select`, `atexit`, `geteuid`, `getppid`, macOS `proc_pidpath`. |
| `windows-sys` | 0.59 | Windows: console modes, `ReadConsoleInputW`, CP_UTF8, Toolhelp parent process, `CREATE_NO_WINDOW`. Job-object functions are declared by hand (no `Win32_System_JobObjects` feature). |
| `rquickjs` | 0.14 | Spec host (`Runtime::{set_interrupt_handler, set_memory_limit}`, `Ctx::{eval_with_options, execute_pending_job}`); no `loader`/`futures` features, no `Persistent` (§9.2). |
| `zstd` | 0.13 | Spec archive. |
| `tar` | 0.4 | Spec archive. |
| `ureq` | 3 (`json` feature) | LLM HTTPS (rustls); release downloads for `update`. |
| `ring` | 0.17 | SHA-256 of release binaries and the ed25519 check of `SHA256SUMS.sig` (D52); already built in through ureq → rustls. |
| `unicode-width` | 0.2 | Display width for layout, truncation, ghost clipping. |

### 4.3 Threads and data flow

```text
 real terminal ── stdin bytes ──► [stdin thread] ──SessionMessage::Stdin────────────────────────────┐
       unix: blocking read(0)                                                                       │
       windows: ReadConsoleInputW; KEY_EVENT (bKeyDown) uChar → UTF-8 bytes (join surrogate pairs)  │
 [signals thread] (unix SIGWINCH/TERM/HUP) ──SessionMessage::{Resize, Term}────────────────────────►┤
 [stdin thread, windows] WINDOW_BUFFER_SIZE_EVENT ──SessionMessage::Resize─────────────────────────►┤
 [resize thread, windows] 250 ms size poll ──SessionMessage::Resize────────────────────────────────►┤
 [pty reader] master.read(16 KiB), one credit per read ──SessionMessage::{Pty, PtyEof}─────────────►┤
 [child wait]  waitpid / child.wait() ──SessionMessage::ChildExit(code)────────────────────────────►┤
 [engine] ──SessionMessage::{Completion, Ghost}────────────────────────────────────────────────────►┤
 [llm worker] ──SessionMessage::Ghost──────────────────────────────────────────────────────────────►┤
 [self-update] (started before the session, D51; sends nothing: it replaces the binary file and logs to pty.log)
                                                                                                    ▼
 main thread (SessionRunner): owns its inbox, the TerminalSession (Menu, Keymap, MarkerTracker, KeyDecoder, settings),
     the TerminalScreen (AlacrittyTerminalScreen: Term model, OutputFilter, OverlayRenderer) and the SessionUseCases
 real terminal ◄── stdout (pty bytes minus markers + overlay frames) ── [main]               (TerminalWriter port)
 [main] ──EngineMessage::{Complete, CommandFinished, CommandEnded, RecordInsert, Llm, Settings, Session,
     ToggleFuzzy, Shutdown}──► [engine]       (CompletionEngine port; bootstrap's ChannelCompletionEngine sends them)
 [pty writer] ◄── Vec<u8> (unbounded) ── main                                          (WrappedShell::write_input)
     └─► master writer (blocking writes never stall main)
 [engine] thread (spawned in bootstrap/engine_server.rs): owns the QuickJS runtime (QuickJsSpecRuntime, !Send) and the
     EngineInbox; its EngineServer holds the CompletionSession (generator states/caches, recency) and the use cases over
     the spec cache (QuickJsSpecRepository), the history (HistoryRepository) and the inline cache (GhostTextCache).
     spawns [exec workers] (tabsmith-exec, one per command, timeout), [llm worker] (llm, one per request) and a
     [recency] saver (recency); exec workers report back on the engine's own channel (EngineMessage::Runtime).
```

Where the threads start (paths are relative to `src/modules/`, `bootstrap/` to `src/`): the stdin and signals threads (on Windows, the stdin thread and the resize poll
thread) in `terminal_session/infrastructure/pty/platform_unix.rs` and `platform_windows.rs` (`spawn_input`,
`spawn_resize`); the PTY reader in `pty_reader.rs` (`spawn_pty_reader`); the PTY writer and the child wait in
`session_launcher.rs`. The engine thread starts in `bootstrap/engine_server.rs` (`spawn`); its loop is
`EngineServer::run`, which dispatches each message in `EngineServer::handle`, and which also starts the `llm` worker
(`answer_llm_ghost_text_request`). The `EngineInbox` is `bootstrap/engine_inbox.rs`;
`bootstrap/channel_completion_engine.rs` implements the `CompletionEngine` port by sending to it. The exec workers
start in `specs/infrastructure/adapters/quickjs_spec_runtime.rs`, the recency saver in
`completion/infrastructure/adapters/state_file_recency_repository.rs`. The `self-update` thread starts in
`self_update/infrastructure/cli/background_update.rs` (`start_background_update`), called by `bootstrap/cli.rs` for
`_ pty` before the session runs; until the session's panic hook is set, a panic on it is logged, not printed.

PTY credits: the reader takes one of 4 credits before every read; the main loop returns one per chunk it handled. At
most 4 × 16 KiB are queued, so a flooding child (`yes`) feels backpressure and Ctrl-C is never stuck behind its
output (an unbounded queue grew to GBs). A batch merges every `SessionMessage::Pty` already queued.

Messages (all `std::sync::mpsc`; the comment above each type names its file under `src/modules/`, or `src/` for `bootstrap/`):

```rust
// terminal_session/infrastructure/pty/session_message.rs
enum SessionMessage { Stdin(Vec<u8>), Pty(Vec<u8>), PtyEof, Resize(u16,u16), ChildExit(i32), Term,
                      Completion(Box<CompletionResult>), Ghost{req: u64, text: Option<String>} }
// bootstrap/engine_message.rs (Settings: settings/domain/value_objects/settings.rs,
//   HistoryEntry: history/domain/entities/history_entry.rs, Shell: shared/domain/value_objects/shell.rs)
enum EngineMessage {
  Complete(CompletionRequest), CommandFinished(HistoryEntry), CommandEnded /* NoHistory: reset, nothing recorded */,
  RecordInsert{cmd: String, name: String}, Llm{req: u64, buffer: String, cwd: PathBuf, shell: Shell},
  Settings(Settings), Session{env: HashMap<String,String>, alias: Option<String>} /* from a SessionContext (§2.11) */,
  ToggleFuzzy, Runtime(RuntimeEvent) /* a generator process finished; EngineInbox hands it to the spec runtime */,
  Shutdown }
// specs/application/ports/runtime_event.rs
struct RuntimeEvent { id: TaskId /* u64 */, output: Result<ProcessOutput, String> }
// specs/domain/value_objects/process_output.rs (fields private)
struct ProcessOutput { stdout: String, stderr: String, status: i32 }
// completion/application/completion_request.rs
struct CompletionRequest { id: u64, buffer: String, cursor: usize /*byte*/, cwd: PathBuf, shell: Shell, history_mode: bool }
// completion/domain/value_objects/completion_result.rs (fields private, built from CompletionResultParts)
struct CompletionResult {
  req: u64, phase: CompletionPhase /* Parse | Generators */, error: bool,
  buffer: String, cursor: usize,    // what the reply was computed from (reconciliation, §7.3)
  tokens: Vec<String>, command_text: String /* before alias expansion */, command_index: usize,
  arg_key: u64,                     // hash of the current arg's and subcommand's Rc addresses
  current_arg: Option<ArgumentHint{name, description}>,
  search_term: String, triggered_generators: Vec<GeneratorKey>,
  items: Vec<Suggestion>,           // filtered, ranked, insertion precomputed (§6.8)
  loading: bool, ghost: Option<String> }
```

Main loop (pseudo-code; `SessionRunner::run` and `SessionRunner::step` in
`terminal_session/infrastructure/pty/session_runner.rs`):

```text
loop {
  timeout = min(resize, lock, llm debounce, spinner, file poll (2 s), exit grace, redraw grace, held keys)
  msg = inbox.recv_timeout(timeout)                 // timeout = TerminalSession::next_deadline
  match msg:                                        // Stdin, Pty, Resize, Completion and Ghost run inside `guard`
                                                    // (panic → pass-through); ChildExit, PtyEof and Term do not
    Stdin(b)      → a late startup-probe reply is dropped first (§4.4; its OSC 11 background goes to
                    ApplyTerminalBackgroundUseCase), then HandleKeystrokesUseCase(b)               // §7.2.4
    Pty(b)        → batch = b ++ all queued Pty; ProcessShellOutputUseCase(batch); return one credit per chunk   // §4.5
    Resize(c,r)   → ResizeTerminalUseCase: master.resize; term.resize; filter.resize; renderer.drop_clear();
                    TerminalSession::resize_applied(now) arms the 150 ms quiet spell (ResizeDebounce `settle_at`)
    Completion(c) → ApplyCompletionResultUseCase(c)                                                // §7.1, §8.4
    Ghost{req,t}  → ApplyLlmGhostTextUseCase: if req is the latest request and the LLM request: llm ghost = t; redraw
    ChildExit(c)  → the runner drops the master on a helper thread (Windows only; ClosePseudoConsole may wait for
                    pipe room that only credits from this loop free), then
                    HandleShellExitUseCase(ShellEnd::ChildExited(c)): exit_code = c; leave at PtyEof or after 200 ms
    PtyEof        → HandleShellExitUseCase(ShellEnd::OutputClosed): leave if the child already exited
    Term          → HandleShellExitUseCase(ShellEnd::Terminated): exit_code = 129; leave
    Timeout       → nothing of its own; it only wakes the loop for the tick below
  }
  after every message and on Timeout, unless the loop is left or the session is in pass-through:
    HandleTimerTickUseCase: resize repaint (§7.6), redraw grace, held keys, lock expiry (§7.3), LLM request
                    (§8.4), spinner frame, settings/session poll
}
EndSessionUseCase; exit(exit_code)
```

Panic guard: a panic in any guarded handler (logged by the panic hook, never printed) writes the cached clear and
switches the session to plain pass-through (stdin → PTY, PTY → stdout, no tracker/menu/frames) until the child exits;
the running shell and its jobs are kept. A read whose handler panicked is forwarded raw unless the menu was
intercepting (an Enter meant for an item is dropped rather than running the line); a PTY batch goes out raw.

### 4.4 `_ pty` startup, exit, fallback

0. `bootstrap` starts the background update check on the `self-update` thread (D51) and goes on without waiting.
1. If stdin or stdout is not a tty → fallback.
2. `shell = --shell | $TABSMITH_SHELL | $SHELL (unix) | "pwsh" (windows)`; error if none → fallback.
3. Load settings (errors → defaults). Open `logs/pty.log`.
4. Raw mode: Unix `crossterm::terminal::enable_raw_mode()` (TCSANOW; cfmakeraw ⇒ ISIG off, so Ctrl-C/Z become bytes).
   Windows: stdin mode `(old & !(ENABLE_ECHO_INPUT|ENABLE_LINE_INPUT|ENABLE_PROCESSED_INPUT)) | ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_WINDOW_INPUT`;
   stdout mode `old | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN`; `SetConsoleCP/SetConsoleOutputCP(CP_UTF8)`; all four restored on exit.
5. **Sync model with the real cursor** (ish clears the screen to sync, `ish/src/ui/ui-root.ts:42`): always write
   `ESC ] 11 ; ? ESC \ ESC [ 6 n` (the background is asked even for a non-`system` theme, so switching to `system`
   later works); read stdin (Unix `select`, Windows `WaitForSingleObject`) for up to 300 ms until a CPR arrives,
   collecting replies (`ESC]11;rgb:R/G/B` with 1–4 hex digits per channel + BEL/ST, and `ESC [ row ; col R`). Any
   other bytes read are forwarded to the PTY after spawn. With a CPR: feed the **model only** `ESC [ row ; col H`.
   Without: write `ESC[H ESC[2J` to stdout (model already at 1;1) and, for the next 3 s, strip a late CPR/OSC 11
   reply from stdin (applying its background) instead of typing it into the shell (ponytail: a reply cut by the
   300 ms timeout is forwarded as typeahead). Rows above the cursor are blank in the model (content printed before
   tabsmith started is not restorable if the menu ever covers it — cosmetic).
6. Session id; child env = current env + `TABSMITH_TERM=<ver>`, `TABSMITH_TERM_TMUX=<ver>` iff `TMUX`,
   `TABSMITH_SESSION_ID=<sid>`; remove `TABSMITH_SHELL`, `TABSMITH_IS_LOGIN_SHELL`. `TERM` inherited (not set).
7. argv: args after `--` whose first one does not start with `-` are the command (replaces the shell); else
   `[shell] + (["--login"] if TABSMITH_IS_LOGIN_SHELL=1 && shell ∈ {bash,zsh,fish}) + args after --` (pwsh pre:
   `-- -NoLogo <outer args>`; fish pre: `-- --private`; `debug shell`: its rc flags). cwd = current dir (fallback
   `$HOME`). Open PTY at `crossterm::terminal::size()`.
8. Write `ESC ] 0 ; <shell basename> BEL` (tab title).
9. Spawn threads (§4.3). Main loop.
10. Exit: write the cached clear, `ESC[0m ESC[?25h`, send `EngineMsg::Shutdown`, kill every generator process still
    running (§9.3), restore raw/console modes, delete `run/<sid>.json`, exit with the child's code: Unix `waitpid`,
    so a child killed by signal n gives 128+n (portable-pty would report 1).

**Fallback** (any error or panic before the main loop): restore terminal state, log, print
`tabsmith had an error: <err>` to stderr, kill live generator processes, then run the plain shell (`/bin/sh` / `pwsh`
when none is known) with `TABSMITH_TERM_DISABLED=1` (so its pre block does not loop): Unix `execvp` via
`CommandExt::exec`; Windows spawn with inherited stdio, wait, exit with its code. **Never** leave the user
without a shell. Everything that can fail runs before the child is spawned, so the fallback never orphans a
shell; a panic inside the main loop uses the pass-through guard of §4.3 instead.

Signals (Unix): SIGTERM/SIGHUP → cleanup + exit 129; SIGWINCH → `Msg::Resize`. SIGINT never arrives (raw mode).

### 4.5 Output path (per PTY batch)

```text
on_output(batch):
  if resize pending: re-arm its 150 ms debounce                // the shell / ConPTY is still redrawing
  body = []
  for piece in splitter.feed(batch):           // §5.1.1, stateful across batches
     Bytes(b)   → model.advance(b); filter.feed(b, &mut body)                 // §7.5.4
     Marker(p)  → tracker.on_marker(p, &mut model)                            // §5.1; tags the model only
  if this batch had no marker: apply Windows-deferred markers (§5.1.4)
  for ev in tracker.events: on_event(ev)       // §7.1: NewCmd / CommandEnded / PreExec
  if the alternate screen was just left: menu.error_reset()
  refresh()                                    // §7.3 lock, §5.6 extraction (live UI: at-prompt only), request
  out = renderer.take_clear()                  // taken late: a panic above leaves the clear for the guard
      ++ body ++ pending full repaint (§7.6) ++ renderer.frame(...)          // §7.5
  stdout.write_all(out); flush                  // ONE write per batch
  replay held keys if their reply came (§7.3)
```

All platforms: the output filter holds back an incomplete trailing escape sequence and UTF-8 char (≤ 3 bytes) until
the next batch, so an appended frame never splits one. Windows: bare LF stays LF because `DISABLE_NEWLINE_AUTO_RETURN`
is set; if `SetConsoleMode` fails with that flag, bare `\n` (not preceded by `\r`) is rewritten to `ESC D`
(ish `stdioProxy.ts`).

---

## 5. Shell integration

### 5.1 Protocol: OSC 697

Wire format `ESC ] 6 9 7 ; <payload> (BEL | ESC \)`. Payload = all bytes up to the terminator (no `;` splitting,
no escaping). A payload is either a bare token or `Key=Value` split at the **first** `=`.

`ShellMarker::parse` reads the payload; `MarkerTracker::on_marker` takes the marker. The tracker never touches the
screen: an "inject" below is a `ScreenCommand` that `ShellOutputFeeder` has `TerminalScreen::apply` run on the model,
in order, at the marker's byte offset; an "event" is a `ShellEvent`, handed on after the batch. The tracker only
decides: the screen adapter does the tagging (`AlacrittyTerminalScreen::apply` → `cell_tagger::apply`, and
`mark_line_start` for the line start below).

| payload | lock-guarded | effect in `MarkerTracker` |
|---|---|---|
| `StartPrompt` | yes | inject `ESC]8;;tabsmith:p ESC\` into the model (prompt tag on): `ScreenCommand::Tag(CellTag::Prompt)`. |
| `EndPrompt` | yes; Windows: deferred | If a command cycle is open (NewCmd seen, no PreExec since) inject `ESC]8;;tabsmith:c<n> ESC\` (`ScreenCommand::Tag(CellTag::Cycle(n))`), else inject `ESC]8;; ESC\` (`ScreenCommand::ClearTag`). Deferred (Windows, §5.1.4): queued, not applied; `MarkerTracker::end_batch`, which `ShellOutputFeeder::feed` calls after each batch, applies the queue once a batch brought no marker. |
| `NewCmd=<sid>` | no (sid must equal ours, D35; a bare `NewCmd` or another sid is ignored); Windows: deferred | `redraw = (a cycle is open and not preexec)`; `n += 1`; `preexec = false`; `no_history = false`; inject `ESC]8;;tabsmith:c<n> ESC\` (`ScreenCommand::Tag(CellTag::Cycle(n))`); **mark the line start** (below; `ScreenCommand::MarkLineStart`); `line_col0 = cursor column == 0`. Then finish commands: take the stored `ExitCode`; find the innermost pending entry whose `pid` equals the last `PID=` (D38). None → event `ShellEvent::NewCommand(None)` (a prompt redraw: SIGWINCH, `zle reset-prompt`, a nested shell's first prompt). Else clear `submitted` and pop that entry and every one nested above it, each with `exit_code`, `end_time = now` (`HistoryEntry::finish`), emitting `ShellEvent::NewCommand(Some(entry))`, or `ShellEvent::CommandEnded` for a blank entry (NoHistory, or nothing scraped). |
| `PreExec` | no; ignored while `preexec` | Apply deferred markers first. `scraped` = `EditBuffer::buffer` trimmed (built as in §5.6; empty when no cycle is open, because `MarkerTracker::edit_buffer_context` is then `None`; `on_marker` answers `MarkerReaction::EditBufferNeeded`; `ShellOutputFeeder` reads the buffer with `TerminalScreen::edit_buffer`; `complete_pre_exec` goes on); `command = submitted` (trimmed) when it is non-empty and occurs in `scraped` as a whole run (start of the scrape or after a space) followed only by spaces and then nothing or a newline (D37), else `scraped`; blank when `no_history` (then reset) or when nothing was scraped. Push `{command, shell, pid, session_id, cwd, start_time: now}` (a `HistoryEntry`) onto `pending`; `preexec = true`; `pwsh_list_view = false`; inject `ESC]8;; ESC\` (`ScreenCommand::ClearTag`); event `ShellEvent::PreExec`. |
| `NoHistory` | no; ignored while `preexec` | Apply deferred markers first; `no_history = true`: the next PreExec records a blank command (D36). |
| `OSCLock=<sid>` | – | If sid == ours → `osc_lock = true`. |
| `OSCUnlock=<sid>` | – | If sid == ours → `osc_lock = false`. |
| `Dir=<path>` | yes | `cwd = path` (untrimmed; used for generators; tabsmith does **not** `chdir` itself). |
| `ExitCode=<i32>` | yes | stored; applied to the commands the next `NewCmd` finishes. |
| `Shell=<name>` | yes | `shell` (name and parsed `Shell`). |
| `PID=<i32>` | yes | `pid` (keys the pending stack). |
| `ZshAutosuggestionColor=<zle spec>` | yes | suggestion colour (zsh). |
| `FishSuggestionColor=<fish spec>` | yes | suggestion colour (fish). |
| `PwshPredictionColor=<SGR params>` | yes | PSReadLine `InlinePredictionColor`, ESC stripped (`[97;2;3m`, `38;5;8`): colours plus required DIM/ITALIC (§5.1.3). |
| `PwshPrompts=<cont>\x1f<prompt text>…` | yes | PSReadLine `ContinuationPrompt` and `PromptText` entries (SGR stripped, untrimmed), stripped from the pwsh buffer (§5.6). Default continuation `>> `. |
| `PwshListView=<0\|1>` | yes | PSReadLine ListView predictions active; reset when a command starts. |
| `ShellPath=<path>`, `TTY=<path>`, `User=<name>` | – | read as `ShellMarker::Ignored`: kept nowhere, used by nothing. `Ignored` is not lock-exempt (`ShellMarker::passes_integration_lock`) and a no-op either way, `User` included. The bash, zsh and fish post scripts still send all three; the pwsh one sends only `User`. |
| anything else (`Log=`, `DoneSourcing`, …) | – | ignored. |

Empty values (`Key=`) are ignored. Values other than `NewCmd` (the sid is compared untrimmed, so `NewCmd= <sid>` is
not ours), `Dir` and `PwshPrompts` are trimmed.

Line start at `NewCmd` (the cursor is where the command line begins): on the cursor's row, every cell before the cursor
without a tag gets `tabsmith:p` (a prompt drawn without markers: pwsh re-wraps a foreign prompt at ReadLine, §5.4.5).
On a redraw, cells still tagged with the old cycle `c<o>` are re-tagged: before the cursor → `tabsmith:p` (readline
redraws only a multi-line prompt's last line, without StartPrompt), at/after the cursor on that row and its wrap
continuations → `c<n>` (readline keeps the typed text and moves over it with CUF after a vi-mode switch, or shifts it
with DCH when leaving i-search).

The wrapper's reaction to the events is in §7.1 (`TerminalSession::on_shell_event`, called by
`ProcessShellOutputUseCase`). For each event the use case first forwards `NewCommand(Some(entry))` and `CommandEnded`
to the engine (`CompletionEngine::command_finished` / `command_ended`, i.e. `EngineMessage::CommandFinished` /
`CommandEnded`), then calls `on_shell_event`: `NewCommand(Some(entry))` is how a finished command reaches
`history.jsonl` (§3.5), `CommandEnded` only resets the engine's end-of-command state, nothing recorded (§4.3).

#### 5.1.1 Splitter (stateful, before the VT parser)

States `Ground`, `Esc`, `Osc(buf)`, `OscEsc(buf)`. `ESC ]` starts collecting; BEL or `ESC \` ends; `ESC` + other byte
in `Osc` → abandon (flush collected bytes as `Bytes`) and re-process. A completed OSC whose payload starts with
`697;` → `Piece::Marker(payload[4..])`, consumed (not forwarded, not fed to the model). Any other OSC → `Bytes`
(original bytes). An OSC buffer > 64 KiB is flushed as `Bytes`. Splitting at the exact byte offset matters: the
cursor position at `NewCmd` must be the one at that byte.

States `Name(buf)`, `NameEsc(buf)`: screen's window name `ESC k … ESC \` (oh-my-zsh and others name tmux windows with
it) → `Piece::WindowName(original bytes)`, sent to the terminal but **not** fed to the model: alacritty has no `ESC k`
and prints the name, which drifts the model's cursor right and, once PROMPT_SP pads the line to the margin, a row
below the real one (every frame then moves the cursor there). Ended as tmux ends it: `ESC \` (BEL does not end it),
CAN/SUB cancel it, `ESC` + other byte ends it and is re-processed. Past 64 KiB the collected part is flushed as
`WindowName` and collecting goes on.

#### 5.1.2 Lock

`OSCLock/OSCUnlock` carry our session id so a nested shell (ssh, a manually started `bash`, a nested tabsmith) that
also emits 697 cannot overwrite our state. While locked only `NewCmd`, `PreExec`,
`NoHistory`, `OSCLock` and `OSCUnlock` are processed (`ShellMarker::passes_integration_lock`; `User` is read as
`Ignored` and has no effect, locked or not). The sid gate (with the bare-`NewCmd` rule, D35) stops
untrusted *output* (a file, an ssh banner) from faking prompts; it is not a trust boundary against local programs:
every child (and a plain subshell, which can unlock) inherits `TABSMITH_SESSION_ID` and could forge markers, but such
a program can already write `history.jsonl` or the rc files directly.

#### 5.1.3 Suggestion-colour specs

- zsh (`fg=8`, `fg=#aaaaaa,bg=…`, `fg=245,bold`, `fg=yellow`): comma list; `fg=`/`bg=` value is `0..255`, a name
  (`black red green yellow blue magenta cyan white`, optional `default`), or `#rrggbb`.
- fish (`555 brblack`, `brblack`, `--bold 666`): every non-option token is a colour: names incl. `br*` variants (8–15),
  hex `rgb`/`rrggbb` (optional `#`). fish renders the first one the terminal supports, so **any** listed colour matches.
- Match: convert both the cell fg (Named n→n, Indexed i→i, Spec rgb→nearest xterm-256) and each spec colour to an
  xterm-256 index; equal ⇒ match; a `bg=` in the spec must match the cell bg the same way.
- pwsh: `PwshPredictionColor` parsed as SGR (`0` reset, `2` DIM, `3` ITALIC, `30–37`/`90–97`/`39`, `40–47`/`100–107`/`49`,
  `38/48;5;n`, `38/48;2;r;g;b`): ghost cell ⇔ the colours match **and** the cell has the listed DIM/ITALIC flags.
  Without a report, or one that sets nothing: `DIM || ITALIC` (PSReadLine's default `\e[97;2;3m`, ish
  `commandManager.ts:106-122`). A custom grey without dim/italic was read as typed text before.
- bash: none.

#### 5.1.4 Windows ConPTY deferral

ConPTY may emit the OSC before the prompt text it belongs to. On Windows, `EndPrompt` and `NewCmd` (and their tag
injections) are queued instead of applied. At the end of a batch, if the queue is non-empty and **this batch added no
new 697 markers**, apply the queue in FIFO order with the current cursor.
`PreExec` and `NoHistory` apply the queue first (a late flush must not reopen a cycle while a command runs).

### 5.2 Launch decision (`_ should-launch`, also called in-process by `init … pre`)

First match wins; prints the reason line.

1. `TABSMITH_FORCE_LAUNCH` set → `✅ TABSMITH_FORCE_LAUNCH` → 0
2. `TABSMITH_TERM_DISABLED` → 1
3. `PROCESS_LAUNCHED_BY_TABSMITH` → 1
4. `TABSMITH_TERM` set: `TMUX` set and `TABSMITH_TERM_TMUX` unset → `✅ new tmux pane` → 0; else `❌ already inside tabsmith` → 1
5. `INSIDE_EMACS` → 1; `TERM_PROGRAM=WarpTerminal` → 1; `__CFBundleIdentifier=com.vandyke.SecureCRT` → 1;
   `__PWSH_LOGIN_CHECKED` → 1; `CI` → 1; `VSCODE_RESOLVING_ENVIRONMENT` (VS Code/Cursor env probing,
   `ish/src/utils/shell.ts:330-372`) → 1; Unix `TERM` unset or `dumb` → 1
6. Unix: stdin not a tty → 1 (the scripts check stdout and interactivity themselves)
7. Windows: `GetConsoleMode(stdin)` fails → 1
8. → `✅ launching` → 0

### 5.3 `init` output

Lines joined by `\n`, then the script body (trailing newlines trimmed) with `{{BIN}}` replaced by the shell-quoted
absolute path of `current_exe()`. `quote(shell, s)` (also used for insertions, D26): single quotes; POSIX `'` →
`'\''`; fish `\` → `\\` and `'` → `\'`; pwsh doubles `'` and the smart single quotes U+2018–U+201B (D29).

| shell, when | header lines | body |
|---|---|---|
| bash/zsh pre | `TABSMITH_SHELL='<_ get-shell result>'` (omitted if unknown), `SHOULD_TABSMITH_LAUNCH=<0\|1>` | `shell/pre.sh` |
| bash post | – | `shell/post.bash` |
| zsh post | – | `shell/post.zsh` |
| fish pre | `set -g SHOULD_TABSMITH_LAUNCH <0\|1>` | `shell/pre.fish` |
| fish post | – | `shell/post.fish` |
| pwsh pre | `$TABSMITH_SHOULD_LAUNCH = <0\|1>` | `shell/pre.ps1` |
| pwsh post | – | `shell/post.ps1` |

### 5.4 Scripts (normative: the files in `shell/`, reproduced verbatim)

All `printf` calls pass values as `%s` arguments (a `%` in a path would break a format string built from the value). `{{BIN}}` is replaced as in §5.3. `tests/snapshots/init_<shell>_<when>.snap` hold the rendered `init` output, and
`integrations_init_syntax` checks it with `bash -n` (+ shellcheck), `zsh -n`, `fish --no-execute` and pwsh's parser
where those are installed.

#### 5.4.1 `pre.sh` (bash, zsh)

```sh
# shellcheck shell=bash
# tabsmith pre (bash, zsh): replace this interactive shell with the tabsmith PTY wrapper (SPEC §5.4.1).
if [ "${SHOULD_TABSMITH_LAUNCH:-1}" = 0 ] && [ -t 0 ] && [ -t 1 ] \
   && [ -z "${BASH_EXECUTION_STRING:-}${ZSH_EXECUTION_STRING:-}" ]; then
  case $- in *i*)
    if [ -n "${BASH_VERSION:-}" ] && { [ "${BASH_VERSINFO[0]}" -lt 4 ] || { [ "${BASH_VERSINFO[0]}" -eq 4 ] && [ "${BASH_VERSINFO[1]}" -lt 4 ]; }; }; then
      :   # bash < 4.4 unsupported (no PS0)
    else
      TABSMITH_IS_LOGIN_SHELL=0
      if { [ -n "${BASH_VERSION:-}" ] && shopt -q login_shell; } || [[ -n "${ZSH_NAME:-}" && -o login ]]; then
        TABSMITH_IS_LOGIN_SHELL=1
      fi
      export TABSMITH_SHELL="${TABSMITH_SHELL:-$SHELL}" TABSMITH_IS_LOGIN_SHELL
      # kitty's bash injection hides KITTY_SHELL_INTEGRATION while the rc files run; the post block needs it.
      [ -n "${kitty_bash_inject:-}" ] && export KITTY_SHELL_INTEGRATION="${ksi_val:-}"
      # Ghostty injected its integration (zsh: set from its .zshenv; bash: while it sources the rc files).
      [ -n "${_ghostty_state+x}${__ghostty_bash_flags+x}" ] && export TABSMITH_GHOSTTY_INJECTED=1
      exec -a "${TABSMITH_SHELL##*/} (tabsmith)" {{BIN}} _ pty
    fi ;;
  esac
fi
```

`exec` replaces the shell (same pid, same tty); argv[0] `zsh (tabsmith)` keeps tab titles sensible. The two variables are exported before `exec` (a prefix assignment is not seen by
`${TABSMITH_SHELL##*/}` in the same command). kitty's bash injection hides `KITTY_SHELL_INTEGRATION` while the rc files
run. Ghostty's injection is detected here, where it is still visible (`_ghostty_state` from its zsh `.zshenv`,
`__ghostty_bash_flags` while its bash wrapper sources the rc files): `GHOSTTY_RESOURCES_DIR` alone is also exported
with `shell-integration = none` and inside tmux.

#### 5.4.2 `post.zsh`

```zsh
# tabsmith post (zsh): OSC 697 markers around the prompt + precmd/preexec hooks (SPEC §5.4.2).
if [[ -n "${ZSH_NAME:-}" && -n "${TABSMITH_SESSION_ID:-}" && -z "${_TS_POST_LOADED:-}" ]]; then
  _TS_POST_LOADED=1
  _ts_osc() { builtin printf '\e]697;%s\a' "$1"; }
  # zsh keeps RPROMPT and RPS1 apart until both are set (`RPROMPT=x` leaves RPS1 unset, and `RPS1=""` then clears
  # RPROMPT): wrap and restore whichever name is set, RPROMPT first.
  _ts_restore() {
    if [[ ${_ts_wrapped:-0} == 1 ]]; then
      PS1=$_ts_u1; PS2=$_ts_u2; _ts_wrapped=0
      if (( ${+RPROMPT} )); then RPROMPT=$_ts_ur1; else RPS1=$_ts_ur1; fi
      if (( ${+RPROMPT2} )); then RPROMPT2=$_ts_ur2; else RPS2=$_ts_ur2; fi
    fi
  }
  # Wrap every prompt that is not wrapped yet, keeping its own text for _ts_restore. Idempotent: it also runs after
  # a theme rebuilt PROMPT behind our precmd.
  _ts_wrap() {
    local sp=$'%{\e]697;StartPrompt\a%}' ep=$'%{\e]697;EndPrompt\a%}'
    [[ $PS1 == *"$sp"* ]] || { _ts_u1=$PS1; PS1="$sp$PS1$ep"$'%{\e]697;NewCmd='"$TABSMITH_SESSION_ID"$'\a%}'; }
    [[ $PS2 == *"$sp"* ]] || { _ts_u2=$PS2; PS2="$sp$PS2$ep"; }
    if (( ${+RPROMPT} )); then
      [[ $RPROMPT == *"$sp"* ]] || { _ts_ur1=$RPROMPT; [[ -z $RPROMPT ]] || RPROMPT="$sp$RPROMPT$ep"; }
    elif [[ $RPS1 != *"$sp"* ]]; then
      _ts_ur1=$RPS1; [[ -z $RPS1 ]] || RPS1="$sp$RPS1$ep"
    fi
    if (( ${+RPROMPT2} )); then
      [[ $RPROMPT2 == *"$sp"* ]] || { _ts_ur2=$RPROMPT2; [[ -z $RPROMPT2 ]] || RPROMPT2="$sp$RPROMPT2$ep"; }
    elif [[ $RPS2 != *"$sp"* ]]; then
      _ts_ur2=$RPS2; [[ -z $RPS2 ]] || RPS2="$sp$RPS2$ep"
    fi
    _ts_wrapped=1
  }
  _ts_preexec() {
    _ts_restore; _ts_osc "OSCLock=$TABSMITH_SESSION_ID"
    [[ -o histignorespace && $1 == ' '* ]] && _ts_osc NoHistory   # kept out of zsh's history: keep it out of ours
    _ts_osc PreExec
  }
  _ts_precmd() {
    local st=$?
    _ts_restore                                   # Ctrl-C at the prompt: preexec never ran
    _ts_osc "OSCUnlock=$TABSMITH_SESSION_ID"; _ts_osc "Dir=$PWD"; _ts_osc "Shell=zsh"
    _ts_osc "ShellPath=${TABSMITH_SHELL:-$SHELL}"; _ts_osc "PID=$$"; _ts_osc "ExitCode=$st"
    _ts_osc "TTY=$TTY"; _ts_osc "User=${USER:-root}"
    (( ${+functions[_zsh_autosuggest_accept]} )) && _ts_osc "ZshAutosuggestionColor=${ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE:-fg=8}"
    _ts_wrap
    # powerlevel10k's precmd re-asserts itself last and rebuilds PROMPT, also on async re-renders: wrap each build.
    # Checked at every prompt, so a p10k loaded or reloaded later is covered too.
    if (( ${+functions[_p9k_set_prompt]} )) && [[ $functions[_p9k_set_prompt] != *_ts_wrap* ]]; then
      functions[_ts_p9k_set_prompt]=$functions[_p9k_set_prompt]
      _p9k_set_prompt() { _ts_p9k_set_prompt "$@"; local r=$?; [[ $1 == instant_ ]] || _ts_wrap; return $r; }
    fi
    _ts_reset_hooks
    ( {{BIN}} _ pre-cmd --alias "$(builtin alias)" >/dev/null 2>&1 & )
  }
  _ts_reset_hooks() {   # precmd LAST (wraps the final PS1), preexec FIRST; re-asserted every prompt
    [[ ${precmd_functions[-1]} == _ts_precmd ]] || precmd_functions=(${precmd_functions:#_ts_precmd} _ts_precmd)
    [[ ${preexec_functions[1]} == _ts_preexec ]] || preexec_functions=(_ts_preexec ${preexec_functions:#_ts_preexec})
  }
  # Themes that replace PS1 after our precmd, or mid-edit (oh-my-posh streaming, pure: `zle -F` handlers, then
  # `zle reset-prompt`): wrap it before the line is drawn.
  _ts_line_init() { [[ $PS1 == *$'%{\e]697;StartPrompt\a%}'* ]] || { _ts_wrap; zle .reset-prompt; } }
  # zsh/zle may not be loaded yet in a .zshrc: add-zle-hook-widget would then do nothing.
  [[ -o zle ]] && builtin zmodload zsh/zle && builtin autoload -Uz add-zle-hook-widget &&
    add-zle-hook-widget line-init _ts_line_init && add-zle-hook-widget line-pre-redraw _ts_line_init
  # The integration Ghostty, kitty and VS Code inject loads around the rc files of the shell they start, which the
  # pre block replaced with the wrapper: load it again in this one. Ghostty exports GHOSTTY_RESOURCES_DIR also
  # with `shell-integration = none` and into tmux: the pre block saw the injection.
  # ponytail: `code --locate-shell-integration-path` runs the VS Code CLI at every start; cache it if that is slow.
  [[ $TABSMITH_GHOSTTY_INJECTED == 1 && -r $GHOSTTY_RESOURCES_DIR/shell-integration/zsh/ghostty-integration ]] &&
    builtin source -- "$GHOSTTY_RESOURCES_DIR/shell-integration/zsh/ghostty-integration"
  unset TABSMITH_GHOSTTY_INJECTED   # not for shells started from this one
  if [[ -n $KITTY_SHELL_INTEGRATION && -r $KITTY_INSTALLATION_DIR/shell-integration/zsh/kitty-integration ]]; then
    builtin autoload -Uz -- "$KITTY_INSTALLATION_DIR/shell-integration/zsh/kitty-integration"
    kitty-integration; builtin unfunction kitty-integration
  fi
  # VSCODE_INJECTION unset: the script must not source this .zshrc again.
  [[ $VSCODE_INJECTION == 1 && -z $VSCODE_SHELL_INTEGRATION ]] && (( ${+commands[code]} )) &&
    VSCODE_INJECTION= builtin source -- "$(code --locate-shell-integration-path zsh)"
  _ts_reset_hooks
fi
```

- Markers sit inside `%{…%}` (zero width). `_ts_wrap` is idempotent (it looks for StartPrompt) and saves the theme's
  own text for `_ts_restore`, so it may run several times per prompt: from `_ts_precmd` (the last precmd hook,
  re-asserted every prompt), after powerlevel10k's `_p9k_set_prompt` (wrapped once p10k is seen; not for its
  `instant_` render), and from the zle `line-init` / `line-pre-redraw` hooks, which re-wrap and redraw a PS1 a theme
  replaced after precmd or mid-edit (oh-my-posh streaming, pure: `zle -F` handlers + `reset-prompt`) (D48).
- RPROMPT and RPS1 stay separate until both are set (`RPROMPT=x` leaves RPS1 unset, `RPS1=""` then clears RPROMPT):
  whichever is set is wrapped and restored.
- `NoHistory` when `histignorespace` keeps a space-prefixed line out of zsh's history (D36).
- `PROMPT`/`prompt` are aliases of `PS1`; PS3/PS4 are not wrapped (wrapping PS4 would pollute `set -x`).
- Terminal-injected integrations are loaded again (D46): Ghostty only when `pre.sh` saw it injected, kitty when
  `KITTY_SHELL_INTEGRATION` is set, VS Code when `VSCODE_INJECTION=1` (emptied for the call, so its script does not
  source this `.zshrc` again). `_ts_reset_hooks` runs after them, so tabsmith's hooks stay outermost.

#### 5.4.3 `post.bash` (bash ≥ 4.4)

```bash
# shellcheck shell=bash
# tabsmith post (bash >= 4.4): OSC 697 markers via PS0 (preexec) and PROMPT_COMMAND (precmd) (SPEC §5.4.3).
if [ -n "${BASH_VERSION:-}" ] && [ -n "${TABSMITH_SESSION_ID:-}" ] && [ -z "${_TS_POST_LOADED:-}" ]; then
  _TS_POST_LOADED=1
  _ts_osc() { builtin printf '\e]697;%s\a' "$1"; }
  : "${TTY:=$(tty 2>/dev/null)}"
  _ts_first() {
    _ts_st=$?
    _ts_osc "OSCUnlock=$TABSMITH_SESSION_ID"; _ts_osc "Dir=$PWD"; _ts_osc "Shell=bash"
    _ts_osc "ShellPath=${TABSMITH_SHELL:-$BASH}"; _ts_osc "PID=$$"; _ts_osc "ExitCode=$_ts_st"
    _ts_osc "TTY=$TTY"; _ts_osc "User=${USER:-root}"
    # VS Code's own prompt command (see _ts_install) wraps the prompt around ours: take its wrap off, it wraps again.
    if [ -n "${_ts_vsc:-}" ]; then
      # shellcheck disable=SC2154  # VS Code's variables
      [[ -n ${__vsc_custom_PS1:-} && $PS1 == "$__vsc_custom_PS1" ]] && PS1=$__vsc_original_PS1
      # shellcheck disable=SC2154
      [[ -n ${__vsc_custom_PS2:-} && $PS2 == "$__vsc_custom_PS2" ]] && PS2=$__vsc_original_PS2
    fi
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
    # PS0 sends NoHistory when the next line does not reach bash's history (its number stays below this one).
    # Taken last, after a user `history -n`. ponytail: PS0 cannot see the line, so under ignoreboth a repeat of
    # the previous command is skipped too.
    case :${HISTCONTROL:-}: in *:ignorespace:* | *:ignoreboth:*) _ts_hn=$HISTCMD ;; *) _ts_hn=0 ;; esac
    # erasedups drops the older copy of a re-run line before adding it, so the number stays put: compare the newest
    # entry (with its time stamp) instead. Only erasedups users pay for the forks.
    _ts_h1=''
    [[ $_ts_hn != 0 && :$HISTCONTROL: == *:erasedups:* ]] && _ts_hn=0 _ts_h1=x$(HISTTIMEFORMAT='%s ' builtin history 1)
    [ "${_ts_wrapped:-0}" = 1 ] && [ "$PS1" = "$_ts_last1" ] && return $st   # already wrapped this prompt
    _ts_u1=$PS1; _ts_u2=$PS2
    PS1="\[\e]697;StartPrompt\a\]$PS1\[\e]697;EndPrompt\a\e]697;NewCmd=$TABSMITH_SESSION_ID\a\]"
    PS2="\[\e]697;StartPrompt\a\]$PS2\[\e]697;EndPrompt\a\]"
    _ts_last1=$PS1; _ts_last2=$PS2; _ts_wrapped=1
    _ts_install
    return $st
  }
  # $1 without our commands, whatever separates them (';' or newline, e.g. bash-preexec), into _ts_kept. Also without
  # bash-preexec's __bp_interactive_mode (then _ts_im=1), and VS Code's prompt commands when $2 is set.
  _ts_strip() {
    local s=$1 kept='' cmd sep
    while [ -n "$s" ]; do
      cmd=${s%%[;$'\n']*}; s=${s:${#cmd}}; sep=${s:0:1}; s=${s:1}
      case ${cmd//[[:space:]]/} in
        _ts_first | _ts_last) ;;
        __bp_interactive_mode) _ts_im=1 ;;
        __vsc_prompt_cmd | __vsc_prompt_cmd_original) [ -n "${2:-}" ] || kept+=$cmd$sep ;;
        *) kept+=$cmd$sep ;;
      esac
    done
    kept=${kept#"${kept%%[![:space:];]*}"}; _ts_kept=${kept%"${kept##*[![:space:];]}"}   # ';;' is a syntax error
  }
  # PROMPT_COMMAND = _ts_first ; <user commands> ; _ts_last   (string, or array from bash 5.1)
  # A DEBUG-trap preexec arms itself with the last prompt command and takes the next command run as the user's, so
  # an arming command stays after _ts_last: bash-preexec's __bp_interactive_mode, or VS Code's whole prompt command.
  # shellcheck disable=SC2128,SC2178,SC2154  # PROMPT_COMMAND is either a string or an array; VS Code's variables
  _ts_install() {
    _ts_im='' _ts_vsc=''
    local pc=() c IFS=$'\n'
    if [[ -z ${bash_preexec_imported:-} && "${PROMPT_COMMAND[*]}" == *__vsc_prompt_cmd* ]] &&
       declare -F __vsc_prompt_cmd_original >/dev/null; then
      # VS Code's own integration (with bash-preexec it uses its hooks instead) evaluates
      # __vsc_original_prompt_command, wraps PS1 and arms its trap: it stays the only prompt command and runs ours.
      # ponytail: VS Code internals, as of 1.10x.
      _ts_vsc=1
      for c in "${__vsc_original_prompt_command[@]}" "${PROMPT_COMMAND[@]}"; do
        _ts_strip "$c" vsc; [ -n "$_ts_kept" ] && pc+=("$_ts_kept")
      done
      __vsc_original_prompt_command=(_ts_first "${pc[@]}" _ts_last)
      [[ ${#PROMPT_COMMAND[@]} == 1 && $PROMPT_COMMAND == __vsc_prompt_cmd_original ]] ||
        { unset PROMPT_COMMAND; PROMPT_COMMAND=__vsc_prompt_cmd_original; }
      return
    fi
    for c in "${PROMPT_COMMAND[@]}"; do _ts_strip "$c"; [ -n "$_ts_kept" ] && pc+=("$_ts_kept"); done
    if [[ -n ${__bp_install_string:-} && "${PROMPT_COMMAND[*]}" == *"$__bp_install_string"* ]]; then
      # bash-preexec installs itself at the first prompt; up to 0.6 it rewrites only ${PROMPT_COMMAND} (a string, or
      # element 0): one string until then. It arms its preexec there, before our _ts_last: arm it again after.
      unset PROMPT_COMMAND
      PROMPT_COMMAND=$'_ts_first\n'"${pc[*]}"$'\n_ts_last\n__bp_interactive_mode'
    elif [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]] ||
         (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
      # bash 5.1+: as an array, every user command still sees the command's PIPESTATUS (_ts_first clobbers it).
      pc=(_ts_first "${pc[@]}" _ts_last ${_ts_im:+__bp_interactive_mode})
      [[ ${#pc[@]} == "${#PROMPT_COMMAND[@]}" && "${pc[*]}" == "${PROMPT_COMMAND[*]}" ]] || PROMPT_COMMAND=("${pc[@]}")
    else
      # ponytail: bash 4.4/5.0 user commands see the PIPESTATUS of _ts_first.
      local kept="_ts_first;${pc:+$pc;}_ts_last${_ts_im:+;__bp_interactive_mode}"
      [[ $PROMPT_COMMAND == "$kept" ]] || PROMPT_COMMAND=$kept
    fi
  }
  # The integration Ghostty, kitty and VS Code inject loads around the rc files of the shell they start, which the
  # pre block replaced with the wrapper: load it again in this one (VSCODE_INJECTION unset: no second ~/.bashrc).
  # Ghostty exports GHOSTTY_RESOURCES_DIR also with `shell-integration = none` and into tmux: the pre block saw
  # the injection. ponytail: `code --locate-shell-integration-path` runs the VS Code CLI at every start; cache it
  # if that is slow.
  if [ "${TABSMITH_GHOSTTY_INJECTED:-}" = 1 ] && [ -r "${GHOSTTY_RESOURCES_DIR:-}/shell-integration/bash/ghostty.bash" ] &&
     ! declare -F __ghostty_precmd >/dev/null; then
    builtin source "$GHOSTTY_RESOURCES_DIR/shell-integration/bash/ghostty.bash"
  fi
  unset TABSMITH_GHOSTTY_INJECTED   # not for shells started from this one
  if [ -n "${KITTY_SHELL_INTEGRATION:-}" ] && [ -r "${KITTY_INSTALLATION_DIR:-}/shell-integration/bash/kitty.bash" ]; then
    builtin source "$KITTY_INSTALLATION_DIR/shell-integration/bash/kitty.bash"
  fi
  if [ "${VSCODE_INJECTION:-}" = 1 ] && [ -z "${VSCODE_SHELL_INTEGRATION:-}" ] && command -v code >/dev/null; then
    VSCODE_INJECTION='' builtin source "$(code --locate-shell-integration-path bash)"
  fi
  # bash 4.4+ prints PS0 before running a command. In PS0 `\!` is the newest history number: below _ts_hn when
  # the line was not added. The expansion needs promptvars (the default).
  _ts_nh=('' $'\e]697;NoHistory\a') _ts_hn=0 _ts_ps0='' _ts_h1=''
  _ts_nhck() { [[ x$(HISTTIMEFORMAT='%s ' builtin history 1) == "$_ts_h1" ]] && _ts_osc NoHistory; }
  # shellcheck disable=SC2016  # expanded by PS0 at each command, not here
  shopt -q promptvars && _ts_ps0='${_ts_nh[\! < _ts_hn]}${_ts_h1:+$(_ts_nhck)}'
  PS0="\e]697;OSCLock=$TABSMITH_SESSION_ID\a$_ts_ps0\e]697;PreExec\a${PS0:-}"
  unset _ts_ps0
  _ts_install
fi
```

- PS0 (printed by bash 4.4+ before a command runs) sends `OSCLock` + `PreExec`. `NoHistory` (D36): under
  `ignorespace`/`ignoreboth`, `_ts_last` stores `$HISTCMD`, and PS0's `${_ts_nh[\! < _ts_hn]}` (needs `promptvars`)
  expands to the marker when the line did not reach bash's history. Under `erasedups` that number does not move for a
  re-run line, so the newest entry (with its time stamp) is compared instead (`_ts_nhck`; the fork is paid only by
  erasedups users). ponytail: under `ignoreboth` an immediate repeat of the previous command is skipped (as bash does);
  `set +o history` is not detected.
- `_ts_first` hands `$?` on to the user's prompt commands; `_ts_last` wraps PS1/PS2 once per prompt (a PS1 changed
  meanwhile is re-wrapped, an already wrapped one is not: no nesting when `_ts_last` runs twice).
- `_ts_install` re-asserts the layout at every prompt (D47): our commands are stripped from every element whatever
  separates them (`;` or a newline, as bash-preexec writes); bash-preexec's `__bp_interactive_mode` stays after
  `_ts_last` (its DEBUG trap takes the next command run as the user's); while bash-preexec has not installed itself
  (its install string is still in PROMPT_COMMAND) everything stays one string (≤ 0.6 rewrites only element 0);
  bash ≥ 5.1 gets the array form, where every user command sees the command's `PIPESTATUS` (ponytail: on 4.4/5.0 they
  see `_ts_first`'s). Inside VS Code's own integration (without bash-preexec), `__vsc_prompt_cmd_original` stays the
  only prompt command and runs ours from `__vsc_original_prompt_command`; `_ts_first` takes VS Code's PS1 wrap off,
  VS Code wraps ours again.
- Terminal-injected integrations as in zsh (D46).

#### 5.4.4 `pre.fish` / `post.fish` (installed as whole files in `conf.d`, §5.5)

```fish
# tabsmith pre (fish): replace this interactive shell with the tabsmith PTY wrapper (SPEC §5.4.4).
if test "$SHOULD_TABSMITH_LAUNCH" = 0; and status is-interactive; and test -t 0; and test -t 1
    set -l login 0; status is-login; and set login 1
    set -gx TABSMITH_SHELL (status fish-path)
    set -gx TABSMITH_IS_LOGIN_SHELL $login
    # fish's exec has no -a; argv0 is cosmetic. `fish --private` stays private inside the wrapper.
    # ponytail: only --private is forwarded; -C/--init-command, --features and -d are not.
    exec {{BIN}} _ pty -- (test -n "$fish_private_mode"; and echo --private)
end
```

```fish
# tabsmith post (fish): OSC 697 markers around the prompt functions + preexec/prompt events (SPEC §5.4.4).
if set -q TABSMITH_SESSION_ID; and not set -q _TS_POST_LOADED
    set -g _TS_POST_LOADED 1
    set -q TTY; or set -g TTY (command tty 2>/dev/null)

    function _ts_osc
        builtin printf '\e]697;%s\a' $argv[1]
    end

    function _ts_copy_fn
        functions --erase $argv[2]
        functions --copy $argv[1] $argv[2]
    end

    function _ts_wrap_prompt
        set -l st $status
        _ts_osc StartPrompt
        builtin printf '%s' (string join \n -- $argv | string collect) # %s: the prompt's backslashes stay literal
        _ts_osc EndPrompt
        return $st
    end

    # Put the user's prompt functions back (before running a command, or before re-wrapping). The copies go too,
    # so a prompt the user erases meanwhile stays erased.
    function _ts_restore
        test "$_ts_wrapped" = 1; or return
        functions -q _ts_user_mode_prompt; and _ts_copy_fn _ts_user_mode_prompt fish_mode_prompt
        functions -q _ts_user_right_prompt; and _ts_copy_fn _ts_user_right_prompt fish_right_prompt
        _ts_copy_fn _ts_user_prompt fish_prompt
        functions -e _ts_user_mode_prompt _ts_user_right_prompt
        set -g _ts_wrapped 0
    end

    function _ts_preexec --on-event fish_preexec
        _ts_osc "OSCLock=$TABSMITH_SESSION_ID"
        # Kept out of fish's history (leading space, private mode): keep it out of ours.
        if string match -q -- ' *' $argv[1]; or test -n "$fish_private_mode"
            _ts_osc NoHistory
        end
        _ts_osc PreExec
        _ts_restore
    end

    function _ts_precmd --on-event fish_prompt
        set -l st $status
        _ts_restore # Ctrl-C or an empty line: fish_preexec never ran
        _ts_osc "OSCUnlock=$TABSMITH_SESSION_ID"
        _ts_osc "Dir=$PWD"
        _ts_osc Shell=fish
        _ts_osc "ShellPath="(status fish-path)
        _ts_osc "PID=$fish_pid"
        _ts_osc "ExitCode=$st"
        _ts_osc "TTY=$TTY"
        if test -n "$USER"
            _ts_osc "User=$USER"
        else
            _ts_osc User=root
        end
        _ts_osc "FishSuggestionColor=$fish_color_autosuggestion"

        # $argv reaches the user's functions: fish 4's transient prompt passes --final-rendering.
        if functions -q fish_mode_prompt
            _ts_copy_fn fish_mode_prompt _ts_user_mode_prompt
            function fish_mode_prompt
                _ts_wrap_prompt (_ts_user_mode_prompt $argv)
            end
        end
        if functions -q fish_right_prompt
            _ts_copy_fn fish_right_prompt _ts_user_right_prompt
            function fish_right_prompt
                _ts_wrap_prompt (_ts_user_right_prompt $argv)
            end
        end
        _ts_copy_fn fish_prompt _ts_user_prompt
        function fish_prompt
            _ts_wrap_prompt (_ts_user_prompt $argv)
            _ts_osc "NewCmd=$TABSMITH_SESSION_ID"
        end
        set -g _ts_wrapped 1

        # Backgrounded inside a throwaway sh: a fish `&` job would overwrite the user's $last_pid.
        command sh -c '"$0" _ pre-cmd --alias "$1" >/dev/null 2>&1 &' {{BIN}} (alias | string collect)
    end
end
```

- `test -t 0` holds because the `conf.d` file `eval`s the init output instead of piping it into `source` (§5.5).
- Wrapping happens on every `fish_prompt` event, so a `fish_prompt` defined later in `config.fish` is still captured.
  D49: `%s` instead of `%b` (the prompt's backslashes stay literal), `$argv` forwarded (fish 4's transient prompt
  passes `--final-rendering`), the copies erased on restore (an erased right/mode prompt stays erased), `--private`
  forwarded to the wrapped fish (ponytail: `-C`, `--features`, `-d` are not), `_ pre-cmd` backgrounded inside `sh -c`
  (a fish `&` job would overwrite `$last_pid`).
- `NoHistory` for a space-prefixed line or in private mode (D36). `ShellPath` comes from `status fish-path`
  (`TABSMITH_SHELL` is removed from the inner shell's env).

#### 5.4.5 `pre.ps1` / `post.ps1` (pwsh)

`pre.ps1` (after the `$TABSMITH_SHOULD_LAUNCH = n` header):

```powershell
# tabsmith pre (pwsh): run the PTY wrapper as a child, then end this shell with its exit code (SPEC §5.4.5).
# Launch only when every argument is an interactive switch (pwsh's shortest unambiguous prefixes): a script path,
# -Command, -File, -EncodedCommand, -CommandWithArgs or -NonInteractive must run in this shell, untouched.
$__tsIs = { param([string[]]$names) [bool]($names | Where-Object {   # 'name:shortest prefix'
  $n, $min = $_ -split ':'; $__tsA.Length -ge $min.Length -and $n.StartsWith($__tsA, 'OrdinalIgnoreCase') }) }
$__tsInteractive = $true
$__tsArgs = @([Environment]::GetCommandLineArgs() | Select-Object -Skip 1)
for ($__tsI = 0; $__tsInteractive -and $__tsI -lt $__tsArgs.Count; $__tsI++) {
  $__tsA = $__tsArgs[$__tsI] -replace '^(--?|/)', ''
  if (& $__tsIs 'workingdirectory:wo', 'wd:wd', 'executionpolicy:ex', 'ep:ep', 'windowstyle:w') { $__tsI++ }
  elseif (-not (& $__tsIs 'nologo:nol', 'noexit:noe', 'noprofile:nop', 'noprofileloadtime:noprofileloadtime',
                          'login:l', 'interactive:i', 'sta:sta', 'mta:mta')) { $__tsInteractive = $false }
}
if ($TABSMITH_SHOULD_LAUNCH -eq 0 -and $__tsInteractive -and [Environment]::UserInteractive -and
    -not [Console]::IsInputRedirected -and -not [Console]::IsOutputRedirected) {
  $env:TABSMITH_SHELL = (Get-Process -Id $PID).Path
  $global:LASTEXITCODE = $null
  # The inner pwsh gets this shell's switches (-ExecutionPolicy, -wd, ...) and no second banner. A leading `-`
  # makes `_ pty` treat them as shell arguments. ponytail: AllHosts profiles (profile.ps1) already ran in this
  # shell and run again in the inner one; the pre block would have to live in $PROFILE.CurrentUserAllHosts.
  & {{BIN}} _ pty --shell $env:TABSMITH_SHELL -- -NoLogo @__tsArgs
  # The outer shell ends with the wrapped one; if the wrapper could not start at all, this shell carries on.
  if ($null -ne $global:LASTEXITCODE) { [Environment]::Exit($global:LASTEXITCODE) }
}
```

- Launches only when every argument is an interactive switch (shortest unambiguous prefixes, after `-`, `--` or `/`):
  a script path, `-Command`, `-File`, `-EncodedCommand`, `-CommandWithArgs` or `-NonInteractive` run in the outer
  shell untouched (a `-c|-command|-f|-file` regex hijacked `pwsh script.ps1` and `-EncodedCommand`).
- The wrapper runs as a child with the outer switches after `-- -NoLogo` (no second banner; `-ExecutionPolicy`, `-wd`,
  `-Login` reach the inner shell). The outer shell exits with the inner one's code; when `_ pty` could not run at all
  (`$LASTEXITCODE` stays null) the outer shell carries on (D1).

`post.ps1`:

```powershell
# tabsmith post (pwsh): OSC 697 markers from wrapped prompt and PSConsoleHostReadLine functions (SPEC §5.4.5).
if ($env:TABSMITH_SESSION_ID) {
  function Global:__TsOsc([string]$p) { "$([char]27)]697;$p$([char]7)" }
  # The markers of a prompt around its text $p: also with no text, for a prompt drawn without ours.
  function Global:__TsMarks($exit, [string]$p) {
    $sid = $env:TABSMITH_SESSION_ID
    $o = (__TsOsc "OSCUnlock=$sid") + (__TsOsc "ExitCode=$exit") + (__TsOsc 'Shell=pwsh') +
         (__TsOsc "PID=$PID") + (__TsOsc "User=$([Environment]::UserName)")
    if ($PWD.Provider.Name -eq 'FileSystem') { $o += __TsOsc "Dir=$($PWD.ProviderPath)" }
    $rl = if (Get-Module PSReadLine) { Get-PSReadLineOption }
    # PSReadLine draws its ListView prediction list below the prompt: tabsmith keeps out of the way.
    $lv = [int]($rl -and $rl.PredictionViewStyle -eq 'ListView' -and $rl.PredictionSource -ne 'None')
    # Prediction colour and continuation/prompt texts, so tabsmith tells typed text from PSReadLine's own drawing.
    if ($rl) {
      $o += (__TsOsc ('PwshPredictionColor=' + ($rl.InlinePredictionColor -replace [char]27, ''))) +
            (__TsOsc ('PwshPrompts=' + ($rl.ContinuationPrompt -replace "$([char]27)\[[0-9;]*m", '') + [char]31 +
                      (($rl.PromptText -replace "$([char]27)\[[0-9;]*m", '') -join [char]31)))
    }
    # After NewCmd, which resets the flag.
    $o + $p + (__TsOsc "NewCmd=$sid") + (__TsOsc "PwshListView=$lv")
  }
  # Wrap `prompt` unless it is ours already; again whenever it is redefined (`. $PROFILE`, `oh-my-posh init pwsh |
  # iex`, `function prompt {…}`). $true when it wrapped. Our functions run with StrictMode off, as PSReadLine's do.
  function Global:__TsWrap {
    Set-StrictMode -Off
    if ("$function:prompt" -eq "$Global:__TsPrompt") { return $false }
    $Global:__TsUserPrompt = $function:prompt
    function Global:prompt {
      $ok = $global:?; Set-StrictMode -Off; $code = $global:LASTEXITCODE
      # $LASTEXITCODE counts only for a native failure: no new error record, or the one native commands add with
      # $PSNativeCommandUseErrorActionPreference. Otherwise it is stale and a cmdlet failed: 1.
      $native = [object]::ReferenceEquals($Error[0], $Global:__TsErr0) -or $Error[0].FullyQualifiedErrorId -eq 'ProgramExitedWithNonZeroCode'
      $exit = if ($ok) { 0 } elseif ($code -and $native) { $code } else { 1 }
      if (-not $ok) { Write-Error '' -ErrorAction Ignore }   # the user's prompt sees the command's $? (no $Error entry)
      $user = & $Global:__TsUserPrompt
      $global:LASTEXITCODE = $code
      return __TsMarks $exit ((__TsOsc 'StartPrompt') + $user + (__TsOsc 'EndPrompt'))
    }
    $Global:__TsPrompt = $function:prompt
    $true
  }
  $null = __TsWrap
  # PreExec once per accepted line (duplicates and history-filtered lines included): wrap PSReadLine's ReadLine.
  # Ctrl-C and blank lines run nothing and get no PreExec, as in the other shells.
  if (-not (Test-Path Variable:\__TsLoaded) -and (Test-Path Function:\PSConsoleHostReadLine)) {
    $Global:__TsLoaded = $true
    $Global:__TsReadLine = $function:PSConsoleHostReadLine
    function Global:PSConsoleHostReadLine {
      $ok = $?   # first statement: PSReadLine's own wrapper reads the last command's status the same way
      Set-StrictMode -Off
      $sid = $env:TABSMITH_SESSION_ID
      # A foreign prompt was just drawn without markers: re-wrap it and track this line from here (the session's
      # shell, pid and dir included, as at any prompt).
      if (__TsWrap) { [Console]::Write((__TsMarks ([int](-not $ok)) '')) }
      if (-not $ok) { Write-Error '' -ErrorAction Ignore }
      $line = & $Global:__TsReadLine
      if (-not [string]::IsNullOrWhiteSpace($line)) {
        # A line PSReadLine keeps out of its history file (sensitive, or the user's handler) stays out of ours.
        # ponytail: the handler runs a second time here; a handler with side effects sees each line twice.
        $h = if (Get-Module PSReadLine) { (Get-PSReadLineOption).AddToHistoryHandler }
        $nh = try { if ($h -and "$($h.Invoke($line))" -in 'False', 'SkipAdding', 'MemoryOnly') { __TsOsc 'NoHistory' } } catch {}   # never lose the line
        $Global:__TsErr0 = $Error[0]
        [Console]::Write((__TsOsc "OSCLock=$sid") + $nh + (__TsOsc 'PreExec'))
      }
      $line
    }
  }
}
```

- `__TsWrap` wraps `prompt` again whenever it was redefined (`. $PROFILE`, `oh-my-posh init pwsh | iex`,
  `function prompt {…}` typed, or defined after the post block). The `PSConsoleHostReadLine` wrapper re-checks before
  each line; when it re-wraps, it writes the prompt's markers without text (`OSCUnlock`, `ExitCode`, `Shell`, `PID`,
  `Dir`, …, `NewCmd`), so the foreign prompt already on screen is tracked (its cells become prompt at `NewCmd`, §5.1).
- Exit code: 0 on `$?`; on failure `$LASTEXITCODE` only when it belongs to this command (no new error record since
  PreExec, or the record a native command adds under `$PSNativeCommandUseErrorActionPreference`), else 1.
  `Write-Error '' -ErrorAction Ignore` puts the command's `$?` back for the user's prompt and for PSReadLine without
  adding to `$Error`.
- PreExec comes from the `PSConsoleHostReadLine` wrapper, once per accepted non-blank line (duplicates and
  history-filtered lines included); Ctrl-C and blank lines run nothing and get none, as in the other shells.
  `AddToHistoryHandler` is never replaced (chaining it with `&` threw on PSReadLine ≥ 2.2 and bypassed its
  sensitive-line filter; PSReadLine skips it for a repeated line). `NoHistory` when that handler answers `False`,
  `SkipAdding` or `MemoryOnly` (ponytail: the handler then runs twice per line).
- Every prompt reports `PwshPredictionColor`, `PwshPrompts` and `PwshListView` (§5.1, §5.6, §7.5.1).
- pwsh does not run `_ pre-cmd` (a process spawn per prompt is slow on Windows): generators use the wrapper's env plus
  `Dir=`, and doctor skips its Session check.

### 5.5 Dotfile installation

Integrations per shell:

| shell | config dir | integration objects |
|---|---|---|
| bash | `$HOME` | dotfile `.bashrc` always + each of `.profile`, `.bash_login`, `.bash_profile` that exists; if none exist, `.profile` (created) |
| zsh | `$ZDOTDIR` or `$HOME` | dotfiles `.zshrc`, `.zprofile` |
| fish | `$XDG_CONFIG_HOME/fish` or `~/.config/fish` | whole files `conf.d/00_tabsmith_pre.fish`, `conf.d/99_tabsmith_post.fish` |
| pwsh | `$PROFILE` (CurrentUserCurrentHost) from `pwsh -NoLogo -NoProfile -Command <write $PROFILE as UTF-8 bytes to the raw stdout stream>` (5 s timeout; Console.Out would use the OEM code page, and changing that would outlive the child) | dotfile `<profile path>` |

Script files `~/.tabsmith/shell/<dotfile name without leading dot and extension>.<pre|post>.<shell>`
(`bashrc.pre.bash`, `zprofile.post.zsh`, `profile.pre.ps1`, …). Their contents (exact bytes, used by `is_installed`;
`'<BIN>'` = `quote(shell, exe)`):

```text
bash:  [ -n "$BASH_VERSION" ] && [ -x '<BIN>' ] && eval "$('<BIN>' init bash <when> --rcfile <rc>)"\n
zsh:   [ -x '<BIN>' ] && eval "$('<BIN>' init zsh <when> --rcfile <rc>)"\n
pwsh:  if (Test-Path -LiteralPath '<BIN>') { $__tsEnc = [Console]::OutputEncoding; $__tsInit = $null; try { try {
         [Console]::OutputEncoding = [Text.UTF8Encoding]::new() } catch {}; $__tsInit = (& '<BIN>' init pwsh <when>
         --rcfile profile) -join "`n" } finally { try { [Console]::OutputEncoding = $__tsEnc } catch {} };
         if ($__tsInit) { Invoke-Expression $__tsInit } }\n                                   (one line)
fish conf.d file:  test -x '<BIN>'; and eval ('<BIN>' init fish <when> --rcfile <file stem> | string split0)\n
                   (stems: 00_tabsmith_pre, 99_tabsmith_post)
```

`<BIN>` is the absolute exe path at install time, so the rc works before the user's PATH is set up (rustup/brew
PATH lines usually come later in the rc). `doctor` detects a moved binary (script bytes differ) and reinstalls.
pwsh decodes native output with `[Console]::OutputEncoding` (the OEM code page on Windows): the init output is read
as UTF-8 and the user's encoding put back before it runs (a `José` in the exe path broke `& '<BIN>' _ pty`); without
a console both setters throw, which is ignored, and an empty init output runs nothing. fish: `eval (… | string
split0)`, never `| source`, which hands the pipe to the script as stdin, so `pre.fish`'s `test -t 0` failed and the
wrapper never started.

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
  whole file (fish conf.d): write it; return
  if dotfile exists: copy to ~/.tabsmith/backups/<unix-secs>/<file name>; uninstall(); contents = read  else contents = ""
  write both script files
  (bom, body) = split_bom(contents)                      // a UTF-8 BOM (pwsh profiles saved by editors) stays first
  (shebang, rest) = split_shebang(body)                  // first line kept on top iff it starts with "#!"
  new = bom + shebang + PRE_MARKER + "\n" + PRE_SOURCE + "\n" + rest
  new = new.trim_end_matches('\n') + "\n" + POST_MARKER + "\n" + POST_SOURCE + "\n"
  if new != contents: write(dotfile, new)

write(path, contents):                                   // D43; used for every dotfile and script file
  target = canonicalize(path), else a dangling symlink's target (relative to its dir), else path
  create target's dir; if target exists: open it for writing first (PermissionDenied → "Permission denied: <path>")
  write <target>.tabsmith-tmp-<pid>, copy the old permissions, rename over target (remove the temp file on error)
```

ponytail: a hard-linked dotfile is split by the rename. Concurrent installs no longer tear lines but can still
duplicate blocks (no lock).

`is_installed()` (first failing reason; dotfile text read without its BOM):
1. dotfile missing → `File does not exist: <path>`; fish `conf.d` file: exact bytes, else `<p> should contain:\n<contents>`.
2. any line matching `^\s*#\s*tabsmith ignore` (case-insensitive) → Ok (user opt-out).
3. `lines` = non-blank lines not starting with `#` (after trim).
4. `lines.first() != PRE_SOURCE` → `Improper integration installation: <dotfile> does not source pre integration first`
   when the line is elsewhere, else `Integration not installed: <dotfile> does not source pre integration`; same for
   post with `lines.last()` (`… last`).
5. each script file must exist with exact expected bytes → `File does not exist: <p>` / `<p> should contain:\n<contents>`.

Uninstall: remove every line equal to `PRE_MARKER`, `PRE_SOURCE`, `POST_MARKER`, `POST_SOURCE` (exact string compare
after `trim_end`) from the text after the BOM; then BOM + `trim()` + `"\n"`; write if changed; delete both script files
(missing = ok; kept when the dotfile write fails). No regexes (a pattern such as `^#.*fig.*var.*` would delete unrelated comments).
fish/whole-file integrations: delete the files. Which integrations `integrations uninstall`
touches: §2.6 (D44); `uninstall` touches all of them.

Error display strings: `Improper integration installation: <msg>`, `Integration not installed: <msg>`,
`File does not exist: <path>`, `Permission denied: <path>`.

### 5.6 Edit buffer and cursor extraction (screen-scraped, cell tags)

Inputs: model grid, cursor `(cl, cc)` (viewport line, column; `cc = cols` in the pending-wrap state), current cycle
`n` (None while `preexec` or before the first `NewCmd` → no buffer), shell, suggestion colour (§5.1.3).

```text
tagged(r)           = row r contains a cell tagged "tabsmith:c<n>"
is_cmd(cell, pos)   = cell tagged "tabsmith:c<n>" && !(pos >= (cl, cc) && is_suggestion(cell))   // D24
prompt_row(r)       = row r contains a cell tagged "tabsmith:p"
wraps(r)            = last cell of row r has WRAPLINE

start = cl; while start > topmost && cl-start < 100 && (wraps(start-1) || tagged(start-1)): start -= 1
end = cl;   unless pwsh ListView is active:
            while end < bottom && (wraps(end) || (first cell of row(end+1) is a prompt cell && tagged(end+1))): end += 1
            // below the cursor only wrap continuations and PS2 rows; not zsh completion listings, not ListView rows

// at-prompt rule (D42): the cursor's logical line (rows joined by auto-wrap, top..=cl) starts on a prompt
top = cl; while top > topmost && cl-top < 100 && wraps(top-1): top -= 1
at_prompt = any prompt_row in top..=cl                        // PS1's last line, PS2, a markerless prompt NewCmd tagged
         || (pwsh && row(top) starts with the ContinuationPrompt)
         || (line_col0 && prompt_row(top-1) && !tagged(end+1))   // PS1 ending in a newline; nothing of this cycle
                                                                 // drawn below (fzf Ctrl-T from an empty line)

buf = "", cursor = None
for r in start..=end:
   if r == start || !wraps(r-1): pad = 0; line_has_cmd = false     // a new logical line
   for c in 0..cols:
      if (r, c) == (cl, cc): at_cursor()
      cell = grid[r][c]; if WIDE_CHAR_SPACER or LEADING_WIDE_CHAR_SPACER: continue
      if is_cmd(cell, (r,c)):
          if line_has_cmd: buf += " " * pad                         // blanks between command cells
          pad = 0; line_has_cmd = true; buf.push(cell.c); buf += cell.zerowidth
      else: pad += 1; shell_ghost |= cell tagged c<n>               // prompt, untagged blank, suggestion, other cycles
   if r == cl && cc >= cols: at_cursor()                            // pending wrap
   if r < end && !wraps(r) && (row had a cmd cell || buf non-empty): buf.push('\n')
at_cursor(): if line_has_cmd: buf += " " * pad; pad = 0             // zsh moves over a trailing blank with CUF on a
             cursor = Some(buf.len())                               // redraw: `git ` must stay `git `, not `git`
cursor = cursor.unwrap_or(buf.len())
pwsh: strip_prompts(buf, cursor)
buf.truncate(max(buf.trim_end().len(), cursor))                     // blank command cells after the cursor
return Extracted{buf, cursor, shell_ghost, at_prompt}
```

pwsh `strip_prompts`: PSReadLine draws its ContinuationPrompt (each line after a real `\n`) and a re-rendered
PromptText (first line, e.g. red on a parse error) after EndPrompt, so they carry the command tag; they are removed
(the reported `PwshPrompts`, §5.1), keeping the cursor on the same character. ponytail: a first line the user typed
starting with the PromptText loses it too.

Retagging: the tags this walk relies on are fixed up at `NewCmd` (§5.1 "Line start"): untagged cells before the
cursor become prompt (a markerless prompt), and on a prompt redraw the old cycle's cells are re-tagged, so text that
readline keeps on screen while it redraws only the prompt (vi-mode switch, leaving i-search) stays in the buffer.

Who uses what: the live UI (requests, menu, ghost, interception) uses the buffer only when `at_prompt` holds and not on
the alternate screen; otherwise there is no current buffer (`cur = None`) and every key goes to the shell (an inline
TUI's rows: fzf `--height`, fish's pager search field). `PreExec`'s scrape ignores `at_prompt`, because a transient
prompt is redrawn without markers before the line and often on another row (§5.1, D37).

Properties: the cursor is a **byte offset into `buf`** but every count derived from it
uses chars; multi-line commands contain real `\n`; RPROMPT/PS2/suggestion text excluded; untagged prompt cells before
the first command cell of a line are not padding; text printed below the command line excluded; scroll and reflow
need no coordinate tracking. ponytail: continuation lines without a prompt (zle Alt-Enter, fish), an empty PS1, a
markerless prompt ending in a newline, and a newline-ended PS1's line while zsh lists completions below it read as
not at the prompt (no menu); recording the NewCmd row with scroll tracking is the upgrade.

---

## 6. Completion engine (engine thread)

Per request: `buffer[..cursor]` → tokenize → select command → expand aliases → parse against specs → assemble
suggestions → trigger generators → filter + rank → precompute insertions → reply (phase `Parse`); then await
triggered generators and reply again (phase `Generators`). Only the latest request is served: before starting and
whenever the event pump runs, a newer `Complete` aborts the current one.

### 6.1 Tokenizer

#### 6.1.1 Grammar

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
  A word that starts with the escape char keeps it in `raw` (`\ fo`, not ` fo`: D22 counts raw chars).
- Adjacent literals form one `concatenation` argument: `--foo="a b"c` is one token, inner text `--foo=a bc`.
- Whitespace (space/tab) ends an argument; newline does not split words (only terminates `$var`).
- Assignments: while the rest matches `^[\w\[\]]+\+?=`, parse `name(=|+=)value` (value ends at a space).
  An empty value (`FOO= cmd`) is a complete empty assignment.
- `{`/`(` start compound/subshell; `&&`/`||` → flattened `list`; `|`/`|&` → `pipeline` (binds tighter than lists).

#### 6.1.3 Current command and tokens

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
| `FOO= git ` | `git`, `""` |
| ` ` (one space) | none |

#### 6.1.4 pwsh

Same parser with escape char `` ` `` instead of `\` (so no backtick substitution, and `\` is a plain char) and no
`$'…'`; quoting rules for insertion differ (§6.8). (PowerShell syntax is otherwise close enough for completion: `;`,
`|`, `&&`, `||`, `'…'`, `"…"`.) ponytail: `'it''s'` reads as `its` and a leading `&` call operator ends the command
(bash rules).

#### 6.1.5 Mid-word guard

If the char at `cursor` exists and is not whitespace (or the cursor is not on a char boundary) → reply `error=true`
without a ghost.

#### 6.1.6 Aliases

Alias map from `run/<sid>.json` `alias` text: strip `^alias\s` per line, split lines; separator `" "` for fish, `"="`
otherwise; `[key, ...rest] = line.split(sep)`; value = `rest.join(sep)` with one leading and one trailing `'` stripped;
empty values are not aliases. The map is rebuilt whenever a new session file arrives. Expansion: while
`tokens.len() > 1` and `aliases[tokens[0].text]` exists and was not used yet: replace the first token's text range by
the alias value in the command text, re-parse, require exactly one top-level `command` (aliases containing `|`, `&&`,
`;` are not expanded), map new tokens inside the alias span to the alias token's original raw node. The command text
**before** expansion is kept too (`original_text`): it is what the menu compares between keystrokes (§7.1), so
`g ` (alias `g=git`) is typing, not a large change. Tests:
`woman` → `woman`; `woman ` → `man ""`; `man woman ` unchanged; `quote ` (alias `'q'`) → `q ""`.

### 6.2 Spec model and normalisation (replaces `@fig/autocomplete-shared` `convertSubcommand`, inferred)

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
generator `{template}` prepended (generators whose template includes `filepaths` or `folders` get default
`trigger:"/"`, `getQueryTerm:"/"`; the engine treats both as path queries, §6.6.4); options with `isPersistent` go
only to `persistent`; string `loadSpec` → `Locations([{global, name}])`; object → `Inline(normalise)`; name collisions
in an index: later wins; `isRepeatable`: absent → `No` (=1 for suggestions, not enforced by the parser),
`false` → `Times(1)` (enforced), `true` → `Unlimited`, number → `Times(n)`; `debounce: true` → 200 ms;
`optionArgSeparators` as a string = its chars.

### 6.3 Spec resolution and loading

`spec_location(name, cwd, is_script: Option<bool>)` (shortcuts `?`/`+` dropped):
- first token (`None`): `name == "bin/console"` or ends with `/bin/console` → global `php/bin-console`; no `/` → global `name`; else local.
- `isCommand` arg (`Some(false)`): path not starting with `/`, `./`, `~/` → global `name` (`npx @capgo/cli` → `@capgo/cli`).
- local: path starting `/` or `~/` → `{local, dir, basename}`; else relative to cwd (strip `./`). Local specs load
  `<dir>/.fig/autocomplete/build/<basename>.js` (else `<basename>/index.js`; a leading `~` = home).
- `isModule:"python/"` + token `http.server` → global `python/http.server`.

`resolve(global name)` → file: names with anything but plain relative components (absolute, `.`, `..`, a Windows
drive prefix such as `C:foo`) → None. Search roots in order (dev folder when `autocomplete.developerMode`, leading
`~` expanded; then `~/.tabsmith/specs/user/`, then `~/.tabsmith/specs/<ver>/`): `<name>.js`, else `<name>/index.js`.
Diff-versioned names (`index.json.diffVersionedCompletions`) go straight to `<name>/index.js`. Missing → None
(silently; e.g. `gcloud/alpha`, `gcloud/beta` are missing upstream).

`load(location)`: name in `autocomplete.disableForCommands` → None. Cache key `type,path,name` (no cache in dev mode;
`clear_caches` on every settings reload). 5 s timeout (interrupt handler + pump deadline). Module evaluation in §9.2.
Spec **identity** is kept: the last normalised `Rc<Subcommand>` per key is reused while the spec's JSON is unchanged
(dev-mode reloads, cache clears), and the first-token spec is one shared `Rc`; the engine keys argument and generator
identity on these addresses, so an unchanged spec is never a "new argument". Then:
- `default` is an object → normalise.
- `default` is a function (6 diff-versioned `index.js`): version = memo `cliVersion-<name>` (successful detections
  only; bypassed in dev mode): `getVersionCommand(exec)` if exported, else
  `semver_clean(exec({command:name, args:["--version"]}).stdout)`; errors/timeout → `None`; `@usermn/sdc` → `None`
  (no `npx`). A detection whose wait is aborted by a newer request keeps running and the next parse waits on the same
  task (one `az version` per session, not one per keystroke). `r = await default(version)`. If `r.versionedSpecPath`
  → load `<dir>/<base>.js` (`heroku/8.6.0`; ponytail: next to `<name>/index.js`), take its `default` (base) and
  `versions`, apply diffs; else `r` is the spec.

Version file pick (decoded from `specs/heroku/index.js`, runs inside the spec's own JS): sort versions with semver;
no target → newest; else newest `<= target`, else newest.

Diff application (Rust; the helper `getVersionFromVersionedSpec` is not on disk, inferred order): sort `versions`
keys by semver; for each key `<= target` (all if `target` is None or unparsable), `base = merge_subcommand(base, versions[key])`.

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

### 6.4 Argv state machine

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
         value = (string | await fn(token, exec)).trim_end(); re-tokenize command with token → value (§6.1.6
         substitute; substitutions stack on the already substituted command);
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
  search_term = last annotation text ("" after a trailing space); arg_st = arg_state(st); raw = raw text of the last token
  if last annotation is composite: arg_st = opt_args
     if its last subtoken is option_arg: search_term = subtoken text; only_args = true
        raw = raw minus the flag part in front of the value (`--color=al` → `al`, `-oal` → `al`)   // D22
     else: option_chain = whole token (§6.7.1 step 5)
  current_arg = cur(arg_st)
  flags = ARGS | (!only_args && !end_of_options && can_subcommands ? SUBCOMMANDS) | (… && can_options ? OPTIONS)
```

Guards: `loadSpec` recursion deeper than 64 is an error (a cyclic user spec would overflow the stack); the
prefix and generateSpec caches are cleared wholesale at 1000 entries (ponytail: wholesale instead of per-entry eviction); a cached nested
state gets the current command's token prefix; an abort from any JS call always propagates. Inline loadSpec
directives replace the parent's whole object (ponytail: 4 upstream specs). generateSpec failures are logged with the
spec name only (logs never hold typed text).

Expected outcomes (mock spec:
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
ctx = { currentWorkingDirectory: tracker.cwd (`Dir=`), else the wrapper's cwd, currentProcess: shell name, sshPrefix: "",
        environmentVariables: session env (run/<sid>.json, else wrapper env),
        searchTerm: result.search_term,            // MUST be real: the bundled filepaths() relies on it (ish bug)
        isDangerous: current_arg.is_dangerous }
tokenArray = token texts from command_index (includes the partial last token)
```

Empty cwd → script/custom generators return `[]`.

### 6.6 Generators

#### 6.6.1 State and triggers

The engine keeps `Slot { key, result, loading, gen_id }` per generator of the current arg (identity = the arg's `Rc`
address, kept stable by §6.3). Per request, for each generator `g` at index `i`:
- trigger if no slot / arg identity changed (the slots are rebuilt empty: results are not carried over to another arg);
- else `filepaths`/`folders` template → the directory part changed (`/`, pwsh also `\`, D33);
  `g.trigger` undefined → trigger only if `arg.debounce`; string `t` → `a.rfind(t) != b.rfind(t)` (char index);
  function → `call(trigger, [a, b])` (sync, 1 s; exception ⇒ trigger); `{on:"change"}` → `a != b`;
  `{on:"threshold", length:n}` → `a.len() > n && !(b.len() > n)`; `{on:"match", string}` → index of `a` in the list ≠ index of `b`.
- A triggered slot keeps its previous result (`loading = true`); with `arg.debounce` wait `debounce ms` (number > 0) or
  200 ms before running (abandon if superseded). One deadline per run: debounce + 2 × max(scriptTimeout,
  g.scriptTimeout) + 1 s (ponytail: custom generators may exec several times); past it the run fails (`[]`).
- `triggered_generators` in the reply = keys of slots whose run was started (used by the visibility machine, §7.1).
- While a slot is loading, its stale results are shown only if the generator has no `getQueryTerm`.
- Slots are reset on `CommandFinished`/`CommandEnded`, on settings reloads and on error replies.

#### 6.6.2 Run

- `template` → §6.6.4. Else `script` → §6.6.3. Else `custom(tokenArray, exec, ctx)` → awaited; `undefined` → `[]`;
  items get `type: item.type || "arg"`; if `filterTemplateSuggestions` and first item has `context` → apply it.
- Every result item is tagged with the generator key (for `getQueryTerm` and §7.1). Any error → `[]` + log.

#### 6.6.3 Script generators

```text
cmd = script is fn ? call(script, [tokenArray]) : script;  falsy → []
input = array ? {command: cmd[0], args: cmd[1..], cwd} : string ? {command:"sh", args:["-c", s], cwd}   // legacy (none in 2.692.3)
                                                          : {cwd, ...cmd}          // Windows: §9.3 picks sh / Git Bash / cmd /C
timeout = max(autocomplete.scriptTimeout, g.scriptTimeout, input.timeout)
out = cached_run(key = g.cache.cacheKey ?? json(input)) → exec(input, timeout)
items = splitOn ? (out.stdout.trim()=="" ? [] : out.stdout.trim().split(splitOn))
      : postProcess ? call(postProcess, [stdout, tokenArray]).filter(x → x && (string || x.name))
      : []
strings → {type:"arg", name:s, insertValue:s, isDangerous}; objects → type default "arg"
```

#### 6.6.4 Templates

- `filepaths` / `folders`: the bundled `filepaths()` helper (71 specs) calls `exec({command:"ls", args:["-1ApL"],
  cwd: dir})`, which tabsmith answers **natively** (§9.3; a missing dir lists nothing, D34), so the helper keeps its
  exact JS semantics. Template generators (`template:"filepaths"`) run in Rust instead (no JS) with the same rules:
  `dir` = `getCurrentInsertedDirectory(cwd, searchTerm)` (expand `~`, `$VAR`, `${VAR:-d}` from env; take up to the last
  separator; relative → `cwd/` + it); names sorted non-dot by `localeCompare` (approximation §6.7.1), then dot names,
  `.DS_Store` and names with control characters skipped (D32), `"../"` always appended; an unreadable dir → `[]`;
  trailing `/` → `{type:"folder"}` else `{type:"file"}`; `context.templateType`; trigger = the directory part changed;
  queryTerm = after the last separator. Separator = `/`, in pwsh also `\` (D33; the JS
  helper still splits on `/` only).
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
g.cache absent: results live in the slot until the trigger fires; cleared on CommandFinished / CommandEnded
failures are never stored; in-flight identical keys are shared
```

### 6.7 Assembly, filtering, ranking

#### 6.7.1 Assembly, in this order

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

#### 6.7.2 History rows (§6.10)

#### 6.7.3 Filter (`filterSuggestions`)

```text
query(s) = s.get_query_term or generator.get_query_term:
             string g → search_term[ search_term.rfind(g).map(+g.len()).unwrap_or(0) .. ]   (D23)
             fn g     → call(g, [search_term]) (sync; error → search_term)
           else search_term
auto_exec_hidden = hideAutoExecuteSuggestion || onlyShowOnTab

if search_term == "":
   out = s.filter(!hidden)  (priority order; none counts as prefix-matched: no spec ghost on an empty token, §6.11)
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

#### 6.7.4 fuzzysort (fresh struct per call)

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

#### 6.7.5 Ranking pipeline

```text
s = assembly (+ history per beta.history.mode / history_mode)
s = update_priorities(s, cmd = tokens[command_index])          // D21
s.stable_sort_by(priority desc)
s = filter(s, …)
if 0 < s.len <= 50: dedupe by (names, insert_value, display_name, args) keeping first

update_priorities: use_recency = sortMethod != "alphabetical"
  for x: name = first name; rec = use_recency && name != "../" ? recency[cmd][name] : None
         boost = rec.map(ms / 1e13).unwrap_or(0)
         p = x.priority.filter(|p| p != 0).unwrap_or(50)        // 0 means unset and is treated as 50
         if x.type != auto-execute: p = clamp(p, 0, 100)
         p = rec.is_some() && 50 <= p <= 75 ? 75 + boost : p + boost
```

`RecordInsert{cmd, name}` (sent by main on every full insertion, skipped if name contains `↪`) sets
`recency[cmd][name] = now_ms` and persists `state.json` 1 s after the first unsaved insert (ponytail: a throttle, not
a trailing debounce; again at shutdown). The save merges with the file (newest timestamp per entry wins), so
concurrent sessions keep each other's entries, and is skipped when the data root is gone (§3.1).

### 6.8 Insertion text (precomputed per item; with D22/D25 and D26–D33)

```text
q = query(item)                                  // §6.7.3; path query = after the last separator (pwsh: also `\`)
raw = raw text of the current token as typed     // composite option value: the value part only (§6.4)
(logical, open) = unquote(raw, shell)            // each logical char with the raw index it starts at (quotes
                                                 // dropped, escapes `\x` / pwsh `` `x `` and pwsh `''` resolved),
                                                 // plus the quote left open and its index
delete = item.type == auto-execute ? 0           // special rows delete like any row (D31)
       : q == search_term ? raw.chars().count()
       : string getQueryTerm g ? raw[raw.rfind(g)+g.len()..].chars().count()
       : path query ? raw[after the last separator..].chars().count()
       : /* function getQueryTerm */ the last q.chars().count() logical chars spell q ? raw chars from the first of them
                                     : approximation (each whitespace counts 2), at most raw.chars().count()
deleted = the last `delete` chars of raw
quote   = deleted starts with ' or "             // the user started the token with a quote: keep quoting
inside  = the open quote, if it opened before the deleted part   // `cat "my dir/x`
text = item.insert_value if set and the item is not a file/folder      // verbatim, may contain {cursor}, \n
       else: m = name_match(item, q) (none → "")
             name = inside ? escape_in_quote(m, inside, shell) + (file ? inside : "")   // a folder leaves it open
                  : pwsh ? escape_pwsh(m, folder, quote, path) : escape_posix(m, folder, quote, path, shell)
             name + (sep ? sep + "{cursor}" : "")   // only the name is escaped: `--out=` keeps the cursor unquoted
if text ends with one or more "\n": keep exactly one
if !text.ends_with('\n') && item.should_add_space && insertSpaceAutomatically: text += " "
if q != "" && deleted == q && text.starts_with(q): delete = 0; text = text[q.len()..]   // don't retype what is typed
execute = text.ends_with('\n') → strip it, send "\r" after the text (D25)
text.retain(!is_control)                         // D32: names reach the PTY as typed bytes
cursor_back = if text contains "{cursor}" at char i: remove it; text[i..].chars().count() else 0

name_match(item, q): candidates = names matching q (prefix: ==, eq-ignore-case, starts_with, starts_with-ignore-case;
   fuzzy: fuzzysort match or q empty), original order; preferVerboseSuggestions && type ∈ {option, subcommand} →
   longest, else first; none but display_name exists → pick from all names the same way
path = item.type ∈ {file, folder};  is_var_ref(s) = "$" + one or more of [A-Za-z0-9_:]

escape_posix(s, folder, quote, path, shell):                             // D26, bash/zsh/fish
   safe(c) = alphanumeric || c ∈ "-_./+,:@%=~"
   tilde   = path && (s starts with ~, = or % || s contains "=~" or ":~")   // a file's ~ would expand; spec `cd ~` means it
   if !quote && !tilde:
       every char safe, or (!path && is_var_ref(s)) → s                     // D27
       every char safe or ' '                       → s with ' ' → "\ "
   folder → quote(shell, s minus "/") + "/"; else quote(shell, s)             // §5.3: '…'\''…' / fish \' \\
escape_pwsh(s, folder, quote, path):
   special(c) = whitespace || c ∈ $ ` ' " ( ) { } ; , & | @ # < > or a smart quote U+2018–U+201E   // D29
   if !quote && (no special char || (!path && is_var_ref(s))) → s
   folder → every special char prefixed with "`" (a bare word, D30); else quote(pwsh, s)
escape_in_quote(s, q, shell):                                             // D28, no quotes added
   '  pwsh: quote(pwsh, s) without its outer quotes;  fish: \ → \\, ' → \';  bash/zsh: ' → '\''
   "  pwsh: ` " $ U+201C–U+201E prefixed with `;  fish: \ " $ prefixed with \;
      bash/zsh: \ " $ ` prefixed with \, ! → "'!'" (history expansion inside double quotes)
```

Examples (tests): buffer `git st`, item `status` → delete 0, text `atus `; buffer `git ch` + `checkout` → `eckout `;
option `--out` requiresSeparator + mandatory arg → text `--out=` + cursor_back 1 then auto-space (so `--out= `
with the cursor right after `=`); `ls --color=al` + `always` → delete 0, text `ways`; `cd my\ di` + folder `my dir/` →
delete 6, text `my\ dir/`; `cat 'my fi` + file `my file` → delete 6 (raw `'my fi`), text `'my file'`;
`cat "my dir/x` + file `x y.txt` → delete 0, text ` y.txt"`; branch `zz;id` → `'zz;id'`; `cd ~` + spec row `~`
stays `cd ~` (never `'~'`);
`echo $HO` + `$HOME` → delete 0, text `ME` (pwsh too); pwsh `cat di` + folder `dir with space/` → text
``r` with` space/``; pwsh file `a’;x;’.txt` → `'a’’;x;’’.txt'`.

Common prefix (`insertCommonPrefix`) is computed in main from the item list (§7.2.5).

### 6.9 First-token completion (`autocomplete.firstTokenCompletion`, default false)

Single token without `/`: when the setting is off → spec with no args (no suggestions). When on: items =
(root spec names from the index, `type:"subcommand"`, priority 40) ∪ (alias names, `type:"shortcut"`, priority 100,
description = alias value) ∪ (executables in the session PATH, `type:"subcommand"`, priority 40; Windows: files with
a `PATHEXT` extension, shown without it), deduped by name, cached 10 s; the cache is dropped when a new session file
(aliases, PATH) arrives and on settings reloads. Token containing `/` → `dotslash` spec
(`{args:{template:["filepaths","folders"]}}`) or `php/bin-console`.

### 6.10 History rows

`prefix = buffer[..start of last token]`. For history entries newest first with `command.starts_with(prefix)`:
`name = command[prefix.len()..].trim_end()`, skipped when empty or when it contains a control character (a multi-line
entry: typing its `\n` would run part of it, stripping it would change it; D32), unique, `{type:"history", description:"past command", name,
insertValue:name, icon:'h', priority: freq(first word of name) > 1 ? 75 + min(freq,10)/10 : 50}`.
`beta.history.mode == "show"`: append rows whose name is not already a spec suggestion name; then each spec item's
priority = max(own, priority of the history row whose first word equals one of its names).
`"history_only"` or the `toggleHistoryMode` flag: only history rows.

### 6.11 Inline ghost (local) in the same reply

`ghost` = per `inline.source`:
- `history` / `history+specs` first: newest history entry with `exit_code == 0` (fallback: any) such that
  `command.starts_with(buffer) && command != buffer` → `command[buffer.len()..]`.
- `specs` / `history+specs` second: first item that is not auto-execute/history, whose insertion has `delete == 0`,
  a `cursor_back` that stays within the trailing space (accepting leaves the cursor at the end: `commit -m '{cursor}'`
  is skipped), non-empty text after trimming a trailing space, and that came from a prefix (not fuzzy) match — never
  on an empty token (§6.7.3) → that text.
- LLM profile: history first (as above); if none, main schedules an LLM request (§8.4).
Only when `inline.enabled`, buffer non-empty and cursor at the end of the buffer. Error replies (a command without a
spec, no command at the cursor) carry the local ghost too; the mid-word error does not.

---

## 7. Dropdown UI (main thread)

### 7.1 Visibility state machine

```rust
enum Vis { Visible, HiddenUntilKeypress, HiddenUntilShown, HiddenByInsertion }
struct Menu { vis: Vis /* init HiddenUntilKeypress */, items, selected: usize, sticky: bool /* hasChangedIndex */,
  scroll_top, last_inserted_gen: Option<GenKey>, just_inserted: bool, history_mode: bool,
  popout: bool, last: Option<Completion>, loading_since: Option<Instant> }
```

- **Error reset**: items = [], vis = HiddenUntilKeypress, history_mode = false, selection reset, `last = None`
  (the pop-out flag survives). Fired by: `Completion.error` (mid-word cursor, no spec, no command), an empty buffer
  (§7.3 `refresh`), `PreExec`, a `NewCmd` that finished a command (`CommandFinished`/`CommandEnded`), a forwarded
  Ctrl-C, Ctrl-D or Enter (the newline echo would re-anchor the stale menu below the line until PreExec), leaving the
  alternate screen.
- **Prompt redraw** (`NewCmd(None)`: SIGWINCH, `zle reset-prompt`, a clock or async theme, a nested shell's first
  prompt): menu, ghost, lock and the last request are kept, and the unchanged buffer is **not** re-requested (its reply
  would count as a keypress and re-show a menu hidden by Esc, Up or a large change). Settings
  and session files are re-checked. For 50 ms an empty line is not acted on (a redraw whose line arrives in a later
  read); after that an empty line resets as usual.
- **On a phase-Parse reply** `c` (only if `c.req` is the latest request; `steps` = requests since `last`, ≥ 1):

```text
if last is None (after a reset): has_new_arg = true; backspaced = false; large = !typing(c.command_text, "", 1)
else:
  has_new_arg = last.arg_key != c.arg_key
  backspaced  = 0 < c.tokens.len < last.tokens.len && last.tokens[c.tokens.len-1] == c.tokens[c.tokens.len-1]
  large       = !typing(c.command_text, last.command_text, steps)
typing(a, b, k) = chars(a) + chars(b) - 2 * common_prefix_chars(a, b) <= k     // at most k typing edits
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

  `command_text` is the text before alias expansion (§6.1.6). The engine serves only the latest request, so replies
  for the buffers typed in between never arrive: one typing edit per request since `last` is allowed.
  ponytail: a large change followed by skipped typing stays hidden until the
  next key; exact = send the previous request's command text with each request.
- **On a phase-Generators reply**: `set_items(c.items)` only.
- `set_items`: always replaces the list (also while HiddenUntilKeypress: show/toggle/Tab must never act on a stale
  list). If `sticky`, the menu is Visible and the previously selected item (by
  `names, type, description`; the query-stripped insertion text changes every keystroke) is in the new list → keep it
  selected and scrolled into view; else `selected = 0, sticky = false, scroll_top = 0`.
- "Shown" ⇔ `vis == Visible && (items non-empty || current_arg has name/description)` and not `autocomplete.disable`.
  Drawn ⇔ shown and the render guards of §7.5.1 allow it.

Behavioural summary to test: typing keeps the menu; a space shows the next level immediately; after a full insertion
the menu reopens only for a new argument or a re-triggered generator (folder `src/`); Esc hides for the rest of the
line (no timer) and only show/toggle, Tab+onlyShowOnTab, backspace into the previous token, a large change, or an
error reset bring it back; Up at index 0 hides (or toggles history with `navigateToHistory`).

### 7.2 Keys, bindings, actions

#### 7.2.1 Decoding stdin (whole chunk, sequentially)

A chunk is split into keys; bracketed paste `ESC[200~ … ESC[201~` is one opaque unit (never intercepted). The decoder
is kept across reads, so a paste split over several reads (macOS ttys deliver ≤ 1 KiB, the reader 4 KiB, Windows 64
input records) stays opaque until `ESC[201~`.

| bytes | key |
|---|---|
| `\r`, `\n` | `enter` (Ctrl-J/Ctrl-M are Enter) |
| `\t` | `tab` |
| `ESC [ Z` | `shift+tab` |
| `ESC [ A/B/C/D`, `ESC O A/B/C/D` | `up/down/right/left` (DECCKM variants included) |
| `ESC [ 1 ; m A..D` | arrows with modifiers (`m-1` bits: 1 shift, 2 alt, 4 ctrl) |
| `ESC [ H`, `ESC O H`, `ESC [ 1 ~`, `ESC [ 7 ~` / `ESC [ F`, `ESC O F`, `ESC [ 4 ~`, `ESC [ 8 ~` | `home` / `end` |
| `ESC [ 3 ~`, `ESC [ 2 ~`, `ESC [ 5 ~`, `ESC [ 6 ~` | `delete`, `insert`, `pageup`, `pagedown` |
| `ESC O M`, `ESC O I` | `enter`, `tab` (keypad application mode, DECKPAM/smkx) |
| `ESC O P..S`, `ESC [ 11..24 ~`, `ESC [ P/Q/S` | `f1..f12` |
| `0x7f`, `0x08` | `backspace` |
| `0x01..0x1a` (others) | `control+<a..z>` (case-insensitive) |
| `ESC` alone (last byte of the chunk), `ESC ESC` | `esc` (the first ESC) |
| `ESC ESC [ …` / `ESC ESC O x` | `alt+<key>` (meta-sends-escape Alt+arrow: unbound, so it reaches the shell whole); not before `ESC[200~` |
| `ESC <utf8 char>` | `alt+<char>` |
| `ESC [ code ; mods u` | CSI-u key (rare: tabsmith strips kitty enable requests, §7.5) |
| other UTF-8 char | `<char>` |
| CPR reply `ESC [ r ; c R` | forwarded to the PTY untouched (the shell asked); a late reply to the startup probe is dropped (§4.4) |
| anything else (`ESC [ ?…`, NUL, other C0) | unknown: forwarded untouched |

#### 7.2.2 Binding grammar

Split on `+` from the left; all but the last segment are modifiers (`ctrl|control`, `shift`, `alt|option`,
`meta|command`; unknown → ignored); `"+"` alone and `"ctrl++"` work. Key names: `backspace enter left|arrowleft
right|arrowright up|arrowup down|arrowdown home end pageup pagedown tab delete insert esc f<N>` or exactly one char.
`shift+<lowercase letter>` → uppercase letter, no shift. Ctrl/Alt + letter match both cases. Arrows also match their
`ESC O x` forms.

#### 7.2.3 Default bindings (D5) and overrides

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
and does nothing. Table rebuilt on settings reload.

#### 7.2.4 Interception

```text
on_input(chunk): drop a late startup-probe reply (§4.4); if keys are held (§7.3): append the chunk to them; return
typed = 0
for key in decoder.decode(chunk):
   prompt       = !preexec && !alt_screen && cur.is_some() /* at the prompt, §5.6 */ && typed < 2    // D39
   shown        = prompt && menu intercepting (shown, items non-empty)
   intercepting = shown && drawable()          // the render guards of §7.5.1 let the menu be seen
   global       = prompt && !shown && !autocomplete.disable && items non-empty
   binding      = prompt ? keymap.lookup(key) : None
   tab_show     = global && onlyShowOnTab && key == plain tab
   ghost        = prompt && drawable() && key ∈ ghost-accept keys (§8.3) && ghost_text(after typed-ahead keys)
   acts         = ghost || tab_show || (intercepting && binding is an action or unknown)
   if acts && echo_pending: hold this key and the rest of the chunk (§7.3); break            // D40
   if intercepting && binding is an action → handle; swallow      (unknown action: swallow, nothing)
   elif tab_show → onlyShowOnTab preamble (§7.2.5); swallow
   elif global && binding ∈ {showAutocomplete, toggleAutocomplete} → handle; swallow
   elif ghost → write the ghost (whole / next word), add it to the typed-ahead text; swallow
   else forward the raw bytes:
        Enter at the prompt → tracker.submitted = basis (§7.3)    // the recorded command, D37
        plain printable char → typed += 1
        note_forwarded(key)                                       // §7.3 typed-ahead model
        control+c, control+d, enter → menu error-reset; redraw
after the loop: re-request if an action asked for it; redraw; ONE write of all forwarded/inserted bytes (last, so a
panic above leaves the read for the guard, §4.3)
```

Esc is swallowed and stops interception immediately (the next Esc reaches the shell; vi-mode users can bind
`autocomplete.keybindings.esc` to `ignore`). With `autocomplete.disable` nothing is ever intercepted for the menu.

#### 7.2.5 Actions

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

Common prefix: 1 item → full insertion; selected type (or `original_type` for auto-execute)
∈ {special, auto-execute} → fail; candidates = items of a matching type (`../` only matches `../`; file/folder match
each other) whose lowercase first name starts with `q.lowercase`; 1 candidate → full insertion (auto-execute → fail);
`lcp` = longest common prefix of the sorted lowercase names; `shared = first_name(selected)[..lcp.chars]` (selected
item's casing); empty or `== q` → fail; text = shared with whitespace → `\ `. `full` = the selected row's insertion
text, escaped by the engine for the shell (the query put back when the engine stripped it); `text` must be a prefix of
`full`, else fail (BEL): the shared prefix is typed only as the start of a correctly escaped insertion (`$XDG_` of
`$XDG_CONFIG_HOME` works; `zz;` of `'zz;a'` and pwsh `my\ dir` of `'my dir1.txt'` fail; ponytail: a prefix that
needs quoting is not quoted half-way). `text == full` (or + space) → full insertion; else partial insertion (delete =
the row's raw delete count, 0 with the typed query not retyped, D22; `just_inserted = true`, menu stays visible).

### 7.3 Writing insertions to the PTY

Typed-ahead model (reconciled in the same read, not after the keys have echoed):

```text
unechoed     = printable keys forwarded at the end of the line that the extracted buffer does not show yet
note_forwarded(key): plain char at the end of the line → unechoed.push(c)
                     anything else (Backspace, arrows, Ctrl-…, paste) → unechoed = ""; echo_pending = Some(req)
on every extraction: drop from unechoed what the new buffer added after the basis (else clear it, unless the pending
                     insertion has not echoed yet)
basis()      = (lock ? lock.expected : cur.buffer) + unechoed     // the buffer once the shell echoed all sent bytes
echo_pending: cleared by a reply for a later request, a new prompt/PreExec, or an empty line
held keys    : a key that would act on the menu/ghost while echo_pending is set is held with everything after it
               (decoder reset), and replayed through on_input once echo_pending clears or after 100 ms (D40)
```

```text
full insertion of item (precomputed delete, text, cursor_back, execute), dropped while preexec:
  ib  = the buffer and cursor the menu's last reply was computed from (else the extracted ones)
  cur = basis()                                          // not just the last batch: keys sent ahead are reconciled
  bytes = (cur extends ib ? "\x7f" × (cur.chars − ib.chars)        // the user typed more meanwhile
           : ib extends cur ? ib[cur.len()..] : "")              // the shell has not echoed yet
        + "\x7f" × delete + text + "\x1b[D" × cursor_back + (execute ? "\r" : "")
  forward bytes; unechoed = ""
  expected = ib[..cursor] minus `delete` chars + text + ib[cursor..]
  if execute: tracker.submitted = expected               // the recorded command, as for Enter (D37)
  lock = (deadline now+100 ms, cap now+1 s, expected, buffer before)
  vis = HiddenByInsertion; last_inserted = item's generator; just_inserted = true
  EngineMsg::RecordInsert{cmd: tokens[command_index], name: first name}   (not for `↪` rows)
partial (common prefix): same bytes path and lock; only just_inserted = true; no record
execute action: "\r", tracker.submitted = basis()

refresh() (after every PTY batch, on lock expiry, on re-requests):
  cur = at_prompt extraction (§5.6) or None; update unechoed
  while locked: buffer == expected → unlock; before the deadline → return (changes do not reach the menu); past it but
       the buffer is still the pre-insertion one and the 1 s cap is not reached → re-arm 100 ms (D41); else unlock
  within 50 ms of a prompt redraw an empty buffer is ignored (§7.1)
  unchanged (buffer, cursor) since the last request → return
  req += 1; empty/blank buffer → menu error-reset, no request; else EngineMsg::Complete(Request)
```

### 7.4 Layout and content

#### 7.4.1 Geometry

```text
rows R, cols C, cursor (cy, cx) from the model (0-based viewport)
w      = min(max(autocomplete.width, 10), C)        // default 40
n_max  = max(autocomplete.height, 1)                // default 7
n      = min(n_max, items.len())
h      = n + 1                                      // list + description bar; arg-hint mode (no items): n = 0, h = 1
below  = R - 1 - cy;  above = cy
side   = below >= h ? Below : above >= h ? Above : (below >= above ? Below : Above)
room   = side == Below ? below : above
if room < h: if items == 0 || room < 2 { hide } n = room - 1; h = room
x0     = clamp(cx - width(query of selected item) - 4, 0, C - w)
         // rows start with ' ' + icon (2 cols) + ' ': the title's first char lands under the typed query's first char
block order is always list rows then the description row:
  Below → list on cy+1 ..= cy+n, description on cy+n+1
  Above → list on cy-h ..= cy-h+n-1, description on cy-1 (block bottom-aligned to the cursor)
```

#### 7.4.2 Rows

List row for item `s` (selected row uses selected colours):
`' '` + icon (2 cols) + `' '` + title + (`' '` + args hint in dim when room) + pad to `w`.
- Icon slot: always two columns. A spec `icon` that is a single wide emoji
  is text (`⭐` for the current git branch). With the kitty graphics protocol (Ghostty, kitty; inside tmux when
  `tmux display` reports `allow-passthrough` on and a kitty or Ghostty client), `fig://icon?type=<t>` is that PNG and `fig://template?color=&badge=` the template
  tinted by `scale_u8` (`a·b/256` per channel, alpha kept) with the badge in white. Otherwise the row type's
  image: subcommand `command`, option `option`, arg `box`, auto-execute `carrot`, file `file`, folder `folder`,
  shortcut `💡` and mixin `➡` on tinted templates, history `📚`.
  PNGs live in `assets/icons` (see its README). Each image is transmitted once (`a=t`), placed every frame over the
  slot (`a=p`, `z=-1`, under the text) and removed by the clear (`a=d,d=a`).
  Inside tmux each image goes through the passthrough (`ESC P tmux; … ESC \`, ESC doubled) with a 2×1 virtual
  placement (`a=T,U=1`), and the slot is two Unicode placeholders (`U+10EEEE` + row/column diacritics, the id in the
  foreground: 256-colour index, or 24-bit plus a top-byte diacritic). tmux keeps, redraws and clears them as text, so
  no placement or delete goes out. A badge cannot sit on a placeholder, so it is drawn as text alone.
- Without images, ASCII right-aligned in the slot: subcommand ` >`, option ` -`, arg/special ` *`, file ` f`, folder
  ` d`, history ` h`, shortcut ` @`, auto-execute ` ↪`.
- Narrow emoji (`⚙️`) and remote image URLs use the row type's icon: their width differs between terminals, which
  would tear the frame.
- Title = `display_name` or names joined `", "`. Highlight: prefix mode → for each name whose lowercase starts with
  the lowercase query, the first `q.chars().count()` chars get `matching-bg`; fuzzy mode → fuzzysort indexes of the best
  name. (No regex.)
- Args hint: for each arg with a name: `name` + (`...` if variadic); optional → `[x]`, mandatory → `<x>`; joined `" "`.
- Truncation by display width (`unicode-width`), overflow replaced by `…`.

Description row: `' '` + (`name: ` bold when the text comes from `current_arg`; just `name` when its description is
empty) + italic description (= selected.description, or `file`/`folder` for those types, else
`current_arg.description`); `No description` in `#8c8c8c` when empty; right-aligned spinner frame (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`,
80 ms) while any generator has been loading > 200 ms. Control characters in titles and descriptions are drawn as
spaces.

Arg-hint mode: one row `name: description`. No key interception (no items).

#### 7.4.3 Pop-out (SHOULD, done)

`toggleDescription` / `alwaysShowDescription`: a 30-column panel beside the list (right if `x0+w+30 <= C`, else left
if `x0 >= 30`, else not shown), `n + 1` rows from the list's top row, description wrapped by display width (`…` on
overflow); the description bar row is then not drawn (ponytail: nor the spinner).

### 7.5 Rendering

#### 7.5.1 Guards (draw nothing, keep only clears)

Renderer: model modes `ALT_SCREEN`, `ORIGIN` or `INSERT`; a non-full scroll region (DECSTBM tracked by the output
filter, reset by a resize or `ESC c`); cursor in pending-wrap state (`grid.cursor.input_needs_wrap`); terminal smaller
than 10×3. Wrapper (`suppress`): not at a prompt (`preexec`, no cycle); no current buffer (the cursor is off the
command line, §5.6); resize debounce or repaint pending; a DCS/SOS/PM/APC string or an over-long sequence open on the
terminal (a frame would land inside it); within 50 ms of a prompt redraw; PSReadLine ListView predictions shown for a
non-empty line (they own the rows below the prompt). Keys act on the menu and ghost only while none of these holds
(`drawable()`, §7.2.4; ponytail: the layout's no-room case on a 3-row terminal is not covered).

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
sgr(cell): "\x1b[0" + fg Named(Black..White)=30..37, Bright*=90..97, Dim* → normal + 2; Indexed(i) → 38;5;i;
           Spec(rgb) → 38;2;r;g;b (exact, no 256 downgrade — fixes ish); bg likewise 40..47/100..107/48;5/48;2;
           flags: BOLD 1, DIM 2, ITALIC 3, UNDERLINE 4, DOUBLE/CURLY/DOTTED/DASHED underline 4:2..4:5, INVERSE 7,
           HIDDEN 8, STRIKEOUT 9; underline colour 58;2/58;5
```

Hyperlinks (incl. tabsmith's model-only tags) are never emitted. Real hyperlinks under a restored span lose their link
(ish has the same limit). ponytail: a covered last-column cell loses the terminal's soft-wrap bit (only copy/paste
joining notices). A cursor in pending-wrap state is restored by re-printing the last cell (full repaint).

#### 7.5.3 Frame scheduling

- Every PTY batch: `take_clear()` first (restores whatever the last frame covered, computed from the model state at
  draw time, which equals the real screen at that moment), then the PTY bytes, then a fresh frame (§4.5).
- State-only changes (navigation, completion reply, ghost, spinner): `frame()` alone — it emits the cached clear plus
  the new frame in one write when the frame changed or disappeared, and nothing when it is identical (the render key
  is the frame bytes themselves; `take_clear` would defeat that and is used only before PTY bytes and at exit).

#### 7.5.4 Output filtering (port of ish `stdioProxy.ts:15-43`)

Strip from the bytes written to the real terminal (the model still sees them): `CSI ?9001h/l` (win32-input-mode),
kitty keyboard `CSI > … u`, `CSI < … u`, `CSI = … u`, `CSI ? u`, modifyOtherKeys `CSI > … m`. Only outside the
alternate screen (inside, apps such as neovim keep their keyboard protocol; tabsmith does not intercept keys there).
The filter also tracks the alternate screen (`?1049/?1047/?47`), DECSTBM and `ESC c` (reset), and holds an incomplete
trailing CSI/ESC sequence and UTF-8 char until the next batch. A DCS/SOS/PM/APC body (`ESC P`/`X`/`^`/`_` … ST, ended
by any ESC, CAN or SUB like the VT parser does) passes through uncarried (sixel can be megabytes) while frames are
suppressed. An unfinished sequence longer than 4096 bytes is passed through instead of carried (no quadratic
rescans; ponytail: no keyboard-protocol upgrade is that long, so it is never stripped).

### 7.6 Resize, alternate screen

- Resize: `master.resize`, `term.resize` (alacritty reflows), scroll region reset, drop the cached clear (the terminal
  reflowed the old frame unpredictably), menu/ghost hidden, arm 150 ms debounce (re-armed by PTY data meanwhile). When
  it fires and not on the alternate screen: full repaint `?2026h ?25l` + for every model row `CUP + SGR0 + EL 2` (erases
  the line in the default pen without touching the scrollback) + its cells up to the last non-blank, + cursor/pen
  restore + `?2026l`, then a normal frame. (`CSI 2J` pushed one copy of the screen into the scrollback per repaint on
  alacritty, VTE, kitty, …; EL 0 after a full row is ignored in pending-wrap state by some terminals and erases the
  last char on others.) On the alternate screen: postpone until it is left. A prompt redraw that the shell does on
  SIGWINCH keeps the menu (§7.1). Unix: SIGWINCH → `crossterm::terminal::size()`; Windows: `WINDOW_BUFFER_SIZE_EVENT`
  + 250 ms polling.
- Alternate screen (`CSI ?1049h/?1047h/?47h`): the clear runs before the batch that switches (always), no frames while
  `ALT_SCREEN`, keys never intercepted, menu state error-reset on leave.

### 7.7 Themes

Eight colours: `main-bg`, `main-text`, `matching-bg`, `selected-bg`, `selected-text`, `selected-matching-bg`,
`description-text`, `description-border` (unused in v1).

- Built-in dark: bg `rgb(48,48,48)`, text `rgb(180,180,180)`, match bg `rgb(95,89,56)`, selected text
  `rgb(253,253,253)`, selected bg `rgb(30,90,199)`, selected match bg `rgb(106,142,218)`, description text
  `rgb(180,180,180)`, border `rgb(65,65,65)`.
- Built-in light: bg `rgb(254,254,254)`, text `rgb(7,7,7)`, match bg `rgb(255,239,152)`, selected text
  `rgb(253,253,253)`, selected bg `rgb(41,105,218)`, selected match bg `rgb(106,142,218)`, description text
  `rgb(7,7,7)`, border `rgb(199,199,199)`.
- `system`: OSC 11 reply (asked at every start, §4.4, so switching to `system` in a running session works) luminance
  `0.299r+0.587g+0.114b < 0.5` → dark, else light; no reply → dark. A theme change applies live (settings poll).
- Custom file: `version:"1.0"` + `theme{}` → explicit colours from `theme.backgroundColor`, `textColor`,
  `matchBackgroundColor`, `selection.{backgroundColor, textColor, matchBackgroundColor}`,
  `description.{textColor, borderColor}` (missing ones from dark); else base16: main-bg=shade0,
  matching-bg=shade1, selected-bg=shade2, description-text=shade5, main-text=shade6, selected-text=shade7,
  selected-matching-bg=accent6, description-border=shade1; missing shades/accents from the dark filler (`#181818
  #282828 #383838 #585858 #b8b8b8 #d8d8d8 #e8e8e8 #f8f8f8`; accents `#ab4642 #dc9656 #f7ca88 #a1b56c #86c1b9 #7cafc2
  #ba8baf #a16946`). Any error → dark.
- Colour parsing: `#rgb`, `#rrggbb`, `#rrggbbaa` (alpha premultiplied against black), `rgb(r, g, b)` with optional
  spaces.
- Output: truecolor SGR if `COLORTERM ∈ {truecolor, 24bit}` or `WT_SESSION` set; else nearest xterm-256 index (6×6×6
  cube + 24 greys).

---

## 8. Inline ghost text

### 8.1 When shown

`inline.enabled`; at a prompt with a current buffer (§5.6; not `preexec`, not alt screen); buffer non-empty; cursor
at the end of the buffer; ghost text non-empty and free of control characters (D32); **the shell is not already showing
its own suggestion** (any suggestion-coloured cell of the cycle at/after the cursor, §5.1.3 — covers
zsh-autosuggestions, fish autosuggestions, PSReadLine predictions); the render guards (§7.5.1). A ghost stays while the
user types characters that match its start (the rest is drawn), from the engine (local) or the LLM reply. The dropdown
and ghost text can be visible together.

### 8.2 Rendering

Drawn in the frame (§7.5) at `(cy, cx)` with `ESC[0m ESC[90m` (bright black, the zsh-autosuggestions `fg=8`
convention), clipped at the first non-blank model cell to the right (an RPROMPT) and at the right edge (never wraps);
leading zero-width chars are skipped (they would combine with the cell left of the span).
Restored like any other span. The shell's echo of the next typed char overwrites the first ghost cell; the next frame
redraws the shortened ghost.

### 8.3 Accept

| key | effect (only when the ghost is shown and the cursor is at the buffer end, so the shell would no-op anyway) |
|---|---|
| `right`, `control+f`, `end`, `control+e` | write the whole ghost text to the PTY; clear ghost |
| `alt+f` (`ESC f`), `control+right` | partial accept: leading whitespace + the next run of non-whitespace chars |
| `tab` | **not** a ghost key (dropdown `insertCommonPrefix` when the menu intercepts, else the shell's own completion) |
| anything else | forwarded; ghost recomputed from the next reply |

The text written is the ghost's rest after the typed-ahead keys (`basis()`, §7.3), not after the last batch, so `l` +
Right in one read does not type `l` twice; the accepted text joins the typed-ahead model. A ghost key is held like a
menu key after an unmodelled key (D40). Accepting does not count as an insertion for the visibility machine (it is
ordinary typing to the shell).

### 8.4 Request flow

Local sources arrive in every `Completion` (§6.11). LLM (when `inline.enabled`, `inline.source` is a valid profile,
and a phase-Parse reply has no local ghost with the cursor at the end of the buffer — error replies too: commands
without a spec get LLM ghosts, since the inline completion depends on the buffer only): main sets
`llm_deadline = last buffer change + inline.debounceMs`; a new request clears it; on expiry send
`EngineMsg::Llm{req, buffer, cwd, shell}` for the last sent buffer. Engine: cache lookup (below) → immediate
`Msg::Ghost`; else spawn an LLM worker (5 s timeout) with the `llm_history` snapshot: the last `inline.historyCount`
usable entries (exit code known, privacy filter), oldest → newest. The worker replies `Msg::Ghost{req, text}`; main
keeps it only if `req` is both the latest request and the LLM request.

Cache: `Vec<(full_text, priority)>`, cleared on
`CommandFinished`/`CommandEnded` (ponytail: linear scan of ≤ 50 entries instead of a radix trie). Lookup: entries where
`full.starts_with(buffer) && full != buffer`, minimum priority (first inserted wins ties) → `full[buffer.len()..]`. The
first completion of a response is stored with priority 0.0, others 1.0. This makes a suggestion survive the user typing
its first characters.

### 8.5 LLM providers

Prompt: the last `inline.historyCount` (49; 0 = none)
usable history commands (non-null exit code, privacy filter below), oldest → newest, then the buffer, numbered like `history`:
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
POST {base_url}/v1/messages                     (a trailing `/` of base_url is dropped, for both providers)
x-api-key: <key>                     (omitted when the key is empty)
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

Cleaning: take the first non-empty line that is not a code fence; strip surrounding backticks/quotes (only when that
quote does not also appear inside); strip a leading history number (`\d+` then two spaces, leading whitespace
allowed), then the buffer if the model echoed it (in that order, so `    3  git status` loses both); `trim_end`;
empty, or any control character left (a bare `\r` survives `lines()`, D32) → none.

Validation: `full = buffer + completion`; if `full.trim()` splits (shell words) into
exactly `["cd", arg]`: expand `~`/`$VAR` from the session env (unknown var → accept); absolute non-directory → reject;
else `cwd.join(path)` must canonicalize to a directory. Everything else is valid.

---

## 9. Specs and the JS host

### 9.1 Bundling (D11)

- `scripts/update-specs.sh [version]` (repo tooling, run by maintainers): download
  `https://registry.npmjs.org/@tabsmith/autocomplete-specs/-/autocomplete-specs-<ver>.tgz`, then
  `COPYFILE_DISABLE=1 tar --no-xattrs -C package/build -cf - . | zstd -19 -o assets/specs.tar.zst`, copy
  `package/LICENSE` to `assets/SPECS_LICENSE`, write `<ver>` to `assets/SPECS_VERSION`.
- `include_bytes!` (cargo feature `embed-specs`, default on; ~4 MB). Every engine start (and `setup`, the doctor fix):
  if `~/.tabsmith/specs/<ver>/index.json` is missing, decompress (`zstd::Decoder` + `tar::Archive`; regular files only,
  macOS AppleDouble `._*` entries skipped) into `specs/<ver>.tmp-<pid>/`, then rename to `specs/<ver>/` (atomic;
  concurrent extractors: the loser deletes its temp dir; a leftover `<ver>/` without an index is replaced). 77 MB on
  disk. Then touch `specs/<ver>/.last-used` and clean up.
- Cleanup policy: another version dir is removed only when its `.last-used` (else the dir mtime) is older than 7 days —
  sessions of an older binary still read their version (ponytail: a session running a week without restart may lose
  it); a `<ver>.tmp-<pid>` dir is removed after 10 min (an extraction that died); only dirs whose name is a numeric
  `1.2.3` version (± `.tmp-…`) are candidates: `user/` and anything else the user keeps there is never touched.
- Index: `index.json` (`completions` 1 484 entries, `diffVersionedCompletions` 6) of the embedded set ∪ `*.js` stems
  present in `specs/user/`.
- Licences: `assets/SPECS_LICENSE` (MIT, Hercules Labs/Fig) is shipped; THIRD_PARTY notices for inlined semver, yaml,
  strip-json-comments, `@fig/autocomplete-*` are not yet.
- SHOULD (post-v1): `tabsmith specs update` downloads a newer tarball into `specs/user/`.

### 9.2 JS host (engine thread, rquickjs 0.14)

Runtime setup: `Runtime::new()`, `set_memory_limit(512 MiB)`, `set_interrupt_handler` returning true once
`now > deadline` (an `Option<Instant>` armed around every eval/call/job and cleared after), `Context::full(&rt)`. One
long-lived context: modules evaluated once and their serialized namespaces cached by path together with the file's
(mtime, size); an edited file is evaluated again (developer mode, F38; old function ids stay valid). Specs keep
module-level caches (e.g. `man.js`).

Module loading without a module loader (every bundled file ends with exactly one `export{…};`, no imports):

```text
src = read file; if src.trim().is_empty() → namespace {}
at = last "export" followed by "{"; parse its clause "A as default, B as versions, C" → pairs (local, exported)
script = "__ts.json((function(){" + src[..at] + "\n;return {" + pairs.map(|(l,e)| e + ":" + l).join(",") + "};})())"
fallback (hand-written dev/user/local spec, the Fig docs form `export default spec;`): at = last "export default";
   script = "__ts.json((function(){" + src[..at] with a leading "export " dropped per line +
            "\nvar __ts_default=" + src[at+14..] + "\n;return {default:__ts_default};})())"
            (ponytail: only `default` is exported this way; named exports such as `versions` need the esbuild form)
neither → error "no trailing export clause"
ns_json = ctx.eval_with_options(script, filename)   (5 s deadline)  → serde_json::Map → normalise `default` (§6.2)
```

Host functions registered by Rust: `__ts_log(level, msg)` (debug log), `__ts_exec(taskId, inputJson) → opId`,
`__ts_sleep(ms) → opId`, `__ts_done(task, ok, payload)`. JS keeps the resolvers of its pending host operations in its
own table (`__ops`) and Rust settles them with `__ts.settle(opId, ok, payload)`, so no JS handle ever lives on the Rust
side (no `Persistent`, nothing can outlive the runtime). Prelude (evaluated once, verbatim from `src/js.rs`):

```js
globalThis.console = Object.fromEntries(["log","info","debug","warn","error"].map(l => [l, (...a) => __ts_log(l, a.map(String).join(" "))]));
let __tid = 0; const __live = new Set(); const __ops = new Map();
globalThis.setTimeout = (fn, ms = 0, ...args) => { const id = ++__tid; __live.add(id);
  __ts.op(__ts_sleep(+ms || 0)).then(() => { if (__live.delete(id)) fn(...args); }); return id; };
globalThis.clearTimeout = id => { __live.delete(id); };
globalThis.Intl ??= { NumberFormat: class { constructor(_l, o = {}) { this.o = o; }
  format(n) { if (this.o.notation !== "compact") return String(n);
    const u = [[1e12,"T"],[1e9,"B"],[1e6,"M"],[1e3,"K"]].find(([v]) => Math.abs(n) >= v);
    return u ? +(n / u[0]).toPrecision(this.o.maximumSignificantDigits ?? 3) + u[1] : String(n); } } };
globalThis.__ts = {
  fns: [], ids: new Map(),
  ser(v, seen = new WeakSet(), d = 0) {                 // functions → {"$fn": id}; drops undefined/symbol; cycle-safe
    if (typeof v === "function") { let id = this.ids.get(v);
      if (id === undefined) this.ids.set(v, id = this.fns.push(v) - 1); return { $fn: id }; }
    if (v === null || typeof v !== "object") return typeof v === "bigint" ? String(v) : v;
    if (v instanceof RegExp) return String(v);
    if (seen.has(v) || d > 200) return null; seen.add(v);
    const out = Array.isArray(v) ? v.map(x => this.ser(x, seen, d + 1))
      : Object.fromEntries(Object.entries(v).filter(([, x]) => x !== undefined && typeof x !== "symbol")
                                            .map(([k, x]) => [k, this.ser(x, seen, d + 1)]));
    seen.delete(v); return out;                         // shared (non-cyclic) refs are duplicated, cycles → null
  },
  json(v) { return JSON.stringify(this.ser(v) ?? null); },
  op(id) { return new Promise((res, rej) => __ops.set(id, [res, rej])); },
  settle(id, ok, v) { const p = __ops.get(id); if (p) { __ops.delete(id); ok ? p[0](v) : p[1](new Error(v)); } },
  mkExec(ctx) {
    const run = input => __ts.op(__ts_exec(ctx.id, JSON.stringify(input))).then(JSON.parse);
    return input => typeof input === "string"
      ? run({ command: "sh", args: ["-c", input], cwd: ctx.cwd }).then(r => r.stdout)
      : run({ cwd: ctx.cwd, ...input });
  },
  call(id, argsJson, ctxJson) {                          // every spec callback goes through here
    const ctx = JSON.parse(ctxJson), args = JSON.parse(argsJson);
    for (const i of ctx.exec) args[i] = this.mkExec(ctx);
    return Promise.resolve().then(() => this.fns[id](...args)).then(r => this.json(r));
  },
  start(task, id, argsJson, ctxJson) {
    __ts.call(id, argsJson, ctxJson).then(r => __ts_done(task, true, r), e => __ts_done(task, false, String(e)));
  },
};
```

A callback task: `start_call(fnId, args, ctx)` registers a task id with its `GenCtx` (cwd, env, shell, search term) and
calls `__ts.start(task, fnId, argsJson, ctxJson)`; argument positions listed in `ctx.exec` receive an
`executeShellCommand` bound to that context (a string runs as `sh -c` and resolves to stdout). Functions are numbered
by identity (the same function always gets the same id), so the table grows with distinct functions only (not
collected, ponytail). Finished results are delivered once.

Callback signatures the engine uses: `custom(tokens, exec, generatorContext)`,
`postProcess(stdout, tokens)`, `script(tokens)`, `trigger(newSearchTerm, oldSearchTerm)`, `getQueryTerm(token)`,
`generateSpec(tokens, exec)`, `loadSpec(token, exec)`, `parserDirectives.alias(token, exec)`,
`getVersionCommand(exec)`, diff-versioned `default(version)`, `filterTemplateSuggestions(suggestions)`,
`generateSpecCacheKey({tokens})`. Results are JSON (functions inside generated specs get ids too). History parsing
never runs JS (F6 is SKIP).

Event pump (manual Promise resolution, no async runtime):

```text
wait(tasks, deadline):                                   // Err(Aborted) as soon as a newer Complete is queued
  loop {
    pump(deadline): fire due timers (settle their sleep ops / finish sleep tasks);
        stop = max(deadline, now + 50 ms)                // the scheduling slice
        while now < stop: arm the interrupt at max(stop, now + 1 s); run one pending job (none → break)
        // each job gets ≥ 1 s of CPU: a long postProcess (brew's 16 k formulae) is never killed by the slice
    all tasks done (or unknown) or now >= deadline → return one entry per task (None = still pending)
    msg = engine_rx.recv_timeout(min(deadline, next timer) - now)
    ExecDone{id,out} → engine task: store the result; JS op: __ts.settle(id, ok, json)
    other            → deferred for the engine loop (a Complete aborts this wait; pending tasks survive the abort)
  }
__ts_exec(task, input): new op id; run §9.3 on a worker thread, which sends EngineMsg::ExecDone{op id}
```

The engine's generator runs wait in 50 ms slices (finished generators are replied together per slice); a callback that
is interrupted fails its task only when its run deadline passes (§6.6.1).

### 9.3 `exec` semantics

```text
input {command, args[], cwd?, env?: {k: v|null}, timeout?}
cwd   = the first existing directory of input.cwd, generator cwd (shell cwd); else the wrapper's cwd; else "/" / "C:\"
short-circuits:
  command == "ls" && args.map(trim) == ["-1ApL"] → native listing of input.cwd joined onto the shell cwd (or the
       cwd above when input.cwd is empty) — never the fallback: a missing dir → {stdout:"", stderr, status:2} (D34);
       read_dir following symlinks, dotfiles included except . and .., dirs get "/", one per line, unsorted
  command ∈ {fig, tabsmith} → {stdout:"", stderr:"", status:1}   (bundled ai() helper)
env   = session env if known (env_clear + session env) else the wrapper's env; remove LS_COLORS CLICOLOR_FORCE
        CLICOLOR COLORTERM; set PROCESS_LAUNCHED_BY_TABSMITH=1 HISTFILE="" HISTCONTROL=ignoreboth
        TERM=xterm-256color NO_COLOR=1; apply input.env (null → remove); a cmd.exe child also gets
        NoDefaultCurrentDirectoryInExePath=1 (cmd would run a repo's `npm.cmd` from the cwd before PATH)
exe   = absolute → as is; a name with a separator → relative to cwd; else search the child env's PATH (Windows: also
        each PATHEXT ext, case-insensitive variable names); not found → reject "failed running executable (<cmd>): not found"
        Windows: `sh -c <s>` with no sh on PATH → Git Bash `bash.exe -c` (next to git.exe, else Program Files /
        LOCALAPPDATA), else `%ComSpec% /C` with the raw string
spawn = no shell; stdin null; Unix pre_exec libc::setsid() (own session/process group); Windows CREATE_NO_WINDOW and
        a job object (assigned right after spawn; ponytail: a process started in that instant escapes)
timeout = max(autocomplete.scriptTimeout (5000; Windows 20000), input.timeout); script generators put
          max(scriptTimeout, g.scriptTimeout, input.timeout) into input.timeout (§6.6.3);
          poll try_wait every 5 ms while reader threads drain both pipes;
          on expiry kill the whole group (Unix kill(-pgid, SIGKILL), Windows TerminateJobObject) and the child
          → reject "timed out after N ms running executable (<cmd>)"
output  = lossy UTF-8; clean: \r\n→\n, remove "\x1b[?25h", strip leading and trailing \n+
status  = exit code; killed by signal → 1;  non-zero status resolves normally (specs check it)
```

Every running exec child is registered (Unix process group, Windows job handle) and killed at session end:
`cleanup`, the fallback path and, on Unix, an `atexit` hook — no generator outlives the session (a hanging one would
run forever, reparented to init).

`bash -c …` appears 138× in 72 spec files: on Windows these work only when Git Bash's `bash` is on PATH; otherwise
those generators yield `[]` (risk, §11).

---

## 10. Test plan

`cargo test` (unit tests next to the code, `#[cfg(test)]`; all OSes in CI). Every finding fixed in the four review
rounds has a regression test next to the code it fixed. Tests stay in the same file under `#[cfg(test)] mod tests`, and
a test module over about 500 lines usually goes to a sibling `<file>/tests.rs` (a few shorter ones do too, and
`architecture_tests.rs`, a test-only file, keeps its `mod tests` inline). Module paths in the table are relative to `src/modules/` (`bootstrap/` and `architecture_tests.rs` to `src/`); a
directory followed by file names stands for those files in it. `tests/` at the crate root holds data only
(`tests/fixtures/`, `tests/snapshots/`). Test-only support lives next to what it serves:
`shared/infrastructure/testing/fixture_log.rs` (the comparison of a log with its frozen file, `P9_WRITE_FIXTURES`) and
`shared/infrastructure/testing/temp_dir.rs` (the temp-dir helper of the file-system tests),
`shell_integration/infrastructure/testing/integration_fixtures.rs`, and the `test_fixtures.rs` files of
`completion/domain`, `completion/application`, `diagnostics/application` and `terminal_session/application` (the
helpers and use-case harnesses that the tests below share).

| where | tests |
|---|---|
| `completion/domain/services/command_line_tokenizer/tests.rs` | Every row of the §6.1.3 table; operators without spaces; unterminated quotes (`complete=false`, inner text); escapes in word vs string vs raw string; concatenation `--foo="a b"c`; `$(…)` nesting; `FOO= cmd`; alias cases from §6.1.6; alias with `\|` not expanded; parse-tree fixtures (`tests/fixtures/shell_parser/`). |
| `completion/application/command_line_parser/tests.rs` | Mock spec (the §6.4 table and the full mock-spec table, as JSON fixtures in `tests/fixtures/parser/cmd_spec.json`); `isRepeatable` limits; persistent option inheritance without mutation; `--` end of options; `loadSpec` locations recursion with `command_index` (`sudo git ` → git spec, generators see `["git",…]`). |
| `specs/infrastructure/adapters/fig_spec_mapper/tests.rs` | The JSON fixtures of `tests/fixtures/parser/` read into the spec model: the full mock-spec table; `exclusiveOn`/`dependsOn`. |
| `specs/domain/services/`: `spec_locator.rs`, `spec_version_selector.rs`; `specs/infrastructure/adapters/fig_version_diff.rs` | `spec_location` table; diff merge (`merge_named` order, `remove`, `merge_args` index quirk) on hand-made fixtures; version pick (newest ≤ target, none → newest). |
| `completion/domain/services/`: `fuzzy_matcher.rs`, `suggestion_filter.rs` | fuzzysort examples (`fs`/`Fuzzy Search` → −16, `test`/`test` → 0, non-match → None); prefix score ladder 0/−1/−2/−3; bucket order A ++ B ++ C; auto-execute rules (dangerous, optional args, hidden setting, onlyShowOnTab); `.` and `/`-folder auto-execute. |
| `completion/domain/services/suggestion_ranker.rs` | dedupe ≤ 50; recency boost bands (`75 + ms/1e13`), `priority: 0` → 50, alphabetical setting. |
| `completion/domain/services/`: `suggestion_assembler.rs`, `history_suggestions.rs` | option-chain rewriting (`-la`); history rows priority. |
| `completion/domain/services/generator_trigger_policy.rs`; `completion/domain/entities/generator_cache.rs` | triggers (string/fn/threshold/match/change); cache TTL/SWR/max-age. |
| `completion/domain/services/insertion_composer.rs` | insertion table from §6.8 (incl. POSIX and pwsh escaping, `{cursor}`, auto-space, raw-token deletion counts with quotes and escaped spaces). |
| `dropdown/domain/entities/menu/tests.rs`; `dropdown/domain/services/common_prefix.rs` | Visibility transitions for: typing, space/new arg, full insertion with and without new arg, generator re-trigger, Esc → stays hidden across tokens, backspace into previous token, large change (paste) vs `just_inserted`, `onlyShowOnTab`, `navigateToHistory`, Up at 0, wrap-around, sticky selection across recompute; common prefix (single item, mixed types, `../`, no prefix → BEL). |
| `dropdown/domain/services/keystroke_parser.rs`; `dropdown/domain/value_objects/keymap.rs`; `terminal_session/domain/entities/key_decoder.rs` | keybinding parser (`ctrl+k`, `control+K`, `shift+tab`, `shift+a`→`A`, `+`, `ctrl++`, `f12`, unknown modifier ignored, multi-char unknown → dropped); `"ignore"`; stdin decoding table §7.2.1 incl. a chunk with several keys and a bracketed paste. |
| `dropdown/domain/services/`: `menu_layout_calculator.rs`, `text_fitting.rs`, `color_parser.rs` | Layout: below/above/shrink/hide decisions at various `cy`, `R`; `x0` clamping; truncation with wide chars; colour parsing (`#rgb`, `#rrggbbaa`, `rgb( 1, 2, 3 )`); 256-colour mapping. |
| `dropdown/domain/services/theme_resolver.rs`; `dropdown/infrastructure/adapters/`: `theme_json_mapper.rs`, `file_theme_repository.rs` | Themes (§7.7): `dark`, `light` and `system` resolve without asking the repository and an empty name is dark; `system` follows the luma of the terminal background (no reply is dark); a custom theme resolves through its palette, a missing one to dark, and one named like a built-in to the built-in. A Fig theme (`version` + `theme{}`) maps to the eight roles and any other JSON value is drawn with the base16 filler, a missing or unparsable colour falling back per colour; a theme file's author is read as an object or a three-item array, and a file that is not an object, a bad author, version or theme, a duplicate author and broken JSON are reported with the serde text, the first colour key that is not a string by name; unknown keys, another version and unparsable colours are accepted. The repository finds a valid theme file and nothing for a missing, unreadable or non-UTF-8 file, a directory named like a theme or no home, reports an invalid file with its path, lists the JSON stems sorted and deduped, never creates the themes directory, joins the name unsanitised and reads the file again on every call. |
| `terminal_session/infrastructure/adapters/overlay_renderer/tests.rs` | **round-trip invariant**: model A ← `output`; model B ← `output + frame + clear`; visible grids (chars, fg, bg, flags) equal — for below, above, ghost, wide chars at both span edges, truecolor cells. |
| `terminal_session/domain/entities/marker_splitter.rs` | OSC splitter across chunk boundaries (split inside `ESC ]`, inside payload, BEL vs ST, non-697 OSC passthrough, 64 KiB flush). |
| `terminal_session/domain/entities/marker_tracker/tests.rs`; `terminal_session/infrastructure/adapters/alacritty_terminal_screen/tests.rs` | markers stripped from output; lock/unlock with foreign sid; `NewCmd=<other sid>` ignored; edit-buffer extraction by feeding a real `alacritty_terminal::Term` with byte transcripts: zsh prompt + command, RPROMPT, PS2 multi-line (real `\n`), wrapped long command, zsh completion listing below (excluded), ghost text in `fg=8` after cursor (excluded) vs typed `fg=8` before cursor (kept), prompt scrolled by output, resize reflow, Windows deferral FIFO. |
| `terminal_session/domain/services/insertion_reconciler.rs` | reconciliation byte strings (`cur` ahead / behind `ib`). |
| `terminal_session/infrastructure/adapters/`: `cell_tagger.rs`, `edit_buffer_extractor.rs` | Cell tags on a real `alacritty_terminal::Term` (§5.6): cells written after a tag command are tagged and a clear command stops it; on mark-line-start the untagged cells before the cursor become the prompt and the redrawn cycle cells are tagged again from the cursor; wrapped rows are followed only when a redrawn cycle is given. Suggestion-colour matching of a cell: a cell with only one of the flags of a reported pwsh prediction colour is typed text; a dim or italic cell is the shell suggestion even without a reported prediction colour; named colours above fifteen map to the default colour; indexed and truecolour cells match like the parsed colour; the pending-wrap cursor counts as past the last column. |
| `terminal_session/infrastructure/adapters/output_filter.rs` | Output filtering (§7.5.4): the keyboard-protocol upgrades (`CSI ?9001h/l`, kitty `CSI … u`, modifyOtherKeys `CSI > … m`) are stripped and other CSI and ESC sequences pass unchanged; a sequence split across batches is held, then stripped or emitted whole; inside the alternate screen the keyboard protocol is kept; DCS and APC strings are tracked across batches (closed by ST, aborted by CAN, a bare LF in the body not rewritten); an overlong sequence is passed through with a bounded carry; the scroll region and a UTF-8 char split across reads are tracked; a bare LF becomes `ESC D` only when asked. |
| `terminal_session/application/handle_keystrokes_use_case/tests.rs` | The key path over the in-memory ports of the harness: keys are forwarded untouched when there is no command line; the shell write comes only after the frame was drawn and the bell rings before the frame and the write; a read that arrives behind held keys is not decoded and the replayed held keys are decoded again; a key that would act is held while the echo is pending; a split paste, also after a hold reset the decoder, and an unbracketed paste are never intercepted; a menu hidden by the render guards does not intercept and a disabled menu lets Tab through; `Alt+f` accepts the next ghost word and `Right` the whole ghost; Enter sets the submitted line only at the prompt and clears the menu before the line runs; insertions build on keys not yet echoed and are recorded in the engine after the lock is set, nothing when the insertion is dropped while a command runs; the history and fuzzy-search toggles ask the engine again before redrawing; pwsh's list view owns the rows below. |
| `terminal_session/domain/entities/terminal_session/tests.rs` | The session aggregate with the instant passed in: the shell the tracker saw wins over the wrapper's; the late-probe window stays open until a cursor report closes it; the nearest deadline wakes the loop, each source of a deadline on its own, and only the exit in a pass-through; a changed line is asked for with the next request id and an unchanged one is not asked again, a blank line counts as a request and asks nothing; no command line on the alternate screen or with the cursor off the line; an insertion stays in the basis while the shell echoes nothing and is released at the cap; no action on an empty line before the prompt-redraw grace is over; the menu and the unchanged line survive a prompt redraw; the end of a command makes the same line be asked again; no file poll for a command that starts; the LLM request is scheduled for an error reply with the cursor at the end and not mid-word, and taken once its debounce ran out; a reply for an older request is dropped; the overlay is drawable on a plain line and not under a guard, and held back over a pwsh list view only for a non-empty line; the LLM ghost text of the current request goes behind the local one; Enter waits behind an unmodelled backspace until the reply came; the spinner ticks only while a reply is loading; the files are polled once the interval passed and the next poll armed; the exit is reported once the child exited and the output ended. |
| `terminal_session/infrastructure/pty/session_runner/tests.rs` | The session event loop without a real PTY (a channel-backed `PtyWrappedShell` over `RecordingMasterPty`): a panic turns the session into a pass-through; a late probe reply is not typed; queued PTY chunks are written as one batch. |
| `shell_integration/infrastructure/adapters/embedded_shell_scripts/tests.rs` | `init` snapshots for every (shell, when) under `TABSMITH_INIT_SNAPSHOT_TEST=1` (`insta`-free: compare to files in `tests/snapshots/`, `TABSMITH_BLESS=1` rewrites them); syntax-check snapshots when tools exist: `bash -n` + shellcheck, `zsh -n`, `fish --no-execute`, `pwsh -NoProfile -Command [ScriptBlock]::Create(...)` (the syntax checks and the temp-HOME paths are in `shell_integration/infrastructure/testing/integration_fixtures.rs`); the post scripts run in real bash/zsh/fish (markers, hook order, bash-preexec layout, injected-integration reload with fake scripts). |
| `shell_integration/infrastructure/adapters/file_dotfile_repository/tests.rs`; `shell_integration/application/integration_installer/tests.rs`; `shell_integration/domain/value_objects/dotfile.rs` | dotfile install into a temp HOME: idempotent (install twice → same bytes), shebang kept first, `# tabsmith ignore`, pre-not-first / post-not-last detection, uninstall restores original content (modulo trailing newline), backup created. The first file holds the disk scenarios; the second runs the same rules over in-memory fakes. |
| `shell_integration/domain/services/launch_policy.rs` | should-launch decision table from env maps. |
| `settings/infrastructure/adapters/setting_value_json.rs`; `settings/infrastructure/cli/settings_command.rs` | get/set JSON-or-string parsing, missing key plain vs json. |
| `settings/domain/value_objects/setting_key_pattern.rs`; `settings/application/delete_settings_use_case.rs` | delete wildcard (`autocomplete.*`, `?`). |
| `settings/infrastructure/adapters/json_file_settings_repository.rs`; `terminal_session/domain/entities/reload_watch.rs`; `terminal_session/application/session_file_poller.rs` | sorted pretty output, corruption handling, mtime reload. |
| `shared/infrastructure/atomic_file.rs`; `shared/infrastructure/json_document_store.rs` | Atomic writes and the JSON document store (§3.2): a private write creates a `run/<sid>.json` file with mode 0600 in a 0700 directory and fails on a purged data root without recreating it, or on a path without a parent, reporting the bare I/O error; a dotfile is replaced atomically (whoever holds the old file sees it whole, a symlinked dotfile stays a symlink, a dangling relative symlink stays and its target is created, permissions stay, no temp file is left, a read-only file still reports PermissionDenied); a save keeps a symlinked file. The store loads a missing or blank file as an empty document, reports invalid JSON or JSON that is not an object as corrupted (with the parser cause), a file it cannot read with the path it was reading and a failed save as writing the path it was given. |
| `history/infrastructure/adapters/jsonl_history_repository.rs` | History file (§3.5): every field is written in order with null for absent values; a line missing the command, shell, session id or start time is dropped, missing optional fields load as none and unknown fields are ignored; a missing data root is not recreated (the entry stays in memory) and a blank command is not written; mode 0600 on Unix; a file of 10 000 lines (one corrupt) is not rewritten at load, the append that goes past them rewrites it with the last 5 000, and a file already past 10 000 lines is compacted at load. |
| `ghost_text/domain/entities/ghost_text_cache.rs`; `ghost_text/domain/services/`: `llm_prompt_builder.rs`, `llm_completion_cleaner.rs` | Cache test vector (`{"echo hello, world":0.5,…}` → `"e"`→`echo hello, world`, `"ls -a"`→`ls -al`, `"other"`→None); history prompt numbering + 10 240-byte truncation + privacy filter; cleaning (fences, echoed buffer, numbering). |
| `ghost_text/application/llm_ghost_text_job.rs` | `cd` validation. |
| `ghost_text/infrastructure/adapters/http_llm_client.rs` | request bodies for both providers serialized exactly as §8.5 (no `temperature`; `output_config` only with effort); response parsing incl. refusal and thinking-before-text. |
| `specs/infrastructure/adapters/quickjs_spec_runtime/tests.rs` (`all_bundle_modules` is `#[ignore]`, run in CI with `--release -- --ignored`; it is compiled with the default `embed-specs` feature) | Load every module of the bundle: each yields a namespace or the documented empties, none fails (the 1 484 entries of `index.json` are counted by `embedded_index_shape` in `specs/infrastructure/adapters/embedded_spec_bundle.rs`); `git` alias directive via a fake exec; `setTimeout` ordering; `Intl.NumberFormat` compact; interrupt handler stops `while(true){}`; legacy string exec. |
| `specs/infrastructure/adapters/process_executor.rs`; `completion/infrastructure/adapters/fs_directory_lister.rs` | `filepaths()` through the native `ls -1ApL` path. |
| `bootstrap/engine_server/tests.rs`; `bootstrap/engine_inbox.rs`; `bootstrap/channel_completion_engine.rs` | The completion engine thread (§6, §9.2), where the use cases of several modules are chained. `EngineServer` runs over a real `EngineInbox`, QuickJS runtime and spec repository on the test thread, with a watcher thread that reads the replies and feeds the inbox: a script generator, a `postProcess` function and a templated custom generator each reply again with their rows, a fresh cached custom result is served in the parse reply, bad script input settles with no rows and the rows of a superseded debounced run stay out of the slot; after the inbox is drained only the latest request is served, settings queued after a request are applied before it is served, and a request that arrives with shutdown is dropped; the local ghost text is added to a generator's reply, a history ghost text to the error reply of a command without a spec, and none for a cursor inside a word; an LLM request is answered at once with no ghost text for a source that is not a profile or a provider that cannot be reached; a session update refreshes the first-token items and keeps the engine running. The inbox: a wait aborts on a newer `Complete` and defers the other messages, and a runtime event the engine loop reads reaches the runtime. `ChannelCompletionEngine`: each call of the port is sent as its `EngineMessage`, in order, and an engine that has stopped is ignored. |
| `bootstrap/bridges.rs`; `bootstrap/diagnostics_bridges.rs` | The bridges that implement the port of one module with the use cases of another: a failed extraction of the spec bundle keeps its whole error chain (`could not extract the completion specs: creating <dir>: <io>`); the integration probe lists and checks integrations like the installer and reads the binary of each installed script (the text between its first two quotes; orphans, unreadable scripts, scripts without quotes and shells not found on `PATH` are skipped); the check fixer installs the integration it is asked to fix, creates the data directory, resets the settings keeping the old bytes named after the clock and installs the spec bundle, each failure with its error text, and marks the restart reminder after installing integrations (even when none was missing) but not after a failed install or after installing the spec bundle; the bundled-specs probe finds the bundle only once the index of this version exists. |
| `architecture_tests.rs` | Architecture fitness test: walks `src/modules/` and fails when a module listed in `REGISTERED_MODULES` breaks a layer rule, among them: `domain` using `application`, `infrastructure`, `bootstrap`, an external crate other than `unicode_width`, an output macro or the logger; `application` using `infrastructure`, `bootstrap`, an external crate other than `unicode_width` or an output macro; a module using another module's use cases or infrastructure; `shared` using another module; a `mod.rs` holding more than child declarations; a `src/modules/<module>.rs` file, or a file of the module other than `mod.rs` outside its `domain/`, `application/` and `infrastructure/` folders; `#[path]`, `include!` and exported macros; `main.rs` or `modules/mod.rs` holding more than module declarations (and `fn main`), or a `use` item. Test-only code (`#[cfg(test)]` items and sibling `tests.rs` files) is exempt from the domain, application and module-boundary rules only; the `shared`, `mod.rs` and splice rules (`#[path]`, `include!`, exported macros) still apply to it. The rules themselves are tested on source snippets in the same file. |
| the `application/` layer of every module, e.g. `settings/application/set_setting_use_case.rs`, `completion/application/complete_command_line_use_case.rs`, `diagnostics/application/run_doctor_use_case/tests.rs` | Use-case tests over in-memory fakes: a use case is built over in-memory repositories and recording, capturing or fixed doubles of its ports (`InMemorySettingsRepository`, `CapturingTerminalWriter`, `FixedClock`), which live under `#[cfg(test)]` next to the trait they stand in for (`settings/domain/repositories/settings_repository.rs`, `terminal_session/application/ports/terminal_writer.rs`, `shared/application/ports/clock.rs`); the test runs the use case and reads back what the doubles saw. |
| `terminal_session/application/session_use_cases/tests.rs`; the `probe`, `passthrough` and `batching` scenarios in `terminal_session/infrastructure/pty/session_runner/tests.rs`; the `tracker_*` scenarios in `terminal_session/infrastructure/adapters/alacritty_terminal_screen/tests.rs` | Characterisation scenarios, each with a log that must equal its frozen file in `tests/fixtures/terminal_session/characterisation/`. The 11 scenarios of `session_use_cases/tests.rs` are whole sessions: `TerminalSessionHarness` (`terminal_session/application/test_fixtures.rs`) wires the use cases over the real screen and in-memory doubles, as production wires them; each scenario (typing, bell, held keys, lifecycle, cleanup, ghost text, lock, resize, LLM ghost text, settings reload, spinner) logs step by step what reached the terminal, the shell and the engine. The `probe`, `passthrough` and `batching` scenarios drive a `SessionRunner` over the ports of that harness and log the same three. The three `tracker_*` scenarios (a bash cycle, the Windows deferral, zsh colours and a nested shell) use no harness, shell or engine: they feed scripted chunks to the real screen, splitter, marker tracker and shell-output feeder and log, per chunk, the filtered bytes for the terminal, the events, the cycle state, the extracted edit buffer and the hyperlink runs of the model (the cell tags). `shared/infrastructure/testing/fixture_log.rs` compares each log with its file: a missing file is created only with `P9_WRITE_FIXTURES=1` (the test still fails, so the file is read before it is frozen); an existing file is never rewritten. |
| `diagnostics/domain/services/doctor_check_catalog/tests.rs`; `diagnostics/application/`: `check_group_builder/tests.rs`, `check_evaluator/tests.rs`, `run_doctor_use_case/tests.rs`, `run_doctor_use_case/execute_tests.rs` | Doctor, check by check: a check is data and its outcome a pure function of facts, so each test hands the facts to `dotfile_outcome`, `data_directory_outcome`, `scripts_outcome`, `settings_outcome`, `specs_outcome`, `windows_console_outcome`, `terminal_outcome`, `session_outcome`, `inline_outcome`, `os_outcome`, `bash_outcome` or `fish_outcome` and reads the `CheckOutcome` (`Ok`, `Warn`, or `Err` with its reason, info lines and `CheckFix`); the same file tests which checks each group holds, in which order and with which `CheckSeverity` (`Normal`, `Soft`, `Skip`). The application layer is tested over in-memory probes: when a group is built and what it asks, what one check asks when it is evaluated, and the run (header before outcomes, fix and re-run, abort, summary, verdict). |
| `self_update/domain/`: `value_objects/` (`release_asset.rs`, `checksum_list.rs`, `install_location.rs`), `services/update_schedule.rs`, `self_update_error.rs`; `self_update/application/update_tabsmith_use_case.rs`; `self_update/infrastructure/adapters/`: `ring_sha256_digest.rs`, `ring_ed25519_checksum_signature.rs`, `file_installed_executable.rs`, `marker_file_update_check_repository.rs`; `self_update/infrastructure/cli/update_command.rs` | The native install's update (§2.10, D50–D52). Asset names per platform and none for a platform without a build; `SHA256SUMS` in text and binary mode; `~/.local/bin/tabsmith[.exe]` native, `target/` and `~/.cargo/bin` not; a check due first, after a day and when the last one is in the future. The use case over in-memory fakes (a hex-encoding digest, a `sig:`-prefix signature): up to date or older downloads nothing, a newer signed and listed release replaces the executable, an unsigned `SHA256SUMS` is refused before the binary is downloaded, a missing or wrong checksum, an invalid version and a binary the executable refuses all leave it untouched, a manual update outside the native install is an error and a background one is skipped without recording a check, and the background check skips while disabled or not due and records a due check even when the channel fails. Adapters: SHA-256 of `abc`; RFC 8032 TEST 1 accepted, one flipped byte or a key that is not hex refused, the shipped key is 32 bytes; on Unix with a real temp dir, a binary printing the expected version renamed in (the old inode intact), a wrong version leaving the exe and no temp file, a symlinked home or `~/.local/bin` still native, a replacement through a symlink landing on the native file (the link kept), an abandoned staged binary removed and a fresh one of another tab kept; the Windows old-copy name carries the pid and the time; the marker's time read back. `GithubReleaseChannel` has no unit test: the release workflow's `verify` job runs the installers and `tabsmith update` against the real releases. |

Smoke / end-to-end: **not automated** (there is no `tests/e2e.rs`). The review rounds ran these flows, and many more,
with a pyte-based PTY driver (step files of keys/waits/snapshots, temp HOME and `TABSMITH_HOME`) against
`debug shell` and real dotfile installs; the list stays the manual checklist:
1. Harness opens its own PTY, runs `tabsmith debug shell bash` (and zsh, fish, pwsh where installed)
   with `TABSMITH_HOME=<tmp>`, feeds the PTY output into a terminal model of 80×24, writes keystrokes.
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
  terminal → misplaced restore spans. Mitigation: icons are images or two-column slots, exact wide-char span extension, full repaint on resize.
- **Content printed before the wrapper started** is blank in the model; if the menu covers it, restoring shows blanks.
- **Programs that change origin mode / scroll regions / charsets at the prompt**: overlay suppressed (guards §7.5.1);
  charset (DEC graphics) not detected → garbled box (rare).
- **Prompts that bypass the wrappers**: themes that replace PS1 after precmd or mid-edit are re-wrapped (zsh p10k hook,
  zle `line-init`/`line-pre-redraw`; pwsh re-wraps a redefined `prompt` at ReadLine; fish/bash re-wrap every prompt).
  A prompt still printed without markers, an empty PS1, a markerless prompt ending in a newline, and continuation
  lines without a prompt give no buffer → no menu (fails safe, §5.6 ponytail).
- The cursor row decides "at the prompt" (§5.6): inline TUIs (fzf `--height`, fish's pager) are left alone, but a
  newline-ended PS1's line while zsh lists completions below it also reads as not at the prompt.
- Typeahead before the wrapper starts may be echoed twice (F36).
- Startup files run twice: the outer shell reads its rc files up to the pre block (zsh `.zshenv`, `/etc/zprofile`, a
  login bash's `/etc/profile`), the wrapped shell reads them all again. fish: only `--private`
  reaches the wrapped fish (`-C`, `--features`, `-d` are dropped).
- Env/alias capture lags one prompt (`_ pre-cmd` runs in the background at precmd); generators use the previous env.
  pwsh never reports its env (generators use the wrapper's).
- Generator commands are third-party spec code run on keystrokes; mitigations: no shell, own process
  group / job, env sanitised, timeouts, killed at session end, `fig`/`tabsmith` stubbed, QuickJS memory/interrupt limits.
  Repo-local `.fig/autocomplete/build` specs run only for path-form commands (`./x`), which already trust that tree.
- Markers are forgeable by any local program (the sid is in every child's env, §5.1.2): marker-based history is not a
  security boundary; untrusted *output* cannot forge them (D35).
- LLM mode sends recent history + cwd to a third party: opt-in only, `inline.historyCount`, privacy filter and
  NoHistory lines excluded, keys only from env.
- History: bash `set +o history` is not detected; under `ignoreboth` an immediate repeat is not recorded; history
  compaction and concurrent `integrations install` runs have no lock.

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
- **Verified by code review and cross-compilation only** (CI builds and unit-tests on `windows-latest`; the review
  rounds had no Windows console): ConPTY ordering and the deferral, the ConPTY close on a helper thread, job objects,
  the Toolhelp parent lookup, the console-less pwsh encoding guard. Ctrl+Space arrives as a NUL key record, which is
  not forwarded (unverified whether PSReadLine's MenuComplete is affected).
- **ConPTY re-rendering**: ConPTY repaints from its own buffer, may reorder OSC relative to text and repaint whole
  regions after resize. Mitigations: deferred `EndPrompt`/`NewCmd` (§5.1.4), redraw after every batch, 150 ms full
  repaint after resize. portable-pty uses the system ConPTY; shipping a side-loaded `conpty.dll` + `OpenConsole.exe`
  next to `tabsmith.exe` (as ish does with `useConptyDll`) is the fallback plan.
- Requires Windows 10 1809+ (ConPTY); legacy conhost without VT processing → doctor error, launch refused.
- `win32-input-mode` (`CSI ?9001h`) from ConPTY must be stripped or Windows Terminal switches key encoding (§7.5.4).
- No SIGWINCH: resize via `WINDOW_BUFFER_SIZE_EVENT` + 250 ms polling; legacy conhost may not send the event.
- Child exit does not EOF the ConPTY pipe: the master is dropped on a helper thread after the child exits (its close
  may wait for pipe room that only the main loop's credits free), then PtyEof or a 200 ms grace ends the loop.
- PSReadLine: PreExec comes from a `PSConsoleHostReadLine` wrapper (§5.4.5); a custom `AddToHistoryHandler` runs twice
  per line. ListView predictions own the rows below a non-empty line: the menu stays hidden there (§7.5.1).
- pwsh: the AllHosts profile (`profile.ps1`) runs in both the outer and the wrapped shell.
- cmd.exe and nushell unsupported (D20); Git Bash supported best-effort only in Windows Terminal (mintty is not a
  Windows console → launch refused); specs that shell out to `bash -c` (138 uses) or macOS tools give no results
  without Git Bash.
- Windows PowerShell 5.1 (`powershell.exe`) SHOULD: same scripts, different `$PROFILE`; `[char]27` used instead of `` `e ``.
- A running `tabsmith.exe` cannot be replaced/deleted (upgrades need all sessions closed).
