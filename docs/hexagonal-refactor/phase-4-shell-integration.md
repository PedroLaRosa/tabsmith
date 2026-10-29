# Phase 4 (shell_integration): preparation

Status: **done** at `ab7052b` (`6f38da1..ab7052b`, gate green). The decisions it took and their deviations are in
the plan's section 9 "Phase 4 notes"; where they differ from the briefs below, the notes win. This prep was
researched on a frozen snapshot of `a3953cf` (end of Phase 2) by five researchers (domain, ports/adapters, use
cases/CLI, tests/scripts, session context) and merged here; the step prompts are in
`phase-4-shell-integration.args.json` next to this file.

Plan reference: [the plan](../hexagonal-refactor-plan.md), section 5 "Phase 4: shell_integration", the
`shell_integration`, `terminal_session` and `bootstrap` layouts in section 4, section 2 (allowed dependencies,
bridges) and section 9 (D1-D9). Detailed traps per legacy file: [`source-map/integrations.md`](source-map/integrations.md),
plus `source-map/types.md`, `pty.md`, `engine.md`, `doctor.md`, `cli.md` for the session-context and caller edits.
The source map cites `af1b304`; everything below cites **`a3953cf`**. Phase 3 does not touch `integrations.rs`,
`doctor.rs` or `cli.rs`, so those line numbers hold at the Phase 4 start. Phase 3 does change `types.rs`,
`engine.rs`, `pty.rs`, `tracker.rs`, `main.rs` and `bootstrap/cli.rs` (it replaced bootstrap's
`shared_dependencies()` with `SharedDependencies::from_env()`): locate items there by name, not by line.

Since this prep: Phase 3 ended at `f485ea7` and added a fitness rule: a registered module's domain or application
may not use a legacy flat module (`LEGACY_MODULES` in `src/architecture_tests.rs`: `cli`, `doctor`, `engine`,
`integrations`, `types`, …). The design below already complies (time through the `Clock` port, `Shell` from the
shared kernel, the legacy-facing pieces in infrastructure or bootstrap); keep it that way, also in terminal_session,
which is checked by hand (T56). The history module (`src/history`) exists; `SharedDependencies::from_env()` is the
one builder of the shared dependencies.

Short names used below: `I` = `src/integrations.rs`; "paths" = the `Ctx` replacement `IntegrationPaths` (P1).

## Where the code is today (`a3953cf`)

| What | Where | Used by |
|---|---|---|
| clap types `ShellArg` (+ `From<ShellArg> for Shell`), `When` (+ `name()`), `InitArgs`, `IntegrationTarget`, `IntegrationsCmd`, `SetupArgs`, `UninstallArgs` | `I:27-128` | `bootstrap/cli.rs` `Command::{Integrations, Init, Setup, Uninstall}`; `cli.rs:4` (`When`), `cli.rs:68` (`DebugCmd::Shell { shell: Option<ShellArg> }`); `doctor.rs:2, 348` (`When`) |
| `mark_restart` | `I:133-141` | `integrations_cmd` (835), `setup` (967), `doctor.rs:322, 404` |
| `init`: `PRE_SH..POST_PS1` (`include_str!`), `init_cmd`, `init_script`, `init_output` | `I:143-222` | bootstrap `Init` arm; `cli.rs:570` (`debug_shell`); test helper `snapshot()` (1108-1111) |
| launch: `should_launch`, `launch_decision`, `should_launch_cmd`, `get_shell_cmd` | `I:224-305` | bootstrap `_ should-launch`, `_ get-shell`; `init_output` (186) |
| `pre_cmd`, `prune_run_files` | `I:307-343` | bootstrap `_ pre-cmd` |
| `PRE_MARKER`/`POST_MARKER`, `Ctx` + `from_env`, `IntegrationError`, `io_err`/`read`/`write`/`remove`, `check_file`, `is_ignore_line`/`split_bom`/`split_shebang` | `I:345-487` | everything below; `doctor.rs` |
| `Integration` (pub fields `shell`, `path`, `file_when`), catalog `integrations()`, `PWSH_PROFILE_CMD` + `pwsh_profile`, methods `describe`/`file_name`/`rc`/`script_path`/`script_contents`/`source_line`/`write_scripts`/`is_installed`/`install`/`is_own_line`/`has_traces`/`uninstall` | `I:489-768` | `doctor.rs:264-406` (fields, `is_installed`, `install`, `script_path`, `clone`), tests |
| `default_shells`, `apply`, `install_dotfiles`, `uninstall_dotfiles`, `target_shells`, `integrations_cmd`, `status` | `I:770-915` | bootstrap `Integrations` arm; `doctor.rs:403` and its test (843, 849) |
| `confirm`, `answer`, `setup`, `manual_text`, `uninstall`, `uninstall_all` | `I:917-1072` | bootstrap `Setup`/`Uninstall` arms |
| 37 tests + 13 helpers | `I:1074-3102` | (see "Test moves") |
| `types::SessionCtx` (+ `path`, `load`), `EngineMsg::Session(SessionCtx)` | `types.rs:190-211, 250` | `pty.rs:524, 1274`; `engine.rs:29-30, 290, 341, 422-426, 440-446, 448-460, 482-486`, test 1344 |
| pty session-file handling: `session_id()`, `mtime()`, fields `session_path`/`session_mtime`, `poll_files`, `cleanup`, test helper `session()` | `pty.rs:311-325, 355-356, 524, 541-542, 1269-1303, 1817-1818` | the PTY session |
| doctor session check (raw sid, `ctx.data/run/<sid>.json is_file()`) | `doctor.rs:491, 528-539` | stays raw (Phase 10) |
| shared pieces reused | `DataDirectory` (`root`, `state_file`, `run_dir`); `user_home_from_env`; `SharedError`; `AtomicFile::{write_keeping_permissions, write_private}`; `JsonDocumentStore`; `exe_path` + `ExecutablePathError`; `parent_exe`; `is_root`; `is_console` (windows); `which`; `output_with_timeout`; `paint`/`epaint`; `OutputFormat`; `Clock`/`SystemClock`/`FixedClock`; `SharedDependencies::from_env()` (Phase 3) | |

## Behaviour that must not change

Every item is binding (D1). Trap IDs are cited by the step prompts.

### init and the launch decision
- **T1 Lazy, ordered init I/O** (`I:180-207`): `exe_path()` runs first, for every phase, unless snapshot (181-185);
  `parent_exe()` only for Bash/Zsh **Pre**, before the launch decision (190-199); the launch decision (env snapshot
  + stdin check) only for **Pre** (186, 199, 202, 205); Post computes neither. A renderer that takes resolved values
  instead of closures changes what runs.
- **T2 Inverted launch flag** (186): `if snap || should_launch().0 { 0 } else { 1 }`: a launching decision renders
  `0`; snapshot renders `0` **without** evaluating the decision.
- **T3 Header lines** (190-206): bash/zsh `TABSMITH_SHELL=<quote(shell, parent)>` omitted when the parent is `None`
  (196), then `SHOULD_TABSMITH_LAUNCH={n}`; fish `set -g SHOULD_TABSMITH_LAUNCH {n}`; pwsh
  `$TABSMITH_SHOULD_LAUNCH = {n}` (other word order). Snapshot: bin `tabsmith`, parent `/bin/zsh` **also for bash**.
- **T4 Template rendering** (208-221): `(Bash|Zsh, Pre)` share `PRE_SH`; exhaustive match, no `_` arm (D8); body
  `trim_end_matches('\n')` (LF only) then `.replace("{{BIN}}", &quote(shell, &bin))` (every occurrence);
  `lines.join("\n")` with **no** trailing newline. `init_cmd` adds it with `let _ = writeln!(stdout, "{out}")`
  (166, EPIPE ignored); `debug_shell` appends `+ "\n"` (`cli.rs:570`).
- **T5 init command** (154-168): `logger::debug("init {:?} {} --rcfile {rc}")` only when `--rcfile` is given and
  **before** rendering; `{:?}` of `ShellArg` prints `Bash`… (keep its variant names and derived `Debug`), the phase
  prints `pre`/`post`. Any error → stdout `# Could not load source: {e:#}`, exit 0, never an `Err`.
- **T6 `TABSMITH_INIT_SNAPSHOT_TEST`** is read with `var_os(..).is_some()` (set-but-empty counts) **when the script
  is rendered** (175), and both `init` and `cli::debug_shell` go through that read. `debug_shell` propagates the
  error with `?` (main prints `error: {err:#}`, exit 1) and keeps its order: shell resolution
  (`cli.rs:557-565`) → init script (570) → `data_directory_from_env().root()?` (571) → `set_var`s.
- **T7 `include_str!`** paths are relative to the file holding them (`../shell/*`, 146-152); from
  `src/shell_integration/infrastructure/adapters/embedded_shell_scripts.rs` they become `../../../../shell/*` (same
  file names, same case: `pre.sh`, `post.zsh`, `post.bash`, `pre.fish`, `post.fish`, `pre.ps1`, `post.ps1`).
  `env!("CARGO_MANIFEST_DIR")` paths do not change.
- **T8 Launch rules** (242-288): first match in exactly this order; `has` = `env.contains_key` (so `CI=` counts);
  `is` = exact case-sensitive equality; the TMUX sub-rule only inside the `TABSMITH_TERM` branch (253-257); the TERM
  rule only under `cfg!(unix)` (270); the stdin message chosen by `cfg!(windows)` (275-279); reason
  `format!("{} {why}", if launch {"✅"} else {"❌"})` built where the rule is (284-287).
- **T9 Two different env snapshots**: the launch facts keep lossy values and drop only non-UTF-8 **keys**
  (229-231); `_ pre-cmd` drops an entry whose key **or** value is non-UTF-8 (316-318). Never share one env reader.
  stdin: `is_console(stdin raw handle)` under `cfg(windows)`, `stdin().is_terminal()` otherwise (232-238).
- **T10 Print macros**: `_ should-launch` `println!("{why}")`, exit 0 = launch, 1 = not (290-294); `_ get-shell`
  `print!("{p}")` (no newline) + `let _ = stdout().flush()`, exit 1 with no output when there is no parent
  (296-305).

### Paths resolution (the `Ctx` replacement)
- **T11 Order and texts** (`Ctx::from_env`, 368-387): `user_home_from_env()?` (`could not determine the home
  directory`) → data root `?` (`could not determine the home directory (set HOME or TABSMITH_HOME)`, unreachable
  after the home but keep the order) → `exe_path()?` (`could not locate the tabsmith executable: <io>`) → `ZDOTDIR`
  (`var_os`, non-empty, non-UTF-8 kept) → `XDG_CONFIG_HOME` (non-empty) else `home/.config`, then `/fish`;
  `pwsh_profile` `None`. Nothing canonicalized; home is the **user** home, never the data root's parent
  (`source_line` strips the home prefix lexically, 627).
- **T12 When it is resolved (D3)**: `integrations_cmd` before dispatch, for all four subcommands, `status`
  included (831; golden `integrations status dotfiles` with `HOME=""` → `error: could not determine the home
  directory`, exit 1, golden.sh:134); `setup` after the root check **and** the prompt, also when the answer was no
  (944-957); `uninstall` after the prompt, only when not cancelled (1033-1039); doctor after its root check
  (`doctor.rs:166-183`). `init`, `_ should-launch`, `_ get-shell`, `_ pre-cmd` never resolve it. Module builders
  must not resolve anything, call `current_exe`, `which` or spawn `pwsh`. The context is resolved whole, once per
  command (reinstall reuses it for both halves).

### Errors
- **T13 `IntegrationError` texts** (399-413): `Integration not installed: {m}`, `Improper integration installation:
  {m}`, `File does not exist: {p}`, `Permission denied: {p}`, `Io` → `{p}: {e}`; `impl Error` has **no** `source()`
  (415). With a source, doctor's `Failed to fix: {e:#}` (`doctor.rs:142`, fix at 321) would print the io error
  twice. Doctor uses the inner message for NotInstalled/Improper, `to_string()` otherwise, and offers no fix for
  PermissionDenied (`doctor.rs:308-318`): the variants stay distinguishable.
- **T14 io mapping** (417-449): `NotFound` → `FileDoesNotExist(path)`, `PermissionDenied` → `PermissionDenied(path)`,
  else `Io(path, e)` (a directory read: `…: Is a directory (os error 21)`); `read` = `fs::read_to_string` (non-UTF-8
  → `Io` InvalidData); `write` keys `AtomicFileError::{Io, Rename}` to the path they carry (the resolved target's
  parent dir for `create_dir_all`, the user-facing link path otherwise) and keeps the unreachable
  `NoParentDirectory` arm; `remove`: `NotFound` is `Ok`.
- **T15 anyhow chains printed with `{e:#}`**: the pwsh `$PROFILE` error (551-560; printed by `apply` 790, `status`
  880, `doctor.rs:276-277`), the executable error (init 164, main), the home errors (main), setup's spec extraction
  (999, main). anyhow's `{:#}` prints Display then `: <cause>` for each `source()`; `{:#}` on a plain std error
  prints only its Display. Every one of these texts must come out byte-identical, including where a caller formats
  a typed error with `{e:#}` directly (P3).

### Dotfile operations
- **T16 `is_installed` precedence** (651-689): whole-file → compare the file with its script (653-655); dotfile:
  read error first (656), BOM stripped (657), a `# tabsmith ignore` line anywhere → `Ok` (658-660); candidate lines
  = trimmed, non-empty, not starting with `#` (661-665; the markers are such comments); then Pre line → Pre script
  file → Post line → Post script file (666-687), interleaved; Improper vs NotInstalled by `lines.contains(&src)`
  (677); messages `{dotfile} does not source {phase} integration` + ` first`/` last`; the script check reads first
  (its read error keyed to the script path) then `{script} should contain:\n{expected}` (452-460).
- **T17 Opt-out** (463-470): `trim_start`, `#`, `trim_start`, ASCII-lowercase, `starts_with("tabsmith ignore")`.
  Only `is_installed` honours it: `uninstall` and `has_traces` still see/remove own lines in an opted-out file.
- **T18 `install` order** (691-725): installed (incl. opt-out) → no-op; whole-file → plain write of its script;
  `path.exists()` (follows symlinks: a dangling link takes no backup and contents `""`); backup dir
  `<data>/backups/<now_secs>` via `create_dir_all` keyed to the dir, `fs::copy` (byte copy, follows symlinks, copies
  permission bits) keyed to the dotfile, named after the link's `file_name()`; two installs in one second share the
  dir; then `uninstall`, then re-read, then **write the scripts before the dotfile** (709), write the dotfile only if
  `new != contents`. The clock is read only inside the `exists()` branch. A non-UTF-8 dotfile is backed up before
  the read fails, so the backup is a copy from the path, never read-then-write.
- **T19 Dotfile layout** (710-720): `{bom}{shebang}{PRE_MARKER}\n{pre source}\n{rest}`, then
  `trim_end_matches('\n')` + `\n{POST_MARKER}\n{post source}\n`; the shebang is detected after the BOM;
  `split_shebang` appends `\n` to a shebang-only file (485).
- **T20 `uninstall`** (748-767): whole-file → remove (missing is fine); dotfile only `if path.exists()`: BOM split,
  `split_inclusive('\n')` lines filtered by `!is_own_line`, written as `format!("{bom}{}\n", kept.trim())` whenever it
  differs (an empty file becomes `"\n"`; leading/trailing blank lines and a final CRLF go); then removes the Pre then
  the Post script files with `?` between, even when the dotfile is missing.
- **T21 `is_own_line`** (727-734) compares `line.trim_end()` with both markers and both source lines: CRLF and
  trailing spaces tolerated, leading spaces not (`is_installed` trims both sides, so an indented source line counts
  as installed but is never removed).
- **T22 `has_traces`** (736-746): whole-file → `path.exists()`; dotfile → Pre script exists || Post script exists ||
  `read(path).is_ok_and(|t| t.lines().any(own))`: read errors are `false` and the BOM is **not** stripped here.
- **T23 Exact strings**: `source_line` (624-638: under home → `${HOME}/` + components joined by `/`, also on
  Windows; else `p.display()`; pwsh `if (Test-Path "{p}") {{ . "{p}" }}`, others `[ -f "{p}" ] && . "{p}"`,
  unescaped); `rc` (579-590: pwsh `profile`; else file name, leading dots trimmed, up to the first `.`);
  `script_path` = `<data>/shell/{rc}.{phase}.{ext}`, ext `ps1` for pwsh else the shell name (592-602);
  `script_contents` exact formats (604-622; pwsh hard-codes `--rcfile profile`); `describe` (564-570: whole-file
  `{shell} {phase}`, dotfile `{shell} pre post into {file_name}`); `file_name` lossy, `""` when none (572-577).
- **T24 Which write** (D1): dotfiles, `~/.tabsmith/shell` script files and fish `conf.d` files all go through
  `AtomicFile::write_keeping_permissions` (431-441; golden dumps them at mode 644). Never `write_private` (0600) or
  `write_document`. `state.json` goes through `JsonDocumentStore::save` (`write_document`); run files through
  `write_private`.

### Catalog and pwsh
- **T25 Catalog** (499-543): Bash = `.bashrc`, then whichever of `[.profile, .bash_login, .bash_profile]` exist in
  that order (`Path::exists`, follows symlinks), else `.profile`; Zsh = `ZDOTDIR` or home, `.zshrc` then
  `.zprofile`; Fish = `conf.d/00_tabsmith_pre.fish` (Pre) then `99_tabsmith_post.fish` (Post), whole-file; Pwsh =
  one dotfile at the override, else `pwsh_profile()?`.
- **T26 pwsh spawn**: only for Pwsh and only when the paths carry no `$PROFILE`; once per catalog build, never
  cached (`apply` builds per call, `status` once: `integrations reinstall` spawns twice, setup's failed auto install
  + manual fallback spawns twice). Doctor's `script_bins` sets the override to `unwrap_or_default()` (an empty path
  is a valid known profile) to avoid a spawn (`doctor.rs:337-341`); its stale-binary fix uses the un-overridden
  paths and spawns (403). An explicit `integrations install dotfiles pwsh` spawns even when pwsh is not on PATH.
- **T27 pwsh error texts** (550-561): `pwsh -NoLogo -NoProfile -Command PWSH_PROFILE_CMD`, 5 s; context `could not
  ask pwsh for $PROFILE` over the io error (`No such file or directory (os error 2)`, `timed out after 5s`,
  `stream did not contain valid UTF-8`); trimmed output; empty → `pwsh printed an empty $PROFILE`. Each surfaces as
  a per-shell entry at its shell's position (`pwsh: …`). Reference texts: `process_runner.rs` tests.

### Commands and their output
- **T28 `apply`** (779-805): catalog failure `"{shell}: {e:#}"`; operation failure
  `"{paint("1", &describe)}: {e}"` (Display only; `paint` checks the **stdout** tty even though the text goes to
  stderr); `ran = true` **before** the op runs; the loop continues past every error; any error discards `ran`;
  `bail!("\n\n{}", errs.join("\n\n"))`. Consumers: main (`error: ` + text), setup (`println!("{e:#}")` at 980, 996),
  `uninstall_all` (`eprintln!("{e:#}")`), doctor (`Failed to fix: {e:#}` via 403), test
  `integrations_install_dotfiles_reports`.
- **T29 Shell sets**: `default_shells` = `[Bash, Zsh, Fish]` + Pwsh when `which("pwsh")` (770-777), used by
  `integrations` subcommands when the target is `all` or `dotfiles` without a shell (823-828; an explicit shell skips
  `which`) and by top-level `uninstall` (1040); `setup` uses `Shell::ALL.filter(which(name))` (958-961), a
  different set. `which` is the shared `executable_lookup::which`, never `js::find_exe`.
- **T30 `integrations` subcommands** (830-872): `Installed!`/`Already installed`/`Uninstalled!`/`Not installed`,
  each suppressed by `--silent`; the restart mark before the print; `Reinstall` = uninstall (silent)`?` then install:
  an uninstall failure aborts with no install and no output; every subcommand returns `Ok(0)`; failures surface only
  through main.
- **T31 `status`** (874-915): catalog, then the check per integration, shell by shell (the pwsh spawn at its
  position); plain prints **all** rows (`{paint("32","✔")|paint("31","✘")} {describe}`) then all `{shell}: {e}`;
  JSON via `serde_json::json!` without `preserve_order`, so keys are sorted:
  `{"errors":[{"error":{"message":null,"title":…},"shell":…}],"integrations":[{"description","file_name","installed","shell"}]}`;
  `json` = `v.to_string()`, `json-pretty` = `serde_json::to_string_pretty`; always `Ok(0)`. Never a
  `#[derive(Serialize)]` DTO and never `OutputFormat::render` (different pretty printer). No test and no golden entry
  covers the JSON (golden.sh:87 fails at parse: `--format` must precede the target).
- **T32 `setup`** (943-1004): root without `--force` → two `eprintln!(epaint(..))` lines (`1;31` `Installing as root
  is not supported.`, `31` `If you know what you're doing, run the command again with --force.`), `Ok(1)`, before
  any prompt; prompt `Do you want tabsmith to modify your shell config (you will have to manually do this
  otherwise)? [Y/n] ` unless `--no-confirm`; paths after the prompt; auto: install, on `Ok` mark (if installed) then
  `Installed!`/`Already installed`, on `Err` `println!(paint("1","Could not automatically install:"))` +
  `println!("{e:#}")` and manual; manual: `print!` (not `println!`) of the manual text, then write the script files
  of the dotfile integrations only (`file_when.is_none()`), on `Err` `Could not install required files:` +
  `{e:#}`; then the spec extraction on every path, fatal, context `could not extract the completion specs`; the
  restart line `Restart your terminal (or open a new tab) to start using tabsmith.` only if not manual and only
  after the extraction; `Ok(0)`. Every line is printed at the same point relative to install, mark and extraction
  (`println!` panics on a closed stdout).
- **T33 `confirm`/`answer`** (920-941): `stdin().is_terminal()` on **every** platform (not `is_console`), checked
  before printing; non-tty → `bail!("You must run with --no-confirm if unattended")`; `print!(prompt)` +
  `stdout().flush()?`; `read_line` error propagated; 0 bytes → `println!()` and no; trimmed, lowercased `""`, `y`,
  `yes` → yes; anything else no. Every other run uses `</dev/null`, so only integrations-diff.sh's pty group (P16,
  group i) sees the prompt bytes, the answers and T12's prompt-then-paths order.
- **T34 `uninstall`** (1032-1072): prompt `Are you sure want to continue uninstalling tabsmith? [Y/n] ` (keep the
  missing "you"); declined → `Cancelled`, `Ok(1)`; paths after the prompt; default shells; uninstall over **every**
  integration (`|_| true`, rewriting untouched `.profile`/`.bash_profile` by the whitespace normalisation, unlike
  `integrations uninstall` which picks `has_traces`); failures `eprintln!("{e:#}")` **before** the purge; purge =
  `remove_dir_all(paths.data)`, `NotFound` ignored, other errors `eprintln!("Could not remove {data}: {e}")` with
  the **raw** io Display (never through `io_err`) and failed; failed → `Uninstalled tabsmith with errors (see
  above).`, 1; else `Uninstalled tabsmith.`, then (purge) `Restart open terminals to stop tabsmith in them.`, then
  `Remove the binary with your package manager, or delete {bin}.`, 0. Never `Err`.
- **T35 Manual text bytes** (1006-1030): header lines, labels `bash:   `, `zsh:    `, `pwsh ($PROFILE):` joined as
  `{label} {line}`; pwsh only when the shells contain Pwsh; fish never; always `.bashrc or .zshrc`; uses
  `home/.bashrc`, `home/.zshrc` (ignores `ZDOTDIR`) and `home/profile`; ends with an extra `"\n"`.
- **T36 Data paths come from the paths value, never from the env**: script files and backups live under
  `paths.data` (599-601, 700); the purge removes `paths.data` (1051). Tests pass temp paths; an adapter that took
  `SharedDependencies.data_directory` for these would write to or delete the developer's real `~/.tabsmith`.

### `state.json` and the restart reminder
- **T37 `mark_restart`** (133-141): resolves `data_directory_from_env().state_file()` at call time (no home → silent
  no-op); `JsonDocumentStore::load`: missing/blank → empty map, corrupt/not-an-object/unreadable → **nothing
  written**; inserts `"doctor.prompt-restart-terminal": true`, keeps every other key (`autocomplete.recency`, doctor's
  reader/clearer); `save` = pretty sorted JSON + `\n` through `write_document` (can recreate a purged data dir: do
  not add a purge guard), 0600; save errors ignored. Golden pins `./.tabsmith/state.json 600`.
- **T38 Who marks**: `integrations install`/`reinstall` and `setup` only on `Ok(true)`, before the print; an install
  error → no mark. Doctor's two fixes mark **unconditionally** after a successful fix (`doctor.rs:320-323,
  402-405`). The per-integration and per-shell install that doctor and the tests call never marks. No test may
  write the real `state.json`. Doctor fixes only without `--all` (`doctor.rs:130-133`) and golden runs only
  `doctor --all`: integrations-diff.sh group h (P16) is the only guard of the fix texts (`Attempting to fix
  automatically...`, `Failed to fix: {e:#}`, the re-run), the per-variant reason, PermissionDenied's missing fix, both
  marks and the `PS.` reader/clearer.

### clap (D4)
- **T39 Frozen identifiers**: `InitArgs` fields `shell`, `when`, `rcfile` (`#[arg(long)]`); `ShellArg` variants
  `Bash, Zsh, Fish, Pwsh` and the phase variants `Pre, Post`, in that order, **without** `///` doc comments (they
  would become per-value help); `IntegrationTarget` doc comments `Shell dotfile integrations`/`All integrations` and
  field `shell`; `IntegrationsCmd` doc comments, only `Install` documents `--silent`, `Status` keeps `#[arg(long, short,
  value_enum, default_value_t)]`; `SetupArgs`/`UninstallArgs` verbatim (`short = 'y'` only on uninstall's
  `--no-confirm`); bootstrap's `Command` docs, order and `#[command(alias = "install")]`. Do not rename the
  `#[derive(Args)]` structs (`InitArgs`, `SetupArgs`, `UninstallArgs`: the struct name is clap's implicit group id)
  nor `IntegrationsCmd`/`ShellArg`; renaming a `ValueEnum`/`Subcommand` enum type is invisible. The golden capture
  diffs every `--help` and `completion bash|zsh|fish`.

### Session context (`_ pre-cmd`, terminal_session)
- **T40 SessionId filter** (`I:309-315`): `std::env::var("TABSMITH_SESSION_ID").ok()` (unset and non-UTF-8 → none),
  non-empty, every byte ASCII alphanumeric, `-` or `_`; no length cap; the comment says "hex" but do not narrow it.
- **T41 Invalid id** → silent `Ok(1)` **before** any data-directory resolution (an invalid id without a home prints
  nothing; a valid id without a home prints `error: could not determine the home directory (set HOME or
  TABSMITH_HOME)`). Never `?`-propagate an invalid id.
- **T42 Error texts** via main `error: {err:#}`: no home → the text above; purged root → `No such file or directory
  (os error 2)`; `run/<sid>.json` is a directory → `writing <path>: Is a directory (os error 21)`.
- **T43 Write** through `AtomicFile::write_private`: `create_dir` (purge guard: a purged root is not recreated),
  `run/` chmod 0700 on every write even when the write then fails, file 0600 at creation, temp `<sid>.tmp-<pid>` left
  behind on failure. Then prune, **only after a successful write** (325-326).
- **T44 Prune** (332-343): any regular file in `run/` (stray `*.tmp-<pid>` too) whose mtime is **strictly** more than
  24 h old, measured with `SystemTime::elapsed()` (not the `Clock`); `DirEntry::metadata()` does not follow symlinks
  (a symlink to a stale file stays, so does its target); directories (`debug-shell-<pid>`) stay; a future mtime
  (`elapsed()` `Err` → 0) stays; every error ignored.
- **T45 Record wire format** (`types.rs:190-199`, written with `serde_json::to_string`): compact, no trailing
  newline, keys `env`, `alias`, `ts` in that order; `alias: None` → `"alias":null`; each field its own
  `#[serde(default)]`; `HashMap<String,String>`, `Option<String>`, `u64`. `ts` = `now_ms()` (0 before the epoch),
  never read in production.
- **T46 Readers**: `load` swallows every failure into `None`; pty resolves the path once at start with `.ok()` (no
  home → no reload, no removal, the session still runs); reload when `metadata().modified()` (follows symlinks) is
  `Some` and differs; the mtime is stored **before** the load; a deleted file (`None`) does not reset it; the message
  is sent only when the load is `Some`; the session file is polled before settings; cleanup removes the file
  **last** with `remove_file` (removes a symlink, not its target), error ignored.
- **T47 Engine semantics** (`engine.rs:422-426, 440-460, 482-486`): an **empty** session env falls back to the
  wrapper env (in `env()` and in `compute`'s inline copy); alias `None` or `Some("")` → no aliases; every `Session`
  message clears the alias cache and the first-token cache; `ts` never read; the initial state behaves as "no
  context".
- **T48 Not the repository**: doctor's Session check (`doctor.rs:491, 528-539`: raw sid, `Some("")` checks
  `run/.json`, `is_file()`, root from the paths value) and the logger's raw `TABSMITH_SESSION_ID` tag stay as they
  are.
- **T49 Coverage**: the golden capture never runs `_ pre-cmd` (it unsets `TABSMITH_SESSION_ID` and runs only
  `--help`); `precmd-diff.sh` is the guard of the writer. Nothing covers the readers (T46-T47): the pty test helper
  sets `session_path: None` (`pty.rs:1817`) and the engine's one Session test checks only the first-token cache, so
  step 17 pins them with characterization tests against the legacy code before switching (P17).

### Tests and tooling
- **T50 Snapshot self-bless** (`I:1160-1172`): the test writes the snapshot when `TABSMITH_BLESS` is set **or the file
  is missing**. `snapshot_path` must stay `CARGO_MANIFEST_DIR/tests/snapshots/init_{shell}_{phase}.snap` with
  `pre`/`post` and the shell names. Never set `TABSMITH_BLESS`; after every test run `git status --porcelain
  tests/snapshots` must print nothing (APFS is case-insensitive: a case-only difference passes locally and goes
  vacuous on Linux CI). The snapshot helper calls the production snapshot-mode render with `snapshot = true`
  passed explicitly (it must not set or read the env var).
- **T51 The `ctx(dir)` fixture values are load-bearing** (1090-1100): home `dir/home`, data `home/.tabsmith` (under
  home, so source lines read `${HOME}/…`), bin `/opt/tab smith/it's/tabsmith` (quoting), no zdotdir, fish dir
  `home/.config/fish`, pwsh profile `Some(home/.config/powershell/Microsoft.PowerShell_profile.ps1)` (without it the
  ungated bom/enumeration tests spawn pwsh or panic).
- **T52 Multi-line string literals**: a sibling `tests.rs` has no `mod tests {` wrapper, so moved items are dedented
  by 4 spaces, **except** the continuation lines of the literals at `I:1732-1737, 1822-1831, 2123-2125, 2158-2161,
  2381-2388, 2580-2583, 2632-2633, 2809-2812` (their leading spaces are shell input). testlock collapses spaces
  inside strings, so it would not notice: compare string tokens (step prompts give the check).
- **T53 testlock** keys tests by fn name: a duplicated name silently hides a missing test. testlock's `DUPLICATE`
  line lists every test name defined twice in the tree; only `should_not_save_over_settings_that_cannot_be_loaded`
  (settings, before Phase 4) may be there. After every move
  `grep -rhn 'fn integrations_' src | sed 's/.*fn //;s/(.*//' | sort | uniq -d` prints nothing and
  `cargo test --locked integrations_` lists 42 (37 here + 5 in shared) plus new ones. Judge a step with
  `python3 scripts/hexagonal-refactor/testlock.py . phase4-start --diff` (Phase 1 left most `integrations_*` tests
  CHANGED vs `ef807c1`).
- **T54 cfg**: 21 tests and `pty_run`, `fake_bin`, `bash_session` are `#[cfg(unix)]`; 16 tests are ungated and
  run on Windows CI (init_snapshots, init_syntax, post_scripts_emit_markers, dotfile_install_uninstall,
  dotfile_detection, enumeration_and_whole_files, install_dotfiles_reports, bash_prompt_command_rewrap,
  pwsh_post_preexec, dotfile_bom_stays_first, zsh_rprompt_wrapped_and_kept, bash_prompt_command_array_runs_first_once,
  confirm_eof_is_no, uninstall_after_binary_moved, manual_lines_are_recognised, prune_run_files), so `ALL`, `ctx`,
  `snapshot_path`, `snapshot`, `syntax_ok`, `init_for`, `run_post`, `GHOSTTY`, `VSCODE_BASH` and the test driver stay
  ungated; `use std::os::unix::…` stays inside the gated fns; `pty_run` imports `std::io::Write` and
  `std::time::Duration` itself. `integrations_should_launch_table` keeps `#[cfg(unix)] #[test]
  #[allow(clippy::type_complexity)]`.
- **T55 pwsh is not installed here**: `pwsh_post_preexec`, `pwsh_post_status_and_rewrap`,
  `pwsh_no_history_and_listview`, `pwsh_utf8_under_oem_code_page`, `pwsh_pre_launch_rules` and the pwsh checker of
  `syntax_ok` are no-ops locally; they first run in CI. The step 1 pins are the local guard for the pwsh bytes. Do not
  install pwsh without the user's consent.
- **T56 Fitness scanner**: it reads string literals as code (`architecture_tests.rs` `paths_in`), and the moved
  `script_contents`/`source_line` format strings hold `]::` and braces. shell_integration is registered only at the
  end, so after each step that creates or changes shell_integration (or terminal_session) files, add the module to
  `REGISTERED_MODULES` locally, run `cargo test --locked architecture`, and revert before committing.
- **T57 Build hygiene**: clippy (`--all-targets`, tests included) may not exceed 3 warnings (unused imports count)
  and sits at exactly 3 (two `useless_format` in I's tests at 1956 and 2061, which move unchanged with their tests,
  and one in `pty.rs`'s tests): no headroom. Where the old code borrowed a field and the new accessor already returns
  a reference, write the accessor without `&` (`&c.home()` is `&&Path`: clippy `needless_borrow` /
  `needless_borrows_for_generic_args`); sites: I:1273, 1393, 1414, 1433-1435, 2052-2083, 2936, 3052, `doctor.rs:293,
  382`. `cfg(windows)` code (the
  `is_console` import 12-13 and the stdin branch 232-236) is never compiled here: move it verbatim and check every
  path by hand (`grep -n 'cfg(' <file>`); if `IsTerminal` is then used only in the non-Windows branch, gate its
  import.

## Decisions

Each settles a plan gap or a conflict between the researchers. Deviations from the plan's letter are marked
**(deviation)**; the last step records them in plan section 9.

- **P1 `Ctx` → domain VO `IntegrationPaths`** in `domain/value_objects/integration_paths.rs`: private fields; `new(home,
  data_root, binary: String, zdotdir: Option<PathBuf>, fish_config_dir, pwsh_profile: Option<PathBuf>)`; accessors
  `home()`, `data_root()`, `binary()`, `zdotdir()`, `fish_config_dir()`, `pwsh_profile()`; production
  `with_pwsh_profile(PathBuf)` (doctor's override, T26); `#[cfg(test)] with_binary(String)` (D7, the "moved binary"
  tests). `Clone + Debug`. Resolution `integration_paths_from(&DataDirectory) -> Result<IntegrationPaths,
  ShellIntegrationError>` lives in `infrastructure/adapters/os_shell_environment.rs` (a legacy anyhow free fn in
  `I` until step 7) with the exact order of T11, exposed as `ShellIntegrationModule::locate_integration_paths()`
  (precedent `SettingsModule::locate_settings_file`). Only CLI handlers and legacy doctor call it, at the T12
  moments; use cases take `&IntegrationPaths` per call, as the old code took `&Ctx`. Behaviour: same six values, same
  reads in the same order, resolved at the same moments; the override stays possible. ("Context" was rejected: it
  already names `SessionContext` and `GeneratorContext`.) Data layout owner: the script dir and the backups dir are
  joined onto `data_root()` (`data_root()/shell` in `Integration::script`, P5; `data_root()/backups/<secs>` in the
  installer, P13), exactly as the legacy code does. The shared kernel's `DataDirectory::shell_dir()`/`backups_dir()`
  (Phase 1, tested, no caller) stay unused: carrying them in `IntegrationPaths` would take `new` to 8 parameters
  (clippy `too_many_arguments`, no budget, T57) and change the T51 fixture, and deleting them would delete two Phase 1
  tests (MISSING vs `phase4-start`). Recorded in the Phase 4 notes; Phase 11 decides (hand-off below).
- **P2 One error, no status VO (deviation).** `domain/shell_integration_error.rs` `ShellIntegrationError` = the five
  `IntegrationError` variants verbatim (same names, arity, Display) + `Environment(String)` + `SpecBundle(String)`;
  `#[derive(Debug)]`, hand-written Display, `impl Error` with **no** `source()` for any variant (T13). A check
  returns `Result<(), ShellIntegrationError>` ("installed" = `is_ok()`), which keeps the locked `matches!(..,
  Err(X(_)))` and `.unwrap()` lines. No `domain/value_objects/integration_status.rs`: the glossary's
  `IntegrationStatus` becomes the status-row DTO in `application/integration_status.rs` (the plan's file). Doctor
  matches the same variants and prints the same texts.
- **P3 Environment and spec-bundle errors are pre-rendered.** The adapter (or bridge) renders the full chain once,
  exactly as anyhow's `{:#}` does (Display, then `: ` + each `source()`), into `Environment(message)` (Display =
  message) or `SpecBundle(message)` (Display = `could not extract the completion specs: {message}`). Examples:
  `could not ask pwsh for $PROFILE: timed out after 5s`, `pwsh printed an empty $PROFILE`, `could not locate the
  tabsmith executable: <io>`, `could not determine the home directory`. Why not a boxed error with `source()`
  pass-through (the `SettingsError::Storage` shape): `apply`, `status` and doctor format the catalog error with
  `format!("{e:#}")` on the value itself, which prints only Display for a non-anyhow error and would drop the cause;
  a pre-rendered message prints the same text under `{}`, `{:#}` and anyhow's `{:#}`. Pin each text with a
  `should_` test that reuses the expected strings of `process_runner.rs` / `executable_path_error.rs`.
- **P4 clap mirrors.** `When` → domain `IntegrationPhase { Pre, Post }` (+ `name()`) and the clap mirror
  `IntegrationPhaseArg` (ValueEnum, variants `Pre, Post`, no doc comments) with an explicit-arm `From` (D8); the clap
  `IntegrationTarget` → `IntegrationTargetArg` (variants, field, doc comments and derives `Subcommand, Clone, Debug`
  verbatim) + `From<IntegrationTargetArg> for IntegrationTarget`. Domain `IntegrationTarget { Dotfiles(Option<Shell>),
  All }` with `shells(pwsh_on_path: impl FnOnce() -> bool) -> Vec<Shell>`: `Dotfiles(Some(s))` → `[s]`;
  `Dotfiles(None) | All` → `[Bash, Zsh, Fish]` + Pwsh when `pwsh_on_path()` (T29, lazy). The CLI passes
  `|| which("pwsh").is_some()`; top-level uninstall uses `IntegrationTarget::All`. Setup's set stays its own
  (`Shell::ALL` filtered by `which`). `ShellArg`, `InitArgs`, `SetupArgs`, `UninstallArgs`, `IntegrationsCmd` keep
  their names (T39). clap derives help, values and completions from variant/field names and attributes, never from
  enum type names; the golden capture proves it.
- **P5 Domain shape.**
  - `Integration` entity (`domain/entities/integration.rs`, `Clone + Debug`): private `shell`, `path`,
    `whole_file_phase: Option<IntegrationPhase>`; constructors `dotfile(shell, path)` and `whole_file(shell, path,
    phase)`; accessors `shell()`, `path()`, `whole_file_phase()`; `describe()` and `file_name()` stay `&self ->
    String` (tests map with `Integration::file_name`/`Integration::describe`); private `rc()`;
    `script(paths, phase) -> IntegrationScript` (the `~/.tabsmith/shell` file: `script_path` + `script_contents`
    bodies); `whole_file_script(paths) -> Option<IntegrationScript>` (fish `conf.d`: its own path + the contents of
    its phase); `pub(crate) source_line(paths, phase)`.
    **(deviation: accessors in place of `to_primitives()`.)** Plan section 1 lists `Integration` among the entities
    exposed through a DTO with `to_primitives()`; it keeps the read-only accessors above instead, and
    `GetIntegrationStatusUseCase` maps `describe()`, `shell().name()` and `file_name()` into each `IntegrationStatus`
    row (P10), because the row also carries the check result (`installed`), which is not an entity property, so a
    `to_primitives()` would not replace that mapping (precedent: `Settings::entries()`, plan section 9).
  - `IntegrationScript` VO: `new(path, contents)`, `path()`, `contents()`, `check(actual: &str) -> Result<(),
    ShellIntegrationError>` (the `Improper` message of `check_file`, 456-459). The read stays with the caller (T16).
  - `Dotfile` VO (`domain/value_objects/dotfile.rs`, also `pub(crate) const PRE_MARKER/POST_MARKER` and the private
    `is_ignore_line`/`split_bom`/`split_shebang`): `new(text)`, `text()`, `is_opted_out()` (BOM stripped),
    `sourced_lines()` (BOM stripped, trimmed, non-empty, not `#`), `with_blocks(pre_source, post_source) ->
    Dotfile` (T19), `without_blocks(pre_source, post_source) -> Dotfile` (T20, `trim_end` matching T21),
    `has_own_line(pre_source, post_source)` (raw `lines()`, **no** BOM strip, T22). Each method reproduces one legacy
    formula with its own BOM/trim rule.
  - Services (free fns, the `settings_catalog.rs` precedent): `integration_catalog::integrations(paths, shell,
    exists: impl Fn(&Path) -> bool, pwsh_profile: impl FnOnce() -> Result<PathBuf, ShellIntegrationError>)`
    (499-543 verbatim; the closure is called only for Pwsh without an override, T25-T26);
    `manual_install_instructions::text(paths, shells)` (T35); `launch_policy::decide(&LaunchFacts) ->
    LaunchDecision` (242-288 verbatim, `cfg!` kept, reason with ✅/❌ built here, T8);
    `init_script_renderer::render(shell, phase, template: &str, binary: &str, parent_shell: impl FnOnce() ->
    Option<String>, launches: impl FnOnce() -> bool) -> InitScript` (186-221 verbatim with `parent_exe()` →
    `parent_shell()`, `should_launch().0` → `launches()`) and `render_snapshot(shell, phase, template)` (= render with
    `tabsmith`, `Some("/bin/zsh")`, launches; constants in the domain).
  - VOs `LaunchFacts::new(env: HashMap<String, String>, stdin_is_terminal: bool)` (has/is/stdin accessors),
    `LaunchDecision` (`launches()`, `reason()`), `InitScript` (the rendered text, unchanged; `text()` /
    `into_string()`).
- **P6 Application service `IntegrationInstaller`** (`application/integration_installer.rs`; the skill allows
  application services; it is not a use case, so use cases may share it): holds `Rc<dyn DotfileRepository>`,
  `Rc<dyn BackupRepository>`, `Arc<dyn Clock + Send + Sync>` (what `SharedDependencies` holds),
  `Rc<dyn ShellEnvironment>`. Methods (bodies = the legacy methods call for call, with `read`/`write`/`remove`/
  `Path::exists`/backup/`now_secs` through the repositories and the clock): `integrations(paths, shell)` (catalog
  with `|p| dotfiles.exists(p)` and `|| environment.pwsh_profile()`), `check(paths, &Integration)` (T16),
  `install` (T18), `uninstall` (T20), `write_scripts` (Pre then Post, `?`), `has_traces` (T22),
  `apply(paths, shells, pick, op) -> Result<bool, Vec<IntegrationFailure>>` (T28 minus the text) and the named
  entry `install_missing(paths, shells)` (= `install_dotfiles`). It never marks the restart reminder (T38). DTO
  `IntegrationFailure { Listing { shell: String, reason: String }, Operation { description: String, reason: String
  } }` (`application/integration_failure.rs`; `reason` = `error.to_string()`, which equals the old `{e:#}` / `{e}`
  by P3). The text is rebuilt by one presenter, `integration_failures_error(Vec<IntegrationFailure>) ->
  anyhow::Error` in `infrastructure/cli/integrations_command.rs`: Listing `"{shell}: {reason}"`, Operation
  `"{paint("1", description)}: {reason}"`, `anyhow!("\n\n{}", parts.join("\n\n"))`. `paint` is evaluated at render
  time instead of inside the loop; the stdout tty does not change within a command. `Vec<IntegrationFailure>` does
  not implement `Error`, so no caller can `?` it into anyhow without the presenter. Styling is testable: each
  presenter that paints (this one, and the plain status rows of P10) renders through a private fn taking `styled:
  bool` and building on the shared `sgr(text, code, styled)`; the public fn passes `stdout_tty()`, which is exactly
  what `paint` does. Tests pin `styled = true` with the literal bytes (`\x1b[1m…\x1b[0m`, `\x1b[32m✔\x1b[0m`,
  `\x1b[31m✘\x1b[0m`) and `styled = false`: a test comparing against `paint(..)` itself cannot fail under the gate,
  whose `cargo test` stdout is a file. Considered: higher-order domain
  methods `check_installation(paths, read)`; rejected as more machinery for the same verbatim sequence. Fitness rule 3
  flags only `*_use_case` and `infrastructure` paths, so it cannot see this application service being imported by
  another module (plan notes and the Phase 10 hand-off say so).
- **P7 Restart reminder (deviation: no VO).** `domain/repositories/restart_reminder_repository.rs`:
  `trait RestartReminderRepository { fn save_pending(&self); }` (best effort, infallible) + `#[cfg(test)]
  InMemoryRestartReminderRepository`; `infrastructure/adapters/state_restart_reminder_repository.rs`
  `StateRestartReminderRepository::new(DataDirectory)` = `mark_restart`'s body verbatim (T37; `state_file()` resolved
  per call). `InstallIntegrationsUseCase`, `ReinstallIntegrationsUseCase` and `SetUpTabsmithUseCase` mark on
  `Ok(true)` before reporting; doctor marks itself, unconditionally (T38). Phase 10 adds `find`/`delete` (and the VO
  if it then earns one). `SharedDependencies.data_directory` is `data_directory_from_env()` evaluated at dispatch,
  and nothing sets env vars between dispatch and the mark, so the resolved file is the same.
- **P8 Ports (deviation: no `UserConfirmation` port; `ShellEnvironment` is smaller; one extra port).**
  - `application/ports/shell_environment.rs` `ShellEnvironment { current_executable() -> Result<String,
    ShellIntegrationError>; parent_shell() -> Option<String>; launch_facts() -> LaunchFacts; pwsh_profile() ->
    Result<PathBuf, ShellIntegrationError> }` + a `#[cfg(test)]` recording double. `OsShellEnvironment` (unit struct)
    delegates verbatim to `exe_path`, `parent_exe`, the T9 launch-facts read (with its `cfg(windows)` branch) and the
    pwsh spawn (`pub(crate) const PWSH_PROFILE_CMD`, same name and bytes), each a call-time read.
  - `application/ports/init_script_templates.rs` `InitScriptTemplates { fn template(&self, shell, phase) -> &'static
    str }` **(extra port)**, implemented by `EmbeddedShellScripts` (the seven consts + the exhaustive match): the
    application cannot reach infrastructure consts any other way.
  - `application/ports/spec_bundle_installer.rs` `SpecBundleInstaller { fn install(&self) -> Result<(),
    ShellIntegrationError> }`, implemented in `src/bootstrap/bridges.rs` by `SpecBundleBridge` over
    `crate::specs::ensure_extracted()`, mapping `Err(e)` to `SpecBundle(format!("{e:#}"))` (Phase 5 swaps the body).
  - `application/ports/data_directory_purger.rs` `DataDirectoryPurger { fn purge(&self, directory: &Path) ->
    std::io::Result<()> }` + `FileDataDirectoryPurger` (`remove_dir_all`, `NotFound` → `Ok`; the raw io error goes
    out, T34).
  - The root refusal, the `[Y/n]` prompt, the paths resolution and the shell sets stay in the CLI handlers, in the
    literal T32/T34 order, calling the shared `is_root`/`which` directly (D2). `confirm`/`answer` move verbatim
    (anyhow) to `infrastructure/cli/user_confirmation.rs`. A `UserConfirmation` port would have no application
    caller: the prompt must come before the paths are resolved, which the handler does.
- **P9 Progressive output through a callback** (precedent `DeleteSettingsUseCase::execute(pattern, announce)`):
  `SetUpTabsmithUseCase::execute(&paths, &shells, automatic, report: impl FnMut(SetupProgress)) -> Result<bool
  /*manual*/, ShellIntegrationError>` with `SetupProgress { Installed(bool), AutomaticInstallFailed(Vec<
  IntegrationFailure>), ManualInstructions(String), ScriptFilesFailed(Vec<IntegrationFailure>) }`;
  `UninstallTabsmithUseCase::execute(&paths, &shells, purge, report: impl FnMut(UninstallProgress)) -> bool
  /*failed*/` with `UninstallProgress { IntegrationsFailed(Vec<IntegrationFailure>), PurgeFailed(String) }` (DTOs hold
  primitives, plan section 1: the use case fills it with `error.to_string()`, the CLI prints `Could not remove {data}:
  {reason}`, the same bytes as the old `{e}`; the `DataDirectoryPurger` port keeps returning `std::io::Result<()>`).
  The CLI prints each event as it arrives (println!/print!/eprintln! exactly as T32/T34), so every line keeps its
  place relative to install, mark, purge and extraction. The spec bundle error is returned after all events.
- **P10 Status** (`GetIntegrationStatusUseCase::execute(&paths, &shells) -> IntegrationStatusReport`, never fails):
  `IntegrationStatusReport { integrations: Vec<IntegrationStatus>, failures: Vec<IntegrationFailure> }`,
  `IntegrationStatus { installed, description, shell, file_name }` (primitives); the plain and JSON presenters stay
  in `integrations_command.rs` with `serde_json::json!` + `to_string`/`to_string_pretty` (T31).
- **P11 Init.** `RenderInitScriptUseCase { environment, templates }::execute(shell, phase, snapshot: bool) ->
  Result<InitScript, ShellIntegrationError>`: snapshot → `render_snapshot` (asks nothing); else
  `current_executable()?` then `render(.., || environment.parent_shell(), || decide(&environment.launch_facts())
  .launches())` (T1). `init_command::init_script(&RenderInitScriptUseCase, shell, phase)` reads
  `TABSMITH_INIT_SNAPSHOT_TEST` at call time (T6) and is the one path for `run_init_command` and `cli::debug_shell`.
  `debug_shell(shell)` keeps its signature (its tests call it directly and re-exec themselves) and builds the use case
  inline over `OsShellEnvironment` + `EmbeddedShellScripts` (legacy `cli.rs` may import infrastructure).
  `DecideLaunchUseCase::execute() -> LaunchDecision`. `_ get-shell` has no use case: its handler calls the shared
  `parent_exe` (no plan use case covers it).
- **P12 Module and bootstrap (deviation: second builder parameter).** `infrastructure/shell_integration_module.rs`:
  `ShellIntegrationModule { data_directory (private), pub(crate) install_integrations, uninstall_integrations,
  reinstall_integrations, get_integration_status, render_init_script, decide_launch, set_up_tabsmith,
  uninstall_tabsmith }` + `locate_integration_paths()`; `build_shell_integration_module(&SharedDependencies,
  Rc<dyn SpecBundleInstaller>)` only constructs (no env read beyond what `SharedDependencies` already did, no
  `current_exe`, no `which`, no spawn: D3). bootstrap builds it per arm (`Integrations`, `Init`, `Setup`,
  `Uninstall`, `_ should-launch`) with `Rc::new(SpecBundleBridge)`; `_ get-shell` needs no module.
- **P13 Repositories.** `DotfileRepository { exists(&Path) -> bool; read(&Path) -> Result<String, E>; write(&Path,
  &str) -> Result<(), E>; remove(&Path) -> Result<(), E> }` (T14 semantics) + `#[cfg(test)]
  InMemoryDotfileRepository` (missing read = `FileDoesNotExist(path)`, remove of a missing file `Ok`);
  `FileDotfileRepository` (stateless; `io_err`/`read`/`write`/`remove` moved verbatim, `exists` = `Path::exists`,
  `pub(crate) io_err` reused by the backup adapter). `BackupRepository { back_up(&self, dotfile: &Integration,
  backup_dir: &Path) -> Result<(), E> }` + `#[cfg(test)]` InMemory; `FileBackupRepository` (stateless:
  `create_dir_all(backup_dir)` keyed to the dir, `fs::copy(path, backup_dir.join(file_name()))` keyed to the dotfile,
  T18). The backup dir is `paths.data_root()/backups/<clock.now_secs()>`, computed by the installer (T36).
- **P14 Legacy during the phase.** `I` stays compiling as a shim that delegates to the new code; shims never cross
  the phase. Rust allows an inherent `impl Integration` block anywhere in the crate, so `I` keeps the I/O methods
  (`write_scripts`, `is_installed`, `install`, `has_traces`, `uninstall`, plus a `script_contents` delegate) on the
  domain entity, through its accessors, until the installer replaces them (steps 9-10). Callers switch in the step
  that removes what they use: `cli.rs` in step 13, doctor in step 10 (it builds its own `IntegrationInstaller` over
  the file adapters in a doctor-local fn; `doctor::run(args)` keeps its signature; marks through
  `StateRestartReminderRepository::new(data_directory_from_env())` at the two old call sites; `script_bins` keeps
  its signature and its override).
- **P15 Tests through a test driver.** `infrastructure/testing/{mod.rs, integration_fixtures.rs}` (declared
  `#[cfg(test)] pub(crate) mod testing;`): `ctx(dir)` (exact T51 values), `syntax_ok` (verbatim), the
  `IntegrationOnDisk` extension trait on `Integration` (`is_installed`, `install`, `uninstall`, `write_scripts`,
  `has_traces`, `script_contents`) and free fns with the legacy signatures (`integrations`, `install_dotfiles` →
  `anyhow::Result<bool>`, `uninstall_dotfiles` → `anyhow::Result<bool>`, `uninstall_all` → `i32`, `manual_text`).
  Their bodies only build the production pieces (file repositories, `SystemClock`, `OsShellEnvironment`, the
  presenter, the use case) and delegate: no branching, no formatting, so every asserted string still comes from
  production code. Locked bodies then change only where the domain forces it (struct literals → constructors, field
  reads → accessors, `Ctx { .. }` → `IntegrationPaths` methods). Same idea for two test-local helpers:
  `launch_decision(env, stdin_ok) -> (bool, String)` in the launch-policy tests and `prune_run_files(dir)` in the
  session repository tests. The 24 script tests move early (step 8) so every later step is checked against them.
- **P16 Pins and differential transcripts first (step 1).** New `should_` characterization tests for the bytes no
  test pins (pwsh script files, the full manual text with and without pwsh, the five Display texts), with expected
  literals printed from the legacy code, never retyped; plus `scripts/hexagonal-refactor/integrations-diff.sh` (a
  scrubbed transcript of setup/uninstall/reinstall/status JSON/should-launch/get-shell/init/pwsh-error runs, modelled
  on `precmd-diff.sh`; also `doctor` without `--all`, whose fixes nothing else runs (group h, T38); the `[Y/n]`
  prompts, the stdin side of the launch facts and paint's escapes on a pty (group i, T33, T9, P6); a data root outside
  HOME and HOME unset with `TABSMITH_HOME` set (group j, T11, T23, T36); directories with their modes, not only files;
  and `sleep 1` before every command that may back up an existing dotfile, because backups go to
  `backups/<now_secs>` and two backups in one second share that directory (T18), which would make the transcript
  depend on second boundaries; one command that backs up several dotfiles reads the clock per dotfile and can still,
  rarely, straddle a second, so a diff only in `backups/SECS` lines is rerun once before it counts) and
  `precmd-diff.sh` copied from `target/hexagonal-refactor/prep/phase-4/`. Every later
  step must leave `diff <(S/integrations-diff.sh BASE) <(S/integrations-diff.sh target/debug/tabsmith)` empty
  (`S=scripts/hexagonal-refactor`, `BASE=target/hexagonal-refactor/baseline-tree/target/debug/tabsmith`), and the
  session steps the same with `precmd-diff.sh`. A local tag `phase4-start` marks the phase's start for testlock.
- **P17 Session context (terminal_session).**
  - `SessionId` VO: `new(String) -> Result<SessionId, TerminalSessionError>` with exactly the T40 predicate,
    `as_str()`, `Clone, Debug, PartialEq, Eq, Hash`. The CLI builds it first and maps `Err` to `Ok(1)` silently
    (T41); pty uses `SessionId::new(sid).ok()` where it resolved the path. Generation stays in `pty.rs` (Phase 9).
  - `SessionContext` entity with its identity: `new(id, env: HashMap<String,String>, alias: Option<String>,
    reported_at_ms: u64)`, accessors, `Clone + Debug` (no `Default`, no `PartialEq`). **(deviation: accessors in place
    of `to_primitives()`, as for `Integration` in P5)**: plan section 1 lists `SessionContext` among the entities with
    `to_primitives()`; the file adapter maps the accessors into its private serde `SessionContextRecord`.
  - `TerminalSessionError { InvalidSessionId(String), Storage(Box<dyn Error + Send + Sync>) }`, Storage transparent
    like `SettingsError::Storage` (Display = inner, `source()` = inner's source): its only printer is main's anyhow
    `{:#}`, so the T42 texts stay byte-identical. `InvalidSessionId` Display `not a session id: {id}` (never printed).
  - `SessionContextRepository { save(&SessionContext) -> Result<(), E>; find(&SessionId) -> Option<SessionContext>;
    last_modified(&SessionId) -> Option<SystemTime>; remove(&SessionId); prune_older_than(Duration) }`
    **(deviation: `last_modified`, the pty's reload trigger)** + `#[cfg(test)]` InMemory (stamps `SystemTime::now()`,
    `save_modified_at` for stale arrangements, same elapsed rule) and `#[cfg(test)]` Unwritable (save fails, records
    whether prune ran).
  - `FileSessionContextRepository::new(DataDirectory)` with a private serde `SessionContextRecord` (T45 verbatim);
    file `run_dir()?/<id>.json` resolved per call (D3); save = `serde_json::to_string` + `write_private` (errors
    boxed into Storage); find/last_modified/remove swallow every error (T46); prune = `prune_run_files`' body on
    `run_dir()`, `max_age` instead of the literal, `DirEntry::metadata()`, `elapsed()` (T44; not the Clock: a
    `FixedClock` cannot age a real file and ms truncation would move the strict boundary).
  - `ReportSessionContextUseCase { Rc<dyn SessionContextRepository>, Arc<dyn Clock + Send + Sync> }::execute(id, env,
    alias)` = build with `clock.now_ms()`, `save?`, then `prune_older_than(24 h)` (the const and the prune doc
    comment live here). `infrastructure/cli/pre_cmd_command.rs` `run_pre_cmd_command(alias, &TerminalSessionModule)`:
    id check → env snapshot verbatim (T9) → execute `?` → `Ok(0)`. `TerminalSessionModule` +
    `build_terminal_session_module(&SharedDependencies)`. bootstrap's `InternalCmd::PreCmd` stays (D4).
  - Readers. The engine is the future completion module, and completion may not depend on terminal_session (plan
    section 2), so the engine never sees a `SessionContext`: the message gets its Phase 6 shape now,
    `EngineMsg::Session { env: HashMap<String, String>, alias: Option<String> }` (`ts` dropped: never read, T45).
    The engine keeps two plain fields, `session_env` (initially empty) and `session_alias` (initially `None`, together
    the old `SessionCtx::default()`), read exactly as today: the `is_empty()` fallback to the wrapper env in `env()`
    and in `compute`'s copy, `Some(text) if !text.is_empty()` in `alias_map`, both cache clears on every message. T47
    holds line for line, and `types.rs`/`engine.rs` import nothing from terminal_session. pty (the future
    terminal_session): `session_id: Option<SessionId>` + `session_contexts: Box<dyn SessionContextRepository>`
    (`FileSessionContextRepository::new(data_directory_from_env())` where the path was resolved); `poll_files` per T46,
    sending `EngineMsg::Session { env: ctx.env().clone(), alias: ctx.alias().map(str::to_owned) }` after a `Some`
    `find`; `cleanup` per T46; delete `pty::mtime`. Doctor's session check untouched (T48). `types::SessionCtx` and its
    now-unused imports go.
  - Readers are pinned first (T49): step 17 starts with characterization tests against the legacy code, committed
    green before the switch: in pty, one `Session` message per mtime change, none at an unchanged mtime, none and no
    mtime reset when the file is deleted, none (mtime updated) for bad JSON; in the engine, an empty session env falls
    back to the wrapper env, a non-empty one replaces it, alias `Some("")` and `None` give no aliases. The switch then
    changes only their arrangement. `cleanup`'s removal is checked by hand, not by a test: `cleanup` also calls
    `crate::js::kill_live_children()`, which SIGKILLs every live exec child of the test process, so a test calling it
    could kill the generators of js tests running in parallel.
  - terminal_session is **not** registered (Phase 9 finishes it); check it locally per T56.
- **P18 Registration and docs.** Step 16b deletes `I` once it is empty, with `mod integrations;` and bootstrap's
  `integrations` import (left in place it is an unused-import warning, over the clippy budget, T57). The last step
  registers `shell_integration`, records P1-P17's deviations and hand-offs in plan section 9 (Phase 4 notes, including
  that `Integration` (P5) and `SessionContext` (P17) keep read-only accessors in place of the plan's `to_primitives()`,
  that the Phase 4 review added fitness rule 7 (`src/main.rs` holds only attributes, `mod` declarations and
  `fn main`, with no `use` item, so `main` calls by full path), turned rule 6 into an allow-list (a `mod.rs` holds
  only attributes and child declarations, so no `type` alias or wrapper fn either) and made rules 1-2 read the crate
  an `extern crate .. as ..` line links, that fitness rule 3 cannot see the `IntegrationInstaller` application
  service, that `DataDirectory::shell_dir()`/`backups_dir()` stay unused, P1, and the 16 Phase 4 commit subjects over the 72-character limit, left in history:
  f5b1b99, 9f5ff3a, c6cede1, 2cfa53b, d78fd05, d39adaa, a9aa205, b33b9e6, b78bf3b, bc3929d, 2875954, step 9's
  8191e36, c6b6533, ea2dc6b, 10a30a5, and 140fad1), updates the plan's status lines and the glossary, and re-points
  the `atomic_file.rs` doc comments (111-112, 145-147) to the new test homes.

### Hand-off notes for later phases
- **Phase 5**: switch `bootstrap/bridges.rs` `SpecBundleBridge` to `specs::InstallSpecBundleUseCase`. That use case
  returns a typed error, and `{:#}` on a plain std error prints only its Display (P3, T15): the bridge must render
  the whole chain, e.g. `ShellIntegrationError::SpecBundle(format!("{:#}", anyhow::Error::from(e)))` (bootstrap may
  use anyhow) or `shared::infrastructure::error_chain::display_chain` (P3's renderer, which settings' `warning`
  also uses since the Phase 4 review; never a third hand-written copy). Check `error: could not extract the completion
  specs: creating <dir>: <io>` byte for byte (today's chain: `ensure_extracted` → `extract_to`'s `with_context`,
  `specs.rs:53`).
- **Phase 6**: `EngineMsg::Session { env, alias }` already has its target shape (P17); it is renamed with the enum
  (bootstrap's `EngineMessage`, built by `ChannelCompletionEngine`), and `ApplySessionContextUseCase` takes over the
  engine's `session_env`/`session_alias`, T47, `session_update_refreshes_first_token_items` and step 17's engine
  tests.
- **Phase 9**: `debug_shell_command` (terminal_session) may not call shell_integration: bootstrap passes a
  render callback `impl FnOnce(Shell) -> anyhow::Result<String>` built over `init_command::init_script` (precedent
  `run_settings_command`'s `print_root_help`), keeping T6's order (the shell comes from `$SHELL` inside the handler).
  `DebugCmd` moves to `bootstrap/cli.rs` and imports `ShellArg` from `shell_integration::infrastructure::cli::
  init_command`. pty's `Box<dyn SessionContextRepository>` becomes the injected port; `session_id()` generation
  moves; register terminal_session.
- **Phase 10**: doctor's own `IntegrationInstaller` wiring, its two unconditional marks and its raw session check move
  into diagnostics through the `CheckFixer`/`SystemFactsProbe` bridges (diagnostics may not use shell_integration's
  application service or use cases); `RestartReminderRepository` gains `find`/`delete` for doctor's
  reader/clearer (`doctor.rs:229-251`), still through `JsonDocumentStore` (keeps other keys). `SystemFacts` carries
  each integration's check result as `Result<(), ShellIntegrationError>` (a domain type diagnostics may import;
  doctor needs the variant: the reason text, and no fix for PermissionDenied); `ShellIntegrationError` is not
  `Clone`/`PartialEq` (`Io` holds a `std::io::Error`), so `SystemFacts` cannot derive them over it. Fitness rule 3 does
  not see application services (it flags only `*_use_case` and `infrastructure` paths): grep diagnostics for
  `shell_integration::application::integration_installer` by hand (doctor.rs builds one in Phase 4).
- **Phase 11**: `DataDirectory::shell_dir()`/`backups_dir()` are still unused (P1): either route the script and
  backup dirs through them or delete them with their two tests, and update the glossary's `DataDirectory` row.

## Target layout (files Phase 4 creates)

```
src/shell_integration/
  mod.rs
  domain/  mod.rs, shell_integration_error.rs
    entities/{mod.rs, integration.rs}
    value_objects/{mod.rs, integration_phase.rs, integration_paths.rs, integration_target.rs, dotfile.rs,
                   integration_script.rs, init_script.rs, launch_facts.rs, launch_decision.rs}
    services/{mod.rs, integration_catalog.rs, launch_policy.rs, init_script_renderer.rs,
              manual_install_instructions.rs}
    repositories/{mod.rs, dotfile_repository.rs, backup_repository.rs, restart_reminder_repository.rs}
  application/  mod.rs, integration_installer.rs, integration_failure.rs, integration_status.rs,
                integration_status_report.rs, setup_progress.rs, uninstall_progress.rs,
                {install_integrations, uninstall_integrations, reinstall_integrations, get_integration_status,
                 render_init_script, decide_launch, set_up_tabsmith, uninstall_tabsmith}_use_case.rs
    ports/{mod.rs, shell_environment.rs, init_script_templates.rs, spec_bundle_installer.rs,
           data_directory_purger.rs}
  infrastructure/  mod.rs, shell_integration_module.rs
    adapters/{mod.rs, file_dotfile_repository.rs (+ file_dotfile_repository/tests.rs), file_backup_repository.rs,
              state_restart_reminder_repository.rs, os_shell_environment.rs, file_data_directory_purger.rs,
              embedded_shell_scripts.rs + embedded_shell_scripts/tests.rs}
    cli/{mod.rs, init_command.rs, integrations_command.rs, setup_command.rs, uninstall_command.rs,
         should_launch_command.rs, get_shell_command.rs, user_confirmation.rs}
    testing/{mod.rs, integration_fixtures.rs}                    (#[cfg(test)])
src/terminal_session/
  mod.rs
  domain/  mod.rs, terminal_session_error.rs, value_objects/{mod.rs, session_id.rs},
           entities/{mod.rs, session_context.rs}, repositories/{mod.rs, session_context_repository.rs}
  application/  mod.rs, report_session_context_use_case.rs
  infrastructure/  mod.rs, terminal_session_module.rs, adapters/{mod.rs, file_session_context_repository.rs},
                   cli/{mod.rs, pre_cmd_command.rs}
src/bootstrap/bridges.rs                                        SpecBundleBridge
scripts/hexagonal-refactor/{integrations-diff.sh, precmd-diff.sh}
```
Not created (deviations, P2/P7/P8): `domain/value_objects/{integration_status, restart_reminder}.rs`,
`application/ports/user_confirmation.rs`, `infrastructure/adapters/stdin_user_confirmation.rs`.

## Glossary additions

To apply to `docs/ubiquitous-language.md` in the last step.

shell_integration (new rows):
| Term | Kind | Meaning | Replaces |
|---|---|---|---|
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

shell_integration (changed rows):
- `IntegrationStatus`: Kind **DTO**; "One row of `integrations status`: installed or not, description, shell name,
  dotfile name. (Why a check failed is a `ShellIntegrationError`.)"; Replaces "the `(bool, Integration)` rows of
  `status`".
- `IntegrationTarget`: "Dotfiles of one shell, or all: `Dotfiles(shell)` is that shell; `Dotfiles` without a shell and
  `All` are bash, zsh, fish, plus pwsh when it is on PATH."; Replaces `IntegrationTarget` (clap; now
  `IntegrationTargetArg` in the CLI).
- `IntegrationScript`: "A file tabsmith writes for an integration, `~/.tabsmith/shell/<rc>.<phase>.<ext>` or a whole
  fish `conf.d` file: path, exact contents, and the `should contain` check."; Replaces `script_path`,
  `script_contents`, `check_file`.
- `Dotfile`: append "pre block = `PRE_MARKER` + pre source line at the top (after BOM and shebang); post block =
  `POST_MARKER` + post source line at the bottom; source line `[ -f "<script>" ] && . "<script>"` (pwsh
  `if (Test-Path "<script>") { . "<script>" }`, a leading home written `${HOME}`); opt-out line `# tabsmith ignore`."
- `RestartReminder`: Kind **Flag (repository)**: "`doctor.prompt-restart-terminal` in `state.json`: saved best effort by
  `RestartReminderRepository` after an install; doctor reads and clears it."
- `IntegrationPhase`: Replaces `When` (clap mirror `IntegrationPhaseArg` in the CLI).

terminal_session (changed rows):
- `SessionId`: "Session identifier shared with the shell scripts: the `TABSMITH_SESSION_ID` the wrapper exports and the
  `run/<id>.json` file name. Valid when non-empty and made only of ASCII letters, digits, `-` and `_`, exactly what
  `_ pre-cmd` accepts; not narrowed to the hex ids the wrapper generates (D1)."; Replaces `session_id()` + the
  `_ pre-cmd` id filter.
- `SessionContext`: "The env (UTF-8 entries only) and alias text a shell last reported through `_ pre-cmd`, stamped with
  the report time in ms. Identity `SessionId` (the file name, not a field of the file). An empty env means 'use the
  wrapper's env'; an empty alias text means no aliases."

diagnostics (changed row):
- `SystemFacts`: "Snapshot of env, OS, tool versions, file existence and each integration's check result
  (`Result<(), ShellIntegrationError>`) that checks evaluate." (was "integration statuses": `IntegrationStatus` is now
  the status-row DTO.)

Words with more than one meaning (new rows):
| profile | pwsh `$PROFILE` (the pwsh dotfile, carried by `IntegrationPaths`) · `LlmProfile` |
| script | `IntegrationScript` (files tabsmith writes) · `InitScript` / `InitScriptTemplates` (what `init` prints) · `GeneratorScript` (spec) |

Words with more than one meaning (changed row; P8 creates no `UserConfirmation` port):
| prompt | shell prompt · `LlmPrompt` · the `[Y/n]` confirmation `setup`/`uninstall` ask (`confirm`/`answer` in shell_integration's `infrastructure/cli/user_confirmation.rs`; no port, not a domain type) |

No rows for repositories, adapters, records, modules or `<Module>Error` types other than `ShellIntegrationError`
(section 1 names them), consistent with the existing glossary.

## Test moves

Bodies change only as listed (all other edits are paths or imports). "Driver" = P15; it keeps the call lines
unchanged. Steps refer to the step briefs below. Every "field → accessor" rewrite drops a borrow the accessor makes
redundant: `&c.home` → `c.home()`, `&pwsh.path` → `pwsh.path()`, `&i.path` → `i.path()` (`&c.home()` is a clippy
`needless_borrow` warning and the budget is 3/3, T57).

| Test (`I` line) | Destination | Step | Restructuring |
|---|---|---|---|
| `integrations_init_snapshots` (1160), `integrations_init_syntax` (1183), `integrations_post_scripts_emit_markers` (1197) | `infrastructure/adapters/embedded_shell_scripts/tests.rs` | 8 | none (`When::*` → `IntegrationPhase::*` is path-only; `when.name()` stays). Helper `snapshot` re-pointed in step 13. |
| the 18 real-shell tests without `Ctx` (1724 … 2881, not `integrations_dotfile_bom_stays_first` at 2045: bash_prompt_command_rewrap, pwsh_post_preexec, pwsh_post_status_and_rewrap, pwsh_no_history_and_listview, zsh_rprompt_wrapped_and_kept, bash_prompt_command_array_runs_first_once, bash_preexec_hooks_run, bash_ghostty_real_integration, bash_vscode_real_integration, fish_prompt_wrappers, no_history_marker, zsh_powerlevel10k_prompt_wrapped, zsh_theme_after_our_precmd_wrapped, terminal_injected_integration_reloaded, pre_detects_ghostty_injection, zsh_async_prompt_rewrapped, bash_erasedups_keeps_reruns, bash_prompt_command_keeps_pipestatus) | same `tests.rs` | 8 | none; T52 literals byte-identical |
| `integrations_fish_confd_launches_in_a_pty` (1664) | same | 2 → 8 | step 2: `Ctx { bin: bin.clone(), fish_dir: home.join(".config/fish"), ..ctx(&dir) }` → `ctx(&dir).with_binary(bin.clone())` (the dropped `fish_dir` equals `ctx()`'s value, T51) |
| `integrations_pwsh_utf8_under_oem_code_page` (1973) | same | 2 → 8 | step 2: `Ctx { bin, ..ctx(&dir) }` → `ctx(&dir).with_binary(bin)`, `c.bin` → `c.binary()`; `PWSH_PROFILE_CMD` imported by name (inline capture) from `os_shell_environment.rs` |
| `integrations_pwsh_pre_launch_rules` (2481) | same | 2 → 8 | step 2: `Ctx { bin: bin.clone(), ..ctx(&dir) }` → `ctx(&dir).with_binary(bin.clone())` |
| `integrations_dotfile_install_uninstall` (1269) | `infrastructure/adapters/file_dotfile_repository/tests.rs` | 3 → 10 | `Integration { shell, path, file_when: None }` → `Integration::dotfile(..)`; `c.home`/`c.data` → `home()`/`data_root()` (step 2) |
| `integrations_dotfile_detection` (1331) | same | 2/3 → 10 | literal → constructor; `Ctx { bin: "/elsewhere/tabsmith".into(), ..c.clone() }` → `c.clone().with_binary("/elsewhere/tabsmith".into())`; `c.home` → `home()` |
| `integrations_enumeration_and_whole_files` (1389) | same | 2/3 → 10 | `i.path`/`pwsh.path` → `path()`; `c.home`/`c.fish_dir` → accessors |
| `integrations_dotfile_bom_stays_first` (2045) | same | 3 → 10 | `pwsh.path` → `pwsh.path()` (10 places) |
| `integrations_dotfile_write_is_atomic_install` (2929, unix) | same | 2/3 → 10 | two literals → constructors; `c.home` → `home()`; `IntegrationError::PermissionDenied` → `ShellIntegrationError::PermissionDenied` (path-only) |
| `integrations_uninstall_after_binary_moved` (2998) | same | 2 → 10 | `Ctx { bin, ..c.clone() }` → `with_binary`; `c.home` → `home()`; `has_traces` via the driver |
| `integrations_manual_lines_are_recognised` (3048) | same (one module, no D6 split) | 3 → 10 | literal → constructor; `c.home` → `home()`; `manual_text` via the driver |
| `integrations_should_launch_table` (1461, unix + `allow(clippy::type_complexity)`) | `domain/services/launch_policy.rs` | 5 | none: a test-local `launch_decision(&HashMap<String,String>, bool) -> (bool, String)` delegates to `decide(&LaunchFacts::new(..))` |
| `integrations_install_dotfiles_reports` (1440) | `infrastructure/cli/integrations_command.rs` | 14 | `c.home` → `home()` (step 2); `install_dotfiles` = driver (renders through the production presenter) |
| `integrations_confirm_eof_is_no` (2918) | `infrastructure/cli/user_confirmation.rs` | 15 | none (`answer` moves verbatim) |
| `integrations_uninstall_reports_failure` (3025, unix) | `infrastructure/cli/uninstall_command.rs` | 15 | `c.home`/`c.data` → `home()`/`data_root()` (step 2); `uninstall_all`/`install_dotfiles` = driver; the production fn must not be named `uninstall_all` |
| `integrations_pre_cmd_file_session_ctx` (1558, unix; NEW vs `ef807c1`, not locked) | `terminal_session/infrastructure/adapters/file_session_context_repository.rs` | 16a | same scenario through `save`/`find` on a repository rooted at the temp dir (file `<temp>/run/abc123.json`), same assertion via accessors; name, `cfg`, doc comment kept |
| `integrations_prune_run_files` (3080) | same | 16a | none: a test-local `prune_run_files(dir)` delegates to the adapter's private prune fn with 24 h |
| step 17 characterization tests (new, written first against the legacy readers, P17): pty `poll_files` (one message per mtime change, none when unchanged, none and mtime kept when deleted, none for bad JSON; `cleanup` is not called from a test, P17), engine `env()`/`alias_map` (empty env → wrapper env, non-empty replaces it, alias `Some("")`/`None` → none) | `pty.rs` / `engine.rs` tests | 17 | arrangement only when switching: the session file through a `FileSessionContextRepository` over a temp `DataDirectory` (same file) instead of `session_path`; `EngineMsg::Session(SessionCtx { .. })` → `EngineMsg::Session { env, alias }` |
| step 1 pins `should_describe_each_integration_failure`, `should_write_the_pwsh_script_files_byte_for_byte`, `should_print_the_manual_install_text_for_bash_and_zsh`, `should_print_the_manual_install_text_with_pwsh` | `shell_integration_error.rs` (2), `integration.rs` (3), `manual_install_instructions.rs` (4) | 1 → 2-4 | new tests; move with their subjects |
| `doctor_script_bins_skip_orphans` (`doctor.rs:829`) | stays | 2, 10 | step 2: `Ctx` literal → `IntegrationPaths::new(home.clone(), home.join(".tabsmith"), "/old/tabsmith".into(), None, home.join(".config/fish"), Some(home.join("profile.ps1")))`, `Ctx { bin, ..c }` → `c.with_binary("/new/tabsmith".into())`, `moved.data` → `moved.data_root()`; step 10: `integrations::install_dotfiles` → driver `install_dotfiles` (path-only) |
| `session_update_refreshes_first_token_items` (`engine.rs`, ~1344) | stays (Phase 6 moves it) | 17 | `EngineMsg::Session(SessionCtx::default())` → `EngineMsg::Session { env: HashMap::new(), alias: None }` (empty env, no alias: what the engine read from the default) |
| pty test helper `session()` (`pty.rs:1817`) | stays | 17 | not a test: `session_path: None` → `session_id: None, session_contexts: Box::new(InMemorySessionContextRepository::default())` |
| `debug_shell_rc_dir_and_quoting`, `debug_shell_survives_closed_terminal` (`cli.rs:759, 822`) | stay | — | none (`debug_shell` keeps its signature; D9 NAME unchanged) |
| `integrations_pre_cmd_file`, `integrations_dotfile_write_is_atomic` (`atomic_file.rs`) | stay | 18 | doc comments re-pointed only |
| `should_keep_the_shared_kernel_rule_in_test_only_code` (fixture literal `use crate::integrations::Ctx;`) | stays | — | untouched (it is rule-4 input, not an import) |

Helpers: `ALL`, `snapshot_path`, `snapshot`, `init_for`, `run_post`, `GHOSTTY`, `VSCODE_BASH` (ungated) and `pty_run`,
`fake_bin`, `bash_session` (`#[cfg(unix)]`) move privately to `embedded_shell_scripts/tests.rs` (step 8; `When` →
`IntegrationPhase` in signatures; `snapshot`'s body re-pointed in step 13); `ctx` and `syntax_ok` go to
`testing/integration_fixtures.rs` (step 8). New `should_` tests per seam are listed in each step.

## Step briefs

Each step ends with the full gate green (`GATE PASS`), the differential transcripts identical, and its commits.
The full prompts are in `phase-4-shell-integration.args.json`.

1. **Differential transcripts, pins, start tag.** Tag `phase4-start`; copy `precmd-diff.sh`, write
   `integrations-diff.sh` (groups a-j incl. doctor's fixes, the pty prompts and a data root outside HOME; both diffs
   empty, deterministic); then pin the untested bytes (P16) in `I`'s tests.
2. **Skeleton, `ShellIntegrationError`, `IntegrationPhase`, `IntegrationPaths`.** `mod shell_integration;`; move the
   error (rename); `When` → `IntegrationPhase` + clap mirror `IntegrationPhaseArg` (still in `I`); `Ctx` →
   `IntegrationPaths` (resolution stays a legacy free fn in `I`); switch `I`, `cli.rs`, doctor, tests.
3. **`Integration` entity, `IntegrationScript`, `Dotfile` (test-first).** Legacy I/O methods stay as an inherent
   `impl Integration` in `I` over accessors and the new VOs.
4. **Catalog, manual instructions, `IntegrationTarget`, `Environment` variant.** Clap target renamed
   `IntegrationTargetArg`.
5. **Launch and init rendering in the domain.** `LaunchFacts`, `LaunchDecision`, `LaunchPolicy` (+ the table test),
   `InitScript`, `InitScriptRenderer` (test-first laziness).
6. **Repositories + InMemory + file adapters.** Dotfile, backup, restart reminder; legacy helpers replaced.
7. **`ShellEnvironment` + `OsShellEnvironment`.** Paths resolution, pwsh `$PROFILE`, launch facts, executable,
   parent shell; legacy switched.
8. **`InitScriptTemplates` + `EmbeddedShellScripts` + the script tests.** Testing fixtures; the 24 tests move.
9. **`IntegrationInstaller` + `IntegrationFailure` + the failures presenter.** Legacy methods and `apply` delegate.
10. **Test driver, temp-HOME tests, doctor off the legacy methods.** Delete the inherent legacy impl.
11. **Use cases: install, uninstall, reinstall, status** (+ `IntegrationStatus`, `IntegrationStatusReport`).
12a. **Use cases: render init, decide launch.**
12b. **Use cases: set up, uninstall tabsmith** (+ `SpecBundle` variant, the two ports, purger adapter, bridge,
    progress DTOs).
13. **Module + `init`/`_ should-launch`/`_ get-shell` handlers + `cli.rs`.** Snapshot helper re-pointed.
14. **`integrations` command.** Status presenters + JSON pins; `integrations_install_dotfiles_reports` moves.
15. **`setup` + `uninstall` commands.** `user_confirmation.rs`; two tests move; `I` keeps only `pre_cmd` and
    `prune_run_files`.
16a. **terminal_session: `SessionId`, `SessionContext`, the repository and its file adapter.** Legacy `pre_cmd` saves
    and prunes through the adapter; `prune_run_files` deleted; two tests move.
16b. **terminal_session: report the session context (`_ pre-cmd`).** Use case, handler, module, bootstrap switch;
    `pre_cmd` deleted, and with it `integrations.rs`, `mod integrations;` and bootstrap's import.
17. **terminal_session: switch the readers.** Characterization tests first; `types.rs` (`EngineMsg::Session { env,
    alias }`), `engine.rs`, `pty.rs`; `SessionCtx` deleted.
18. **Register `shell_integration`, docs.** Plan section 9 notes, glossary, comments.

(The plan's six steps map as: plan 1 → steps 2-5; plan 2 → 6-7, 12b; plan 3 → 9-12b; plan 4 → 8, 13-15; plan 5 →
16a-17; plan 6 → 16b (deletion), 18.)

## Open questions (for the plan owner; the default is what the step prompts do)

1. **Name of the `Ctx` replacement.** Default `IntegrationPaths` (every field is a path, the binary included).
   Alternative `IntegrationContext` (two researchers' pick), rejected because "context" already names
   `SessionContext` and `GeneratorContext`.
2. **No `UserConfirmation` port** (P8). Default: the CLI handlers own the root refusal, the prompt, the paths
   resolution and the shell sets, so their order stays literal and `integrations_uninstall_reports_failure` keeps its
   explicit `[Zsh]` scenario; `confirm`/`answer` live in `infrastructure/cli/user_confirmation.rs`. Alternative: the
   plan's port, with the use cases resolving the paths through `ShellEnvironment` after asking.
3. **No domain `IntegrationStatus` / `RestartReminder` VOs** (P2, P7). Default: one `ShellIntegrationError` for checks
   and operations; `IntegrationStatus` re-kinded as the status-row DTO; the reminder is a repository method until
   Phase 10.
4. **Extras accepted as deviations**: the `InitScriptTemplates` port, the second parameter of
   `build_shell_integration_module`, the `IntegrationInstaller` application service, `last_modified` on
   `SessionContextRepository`. Default: accept all four.
5. **Register `terminal_session` at the end of Phase 4?** Its session-context files would pass today. Default: no
   (the plan registers a module when it is finished, Phase 9); check it locally per step instead.
6. **pwsh locally.** Five script tests and the pwsh syntax checker never run on this machine. Default: do not install
   pwsh; the step 1 pins guard the bytes and CI runs the tests on push. Installing it (`brew install --cask
   powershell`) needs the user's consent.
7. **`DataDirectory::shell_dir()`/`backups_dir()` stay unused** (P1). Default: shell_integration joins `shell/` and
   `backups/` onto `data_root()` as the legacy code does, and Phase 11 decides. Alternatives: carry both dirs in
   `IntegrationPaths` (8 constructor parameters, a clippy warning over the budget, a different T51 fixture), or
   delete the two methods with their Phase 1 tests.

## Launch

Before launch:
1. Copy `phase-4-shell-integration.md` and `phase-4-shell-integration.args.json` from
   `target/hexagonal-refactor/prep/phase-4/` into `docs/hexagonal-refactor/` (every step prompt and the review notes
   read them there, and this doc's relative links assume that folder). `precmd-diff.sh` stays where it is: step 1
   copies it into `scripts/hexagonal-refactor/`.
2. Change the plan's Phase 4 heading to `### Phase 4: shell_integration (prepared, not started:
   [research](hexagonal-refactor/phase-4-shell-integration.md))`, as Phase 3's was.
3. Commit `docs: hand Phase 4 over with its research` (trailer included) and use that commit as `startRev`, so step
   1's `phase4-start` tag includes the docs.

From the repo root, with the gate green at the end of Phase 3:

```js
Workflow({
  scriptPath: "docs/hexagonal-refactor/phase-workflow.js",
  args: /* docs/hexagonal-refactor/phase-4-shell-integration.args.json with startRev = HEAD */
})
```
