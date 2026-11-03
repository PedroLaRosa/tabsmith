# Phase 10, run 4 preamble: what runs 1-3 built (binding; where a step brief and this file differ, this file wins)

HEAD at the start of run 4 passes the gate (1975 tests, clippy 3 of 4, windows ok, golden identical); testlock vs `phase10-start`
MISSING 0 / CHANGED 0 / DUPLICATE 0. The whole diagnostics module exists except the three bootstrap bridges: domain, application
(use cases, builder, evaluator, ports + doubles), infrastructure (adapters, `DiagnosticsModule`/`build_diagnostics_module`, the three
CLI handlers). `doctor.rs` and `cli.rs` are STILL the live code: bootstrap/cli.rs still dispatches to them. The only live-code change
so far is that doctor.rs imports the moved `Spinner` from `diagnostics::infrastructure::adapters::spinner` (verbatim move, 10.13a).
Run 4 = 10.16 bridges, 10.17 rewire + the first end-to-end proof, 10.18a/b deletion, 10.19 docs, 10.20 close. This is where behaviour
CAN change: every difference found by the diff scripts is a bug in the new code (never re-baseline, never edit a baseline).

## Real names to use (read the code; these are the ones that differ from the briefs or were not in them)
- `build_diagnostics_module(dependencies: &SharedDependencies, settings: Rc<dyn SettingsRepository>, restart_reminder: Rc<dyn RestartReminderRepository>,
  integrations: Rc<dyn IntegrationProbe>, bundled_specs: Rc<dyn BundledSpecsProbe>, fixer: Rc<dyn CheckFixer>) -> DiagnosticsModule` in
  `diagnostics/infrastructure/diagnostics_module.rs`; `DiagnosticsModule { run_doctor, collect_diagnostic_report, show_recent_logs }`.
- Handlers: `run_doctor_command(args: DoctorArgs, build_module: impl FnOnce() -> DiagnosticsModule)`, `run_diagnostic_command(args: DiagnosticArgs, &DiagnosticsModule)`,
  `run_debug_logs_command(&DiagnosticsModule)`; `DoctorArgs` / `DiagnosticArgs` are `pub(crate)` in `diagnostics/infrastructure/cli/{doctor_command,diagnostic_command}.rs`.
- Ports the bridges implement: `IntegrationProbe { locate_paths, list, check, script_binaries }`, `BundledSpecsProbe::is_extracted(&Path)`, `CheckFixer::fix(&CheckFix, &IntegrationPaths) -> Result<(), String>`
  (`ports/*.rs` hold the doc comments: read them). `CheckFix::marks_restart_reminder()` is the domain rule for the reminder (errata item 2): the bridge calls `save_pending()` iff it is true, AFTER an Ok install.
- `FileRepairs::new(Arc<dyn Clock + Send + Sync>)` with `create_data_directory(&Path)` / `reset_settings(&Path)` returning `io::Result<()>`; `LenientSettings` is built inside `build_diagnostics_module` from the `settings` argument.
- The `Failed to fix: ` text per fix is produced by the bridge (`rendered(e) = format!("{:#}", anyhow::Error::from(e))` per design section 7 table), the reporter prints `Failed to fix: {reason}`. The io fixes print the BARE io Display
  (no path). `InstallMissingIntegrations` errors go through `integration_failures_error` (a "\n\n" + failure paragraphs text, bold iff stdout is a tty AT FIX TIME). Pin each variant with a bridge test.
- `script_binaries` is a VERBATIM move of `script_bins` (doctor.rs, `git show phase10-start:src/doctor.rs`): profile patched to `Some(PathBuf::new())` so pwsh is never spawned, `Shell::ALL` filtered by `which`,
  `integrations(..).unwrap_or_default()`, text between the first two `'`. The locked test `doctor_script_bins_skip_orphans` moves to the bridge tests in 10.18a (same fixtures/assertions).
  `install_missing` must get the ORIGINAL paths (pwsh_profile None) so pwsh is spawned again exactly as today.
- `phase10-greps.sh` F5 (the `Failed to fix: ` prefix, the hint line, the root message exist ONCE in production) FAILs until 10.18a deletes doctor.rs; A1-A8, F1 too. Everything else must be OK/REVIEW.
- The golden capture in gate.sh covers `doctor --all`, `diagnostic`, `debug logs` and the no-HOME lines only. The non-`--all` flow, the fixes, the spinner bytes, strict mode, the restart epilogue and the diagnostic env branches are
  covered ONLY by `scripts/hexagonal-refactor/doctor-diff.sh` (470 commands, ~3.5 min, needs cc + python3): `cargo build --locked` first, then `scripts/hexagonal-refactor/doctor-diff.sh target/debug/tabsmith > out.txt` and `cmp out.txt target/p10/doctor-diff.baseline.txt`.
  Run it with a timeout of 10 minutes. Its output is sectioned (`======== <scenario>`, `--- stderr`, `--- exit`, `--- spawns`, `## tree`): use `diff -u` to find the scenario.
- Run the doctor-diff at 10.17 (after the rewire), at 10.18a (after the deletion) and in 10.20. Any difference blocks the step.
- The reviewers of runs 1-3 found: a `mod` line dropped for five steps (E4 guards it), Windows path separators in test doubles (use `test_fixtures::logged_path`), oversized inline test modules (split to `tests.rs` over ~500 lines).

## Process rules (unchanged)
- `git commit -F <file>` only; the subject at most 72 characters (`git log -1 --format=%s | awk '{print length}'`); trailers exactly
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01D36FSrEYWhahJbdyhds8F8`. Never amend a commit that was an earlier step's; never rebase, force, tag or push.
- RED first for real, say in the report which tests you saw fail. Test names unique tree-wide: `grep -rn "fn <name>\b" src` (git grep does NOT see untracked files). Never edit an existing test's assertions; the locked tests move ONLY as listed in design section 13.
- Test doubles/fixtures: each item and each `use` only it needs carries its own `#[cfg(test)]`, never `#![cfg(test)]`. Clippy has ONE spare warning: run `cargo clippy --all-targets --locked` before the commit.
- Never run `cargo clean`. No test may exec a freshly written file (ETXTBSY) or read the real HOME/PATH/env.
- 10.19 updates docs only (plan status + Phase 10 notes, glossary, README status table + handoff, design status; `docs/SPEC.md` is Phase 11's). Copy `target/p10/steps.json` into the repo as
  `docs/hexagonal-refactor/phase-10-diagnostics.args.json` in the shape {phase, startRev, branch, reviewNotes, steps} of `phase-9-terminal-session.args.json` and `target/p10/run{2,3,4}-since.md`, `review-notes.md`, `design-errata.md`
  into `docs/hexagonal-refactor/phase-10-run-notes/` (the Phase 9 precedent).
