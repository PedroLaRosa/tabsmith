# Phase 10 design errata (planner round 2)

The planner was read-only on docs/, so these amendments live here. Step 10.0 applies them to
docs/hexagonal-refactor/phase-10-diagnostics.md BEFORE committing the design doc (edit the sections named below).
The step prompts already carry the final wording; where a prompt and the old design text disagree, the prompt wins.

1. Section 5 / 4 (ports, DTOs): `RecentLogs` is replaced by `LogPaths { cli_log: PathBuf, pty_log: PathBuf }`.
   `ShowRecentLogsUseCase` has two methods: `paths(&self) -> Result<LogPaths, DiagnosticsError>` (logs_dir() first,
   no I/O besides the path) and `pty_tail(&self, paths: &LogPaths) -> Option<Vec<String>>` (reader.read + last 50
   lines). Reason: cli.rs:159-163 prints the two path lines BEFORE std::fs::read(&pty); the command keeps that order
   (matters when pty.log is a FIFO, a device or huge). `debug_logs_command`: paths()? -> println the two paths ->
   pty_tail() -> if Some: println!() + lines.
2. Section 4 (`CheckFix`): new method `marks_restart_reminder(&self) -> bool`, explicit arms:
   CreateDataDirectory false, InstallIntegration true, InstallMissingIntegrations true, ResetSettings false,
   InstallSpecBundle false (trap T18, doctor.rs:351-355 and 432-438). Section 7: `CheckFixerBridge` applies the fix
   with five explicit arms and, after an Ok, calls `restart_reminder.save_pending()` iff
   `fix.marks_restart_reminder()`. The policy is a domain rule under a pure test; the bridge only delegates.
   (script_binaries parsing stays a verbatim move, D1.)
3. Section 6 (`doctor_command`): `run_doctor_command(args, build_module: impl FnOnce() -> DiagnosticsModule)`; the
   root check runs first and the module is built only after it (so root without --all constructs nothing, by
   construction). Bootstrap passes the fn item: `run_doctor_command(a, diagnostics_module)`.
4. Section 6 (`OsDiagnosticHost`): the stdout handling of shell_version is a pure helper
   `first_line_of(stdout: &[u8]) -> Option<String>` (lossy UTF-8, lines().next(), trim; empty stdout None, blank
   first line Some("")), unit-tested without a process. No test execs a freshly written script (ETXTBSY flake,
   rust-lang/rust#114554); the process tests are: a missing program gives None.
5. Section 8 / 13 (tests): (a) use-case tests for Soft + --strict + a fix (fixed then Ok; still failing aborts with
   errors == 1 and no second Fixing); (b) a test-only decorator `LoggingRestartReminder` in application/test_fixtures.rs
   wraps a RestartReminderRepository and pushes `reminder.is_pending` / `reminder.clear_pending` into the EventLog,
   used by the epilogue order tests; (c) `doctor_group_soft_fix_abort_flow` MUST assert `run.counts()` ==
   (1,1), (1,0), (2,1) via the DoctorRun it passes to run_group, plus the probe EventLog; the DoctorRun replay in
   doctor_run.rs is the arithmetic half only.
6. Section 14 (rules): every item of a test double or fixture, including each `use` line only it needs, carries its
   own `#[cfg(test)]` (precedent terminal_session/application/test_fixtures.rs); never `#![cfg(test)]` (the fitness
   scanner does not understand it). Use named structs or type aliases for scripted maps (clippy::type_complexity).
   BASELINE_CLIPPY=4 has no headroom: run `cargo clippy --all-targets --locked` before committing in the double-heavy
   steps (10.7a, 10.7b, 10.10a, 10.10b).
7. Section 12 deviations: no new deviation is added for the debug-logs order (it is kept). The section 11 trap table
   row C9 additionally says: paths printed before the pty.log read.

Rejected findings: see openQuestions of the planner answer (early real-adapter smoke test; placeholder finding).
