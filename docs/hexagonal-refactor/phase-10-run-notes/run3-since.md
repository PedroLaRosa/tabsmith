# Phase 10, run 3 preamble: what runs 1-2 built (binding; where a step brief and this file differ, this file wins)

HEAD at the start of run 3 passes the gate (1893 tests, clippy 3 of 4, windows ok, golden identical); testlock vs `phase10-start`
MISSING 0 / CHANGED 0 / DUPLICATE 0. `doctor.rs` and `cli.rs` are STILL the live code and untouched; nothing in `src/diagnostics`
has a production caller yet (`#![allow(dead_code)]` covers it). Run 3 builds the infrastructure ADAPTERS and the CLI handlers of
diagnostics (10.12a-10.15b); the bootstrap bridges (10.16) and the rewire (10.17) come in run 4, so in run 3 `bootstrap/cli.rs` is NOT
changed and the CLI behaviour cannot change. The step briefs of run 3 were written before runs 1-2 existed: read the REAL ports in
`src/diagnostics/application/ports/*.rs`, the use cases and the domain before you implement anything, and use their real names.

## Real names and contracts that differ from or add to the design text
- `DiagnosticHost::env_text` was RENAMED `non_empty_env` (trait, InMemoryDiagnosticHost, callers): non-empty UTF-8 env text. Nothing else about the
  trait changed; `OsDiagnosticHost` implements: user, non_empty_env, environment_variables, shell_version, current_dir, current_exe, in_wsl, version, platform.
- `SystemFactsProbe` contracts (doc comments on the trait are binding): `directory_exists` = `Path::is_dir` (follows symlinks; a FILE of that name is not a
  directory); `file_exists` = `Path::is_file` (a directory of that name is not a file); `env_is_non_empty` = `var_os` set and not empty;
  `env_text` keeps an empty value; `settings_file_loads` = the settings document store loads the file. Cover them in `OsSystemFactsProbe` tests with TempDir
  (a directory named `<sid>.json`, a regular file at the data-root path).
- `tabsmith_version()` of the probe MUST return `env!("CARGO_PKG_VERSION")` (an empty version makes an empty `TABSMITH_TERM` count as Ok).
- `CheckFixer::fix(&CheckFix, &IntegrationPaths) -> Result<(), String>` receives the ORIGINAL paths (pwsh_profile None); `CheckFix::marks_restart_reminder`
  exists in the domain (errata item 2); the bridge (run 4) applies it. `RecordingCheckFixer` records the paths it got.
- `LogPaths { cli_log, pty_log }` and `ShowRecentLogsUseCase::{paths, pty_tail}` (errata item 1): `debug_logs_command` = `paths()?` -> print the two path lines -> `pty_tail(&paths)`
  -> only for `Some(lines)`: `println!()` then each line. The path lines are printed BEFORE the pty.log read.
- `CollectDiagnosticReportUseCase` returns the domain `DiagnosticReport` (`DiagnosticSection`s of `(String, DiagnosticValue)` rows; Text / Flag / Missing). `to_toml` and the
  JSON map live in `infrastructure/cli/diagnostic_command.rs` (TOML: insertion order, `Missing` rows dropped; JSON: a `serde_json::Map`, keys ALPHABETICAL, `Missing` as null).
- `RunDoctorUseCase::new(builder, evaluator, fixer, reporter, system, integrations, restart_reminder)`; `execute(DoctorRequest) -> Result<DoctorVerdict, DiagnosticsError>`.
  `CheckGroupBuilder::new(system, integrations, settings)`, `CheckEvaluator::new(system, integrations, bundled_specs)`. `DoctorRequest::new(all, strict)`.
- The reporter methods (see `ports/doctor_reporter.rs`) are one `println!`/`eprint!` of today each; the use case calls `start_progress`/`stop_progress`, `group_header`, `outcome`,
  `fixing`, `fix_failed`, `rechecking`, `blank_line`, `doctor_found_errors`, `everything_looks_good`, `support_hint`, `restart_notice`. The reporter owns the spinner.
- Test doubles that log a path use `test_fixtures::logged_path` (forward slashes) so tests pass on windows-latest CI too; any new double does the same. Tests that need a Windows
  path separator must not hard-code `/` for a `Path::join`.
- The fitness test now treats `<what>_tests.rs` below a module as a sibling test file (run 2 fix). Test modules over ~500 lines go to a sibling `tests.rs`.
- `phase10-greps.sh` B13 was fixed (the DiagnosticHost method names `current_dir`/`current_exe` are allowed); E4 (every .rs declared by a mod) exists. Run
  `scripts/hexagonal-refactor/phase10-greps.sh` after every step; only the A and F1/F3 FAILs may remain until 10.18a.
- `legacy` facts to keep verbatim in the adapters: `OsDiagnosticHost::user` uses `cfg!(windows)` (USERNAME) else USER; `in_wsl` is `cfg!(target_os="linux") && (WSL_DISTRO_NAME non-empty
  || /proc/sys/kernel/osrelease lowercased contains "microsoft")`; `environment_variables` = `std::env::vars_os()` with non-UTF-8 KEYS dropped and values lossy, unfiltered and
  unsorted; `shell_version` = `Command::new($SHELL).arg("--version").output()` (NO timeout), stdout only, lossy, first line trimmed via `first_line_of` (blank first line = `Some("")`,
  empty stdout = `None`). `OsSystemFactsProbe::tool_version` uses `output_with_timeout(.., 5 s)` as doctor.rs does; `console_attached` / `windows_build` carry the
  `#[cfg(windows)]` bodies verbatim and a `#[cfg(not(windows))]` twin, in the same file as `version_from`.
- Spinner and `restore_cursor_on_ctrl_c` are MOVED verbatim (10.13a), the diff against `git show phase10-start:src/doctor.rs` of the moved block must be empty apart from `use` paths/visibility.
  The Windows compile check (`windows: ok` in the gate) covers the cfg(windows) hunks; still diff them by eye.

## Process rules (unchanged from run 2)
- `git commit -F <file>` only; the subject at most 72 characters (`git log -1 --format=%s | awk '{print length}'`); trailers exactly
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01D36FSrEYWhahJbdyhds8F8`.
- RED first for real, say in the report which tests you saw fail. Test names unique tree-wide: check with `grep -rn "fn <name>\b" src` (git grep does NOT see untracked files).
- Test doubles/fixtures: every item and each `use` only it needs carries its own `#[cfg(test)]`, never `#![cfg(test)]`. Clippy has ONE spare warning: run `cargo clippy --all-targets --locked` before the commit.
- Never run `cargo clean`. `docs/git-convention.md` does not exist; agent-brief.md and design section 14 are the commit rules.
- Adapters touch the real world: NO test of an adapter may read the real env/HOME/PATH or exec a freshly written script (ETXTBSY); use TempDir, pure helpers and a missing-program case.
