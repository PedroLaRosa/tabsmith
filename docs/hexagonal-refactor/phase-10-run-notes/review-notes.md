# Phase 10 review notes (diagnostics), binding for reviewers, skeptics and fixers

Phase 10 moves `src/doctor.rs` (927 lines) and the diagnostic / debug-logs parts of `src/cli.rs` into the registered
module `diagnostics`. Behaviour must not change: stdout and stderr bytes and their ORDER, exit codes, files written
(bytes, modes), process spawns (count, argv, order, timing), the `logs/*.log` text, clap identifiers.

Read, in this order: `docs/hexagonal-refactor/phase-10-diagnostics.md` (the design, sections 1-17), then
`target/p10/design-errata.md` (amendments; where a step prompt and the old design differ, the step prompt and the
errata win), then `target/p10/steps.json` (the step briefs), then the recon notes `target/p10/recon-*.md` (git-ignored).
The original code is `git show phase10-start:src/doctor.rs` and `git show phase10-start:src/cli.rs`
(`phase10-start` = `b0fe04d`; before step 10.0 has created the tag use `b0fe04d`).

The design deviations (13) are accepted: CheckFix::InstallMissingIntegrations (not ReinstallIntegrations);
SystemFacts is the env group's snapshot; RestartReminderRepository gets is_pending / clear_pending (insert false,
keep the key); settings reset and data-dir creation are plain std::fs in FileRepairs; more ports and bridges than the
plan's four; extra VOs; Integration derives PartialEq, Eq; no cfg(windows) in domain / application;
`doctor_script_bins_skip_orphans` moves to the bootstrap bridge tests; `doctor_group_soft_fix_abort` is split (the
policy half keeps the name in doctor_run.rs, the flow half is `doctor_group_soft_fix_abort_flow`); `cli` and
`doctor` stay in LEGACY_MODULES; LaunchPolicy is not reused; `diagnostics` reaches specs and the installer only
through bootstrap bridges.

THE hard property: doctor is LAZY. Check, for every change, that:
1. a group is BUILT (its construction I/O: which(), pwsh $PROFILE spawn, bash login-file stats, settings load + warn
   log, which("fish")) only after the previous group passed, and BEFORE its header is printed (--all);
2. each check's facts are asked only when that check is evaluated; Skip checks and checks after an abort are never
   evaluated; a fix is followed by a fresh evaluation of that one check (the dotfile LIST is a snapshot, not re-listed);
3. the settings are loaded ONCE per run, when the environment group is built;
4. sw_vers / bash --version / fish --version spawn only when their turn comes;
5. the order of reporter events equals the order of the old println!/eprint! calls.

Test lock: every existing #[test] keeps its fn name and assertions. Allowed restructurings only: the five tests in
design section 13 (and the 3 `load_or_defaults` tests re-homed in 10.18b). `python3 -B scripts/hexagonal-refactor/testlock.py .
ef807c1` and `. phase10-start` must show MISSING 0 and DUPLICATE 0. New test names must be unique tree-wide.

Architecture: domain = std only (no anyhow / serde / clap / cfg / fs / env / Command / thread / println);
application = no infrastructure, no cfg; ports in application/ports with `#[cfg(test)]` doubles; anyhow, clap and
serde_json only in infrastructure/cli (+ bootstrap); `IntegrationInstaller`, `InstallSpecBundleUseCase`,
`embedded_spec_bundle`, `FileDotfileRepository` only in bootstrap; everything pub(crate) at most; one type per
snake_case file; mod.rs only declares children; no glob re-exports; no new crates; Cargo files untouched.
`scripts/hexagonal-refactor/phase10-greps.sh` encodes these (A and E1 FAIL until 10.18a on purpose).

Known and accepted: the manual smoke (real terminal spinner, Ctrl-C, colours, Windows) is the user's; clippy budget is
4 on Linux with no headroom (items of test doubles carry their own #[cfg(test)], never `#![cfg(test)]`).
