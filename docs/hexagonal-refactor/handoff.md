# Hexagonal refactor: handoff (2026-11-02, after Phase 10: how to start Phase 11)

Where the refactor stands, and how to continue in a new session. Read this first, then [`README.md`](README.md),
[`agent-brief.md`](agent-brief.md), [`phase-10-diagnostics.md`](phase-10-diagnostics.md) (the Phase 10 design: the lazy-doctor
problem, the file tree, traps, the 12 deviations, hand-offs) and the plan's section 9 "Phase 10 notes" for the conventions.
**Phase 10 is done (last code commit `3273a89`); Phase 11 is not started.** This file replaces the earlier Phase 9 handoff
(Phases 0-10 are in the plan's section 9).

## The task

The Phase 10 request was to implement the plan's Phase 10 (`diagnostics`: `doctor.rs`, the diagnostic and debug-logs parts of
`cli.rs`) on the new branch `refactor/hexagonal-diagnostics`, as a refactor: tests locked, behaviour unchanged. **It authorised one
new PR for that branch, off PR #1's branch (`refactor/hexagonal-architecture`); no step pushed.** Opening the PR is the
orchestrator's, after the 10.20 close; say in it that the manual smoke is pending and that Phases 8, 9 and 10 are in the diff.
Nothing else is authorised: ask the user before any other push, tag push or PR. The local remote-tracking ref
`origin/refactor/hexagonal-dropdown-terminal-session` is still at `d1fdd67` (the end of Phase 9, run 5).
**Phase 11 (cleanup and `docs/SPEC.md`) is NOT started and must not be started without the user asking for it.** Before it, the
user owes the manual smoke below.

## Where the work lives

- Branch **`refactor/hexagonal-diagnostics`**, created off `refactor/hexagonal-dropdown-terminal-session`. Stay on it; never commit
  on `main`.
- Local tags (not pushed): `phase4-start` ... `phase9-guards` and **`phase10-start` = `b0fe04d`** (the testlock baseline of every
  Phase 10 step; `src/` equals the Phase 9 end). A Phase 11 tag `phase11-start` would be set at the commit that starts it.
- Code ends at `3273a89`; one `docs:` commit follows (the plan, the glossary, this hand-off, the README, the design's Status line,
  the args file and the run notes). `git diff 3273a89 HEAD -- src` is empty.

## Binding rules

- Load the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture` before touching code
  (AGENTS.md). Commit after each finished task.
- Never rewrite committed history (no amend, rebase, reset of a commit, filter-branch, force). Fix forward.
- Commit subjects: `refactor(<module>): <why>`, `test(<module>): ...`, `docs: ...`, `chore(scripts): ...`, at most 72 characters,
  ending with the two trailer lines `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and
  `Claude-Session: <the session link>` (agent-brief.md's Opus line is superseded). `docs/git-convention.md` (named in
  AGENTS.md) does not exist.
- **Commit with `git commit -F <file>`, not `-m`**: the RTK command hook rewrites words inside a quoted `-m` string. Measure
  the subject first: `[ "$(head -1 <file> | awk '{print length}')" -le 72 ] && git commit -F <file>`. Five Phase 10 subjects are over
  72 (`7ef9712`, `8675f09`, `bebc322`, `7d7674a`, `1b2a152`) and two of Phase 9; history is not rewritten.
- Exact dependency versions only; no new crates.
- **Tests are locked** against `ef807c1`: never weaken, delete, rename or `#[ignore]` one. MISSING must stay 0 and DUPLICATE 0
  (testlock keys tests by fn name). Only restructuring is allowed in a test: paths, glossary renames, moving it, splitting it
  into parts that assert the same, building the same scenario through a new constructor.
- **Behaviour must not change.** Where the plan's letter would change it, keep the behaviour and record a deviation (D1). Known
  behaviours kept in Phase 10 and reported, not fixed, are in the plan's section 9 "Phase 10 notes" ("Noticed and not fixed").
- Never edit or commit in the main working tree while a workflow's agents are committing there, and never `SendMessage` an
  agent that runs inside a workflow. Draft docs under `target/` (git-ignored) meanwhile.
- Never run `cargo clean` (it deletes the whole `target/`: the baseline tree, the golden capture and the `target/p10/` notes), and
  never run cargo from outside the repo root with a relative `CARGO_TARGET_DIR`.
- **Pushing:** only the one PR above is authorised, and the orchestrator opens it.

## Tools (all run from the repo root)

Committed, in `scripts/hexagonal-refactor/`:
- `gate.sh` (`--fast` skips the golden capture): build, tests, clippy (default budget 3; the tree has 3), the `cfg(windows)` check
  (`windows: ok` needs the `x86_64-pc-windows-gnu` target, installed on the Linux machine of Phase 10), the 131-command golden CLI
  capture and the test lock against `ef807c1`. It needs `make-baseline.sh` once per machine. `GATE PASS` is required to commit.
- `testlock.py . <rev> [--diff]` (use `ef807c1` and `phase10-start`); run it as `python3 -B` or delete
  `scripts/hexagonal-refactor/__pycache__` afterwards.
- **`doctor-diff.sh <binary>`** (new in Phase 10): 470 doctor, diagnostic and debug-logs commands in 61 scenarios. After
  `cargo build --locked`, `scripts/hexagonal-refactor/doctor-diff.sh target/debug/tabsmith > out.txt` (about 3 minutes; needs `cc`
  and `python3`; run it with a timeout of 10 minutes) and `cmp out.txt <the transcript of the baseline binary>`. The baseline
  transcript was `target/p10/doctor-diff.baseline.txt` (11010 lines, git-ignored; regenerate it with
  `scripts/hexagonal-refactor/doctor-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith`). It is the only net for
  the non-`--all` doctor flow, the fixes, the spinner bytes, strict mode and the restart epilogue. Never re-baseline.
- **`phase10-greps.sh [base-rev]`** and `phase10-literals.py` (new in Phase 10): the acceptance greps of the design's section 15.
  At `3273a89` there is no FAIL; the REVIEW lines (B8, B14, B16-B20, C1-C3, D3-D5, E3, G1, Z) are explained in the design.
- `integrations-diff.sh`, `precmd-diff.sh`, `inline-diff.sh`, `theme-diff.sh`, `pty-diff.py` and `session-diff.py`: each prints a
  scrubbed transcript; diff the baseline binary `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` against
  `target/debug/tabsmith` (after `cargo build --locked`): all must be empty. On Linux put `scripts/hexagonal-refactor/compat` on PATH
  (the gate does it itself, so do not export it before calling `gate.sh`).
- `phase9-greps.sh [base-rev]` and `fitness-mod.sh`.
- `cargo test --locked --release -- --ignored all_bundle_modules` (`1 passed`), `cargo check --locked --no-default-features
  --all-targets`, and `cargo test --locked debug_shell_survives_closed_terminal` alone x3.

Committed, in `docs/hexagonal-refactor/`: `phase-workflow.js` (the per-run Workflow), the research files and `*.args.json` of
Phases 3-10, `phase-8-run-notes/`, `phase-9-run-notes/` and `phase-10-run-notes/` (the binding preambles of runs 2-4, the review
notes and the design errata), `source-map/`.

Local, git-ignored, under `target/hexagonal-refactor/` and `target/p10/` (they survive the session but NOT `cargo clean`): the
baseline worktree and its binary, the golden capture, the Phase 10 recon notes, the doctor-diff transcripts and the step gate logs.

## Done

| Phase | Range | Result |
|---|---|---|
| 0-9 | up to `b898db1` | see the plan, section 9 |
| 10, guards and domain (run 1: 10.0-10.6, with two review fixes and a script guard) | `eac17fb..7b81f61` | tag `phase10-start`, `phase10-greps.sh`, `doctor-diff.sh`, `diagnostics` registered, the design; `RestartReminderRepository::{is_pending, clear_pending}`; the check VOs, `DoctorRun`, `DoctorCheckCatalog`, `UsernameScrubber`, the report VOs |
| 10, application (run 2: 10.7a-10.11, with three review fixes) | `413348d..0f14b30` | the ports and doubles, `CheckGroupBuilder`, `CheckEvaluator`, `RunDoctorUseCase`, `CollectDiagnosticReportUseCase`, `ShowRecentLogsUseCase` |
| 10, infrastructure (run 3: 10.12a-10.15b) | `9b78c6e..c3e7bfe` | `OsSystemFactsProbe`, `LenientSettings`, `FileRepairs`, the spinner and Ctrl-C handler, `TerminalDoctorReporter`, `FileLogReader`, `OsDiagnosticHost`, `DiagnosticsModule`, the three commands |
| 10, bridges, rewire, deletion (run 4: 10.16-10.18b) | `e3e20a0..3273a89` | `IntegrationProbeBridge`, `BundledSpecsBridge`, `CheckFixerBridge`; `bootstrap/cli.rs` dispatches to diagnostics; `doctor.rs`, `cli.rs` and `load_or_defaults.rs` deleted; the six locked tests at their homes |
| 10, docs and close (10.19, 10.20) | the docs commit, then the close | the plan, glossary, README, this hand-off and the design status; the close re-runs the verification |

State at `3273a89` (the docs commit after it changes no code; numbers measured in 10.19): full gate `GATE PASS`, build warnings 0,
1995 passed, 0 failed, 1 ignored (`all_bundle_modules`), clippy 3 (budget 3), golden identical, `windows: ok`; testlock against
`ef807c1` MISSING 0, DUPLICATE 0, CHANGED 213, NEW 1714; against `phase10-start` MISSING 0, DUPLICATE 0, CHANGED 6 (`diagnostic_toml`,
`doctor_group_soft_fix_abort`, `doctor_script_bins_skip_orphans`, `doctor_version_parsing`, `should_fall_back_to_the_default_settings_when_they_cannot_be_loaded`,
`should_give_the_settings_the_repository_loads`), NEW 581, PATH-ONLY 1; `doctor-diff.sh` identical to the baseline transcript after the
rewire (10.17) and the deletion (10.18a); `integrations-diff.sh` and `inline-diff.sh` empty at 10.17; `phase10-greps.sh` has no FAIL
(A, B1-B7, B9-B13, B15, D1-D2, E1, E2, E4 and F are OK). The module is 64 files in `src/diagnostics` plus
`bootstrap/diagnostics_bridges.rs` (645 lines, 23 tests).

## Next: Phase 11 (cleanup and documentation), only if the user asks

Phase 11 has no research file. Its inputs are the plan's section 5 "Phase 11" and the Phase 10 notes' hand-offs in section 9. What it
starts from:
- `src/types.rs` holds only `now_ms` and `now_secs`, both without a caller: delete the file. `LEGACY_MODULES` in
  `architecture_tests.rs` lists only dead names now (`cli doctor engine icons inline integrations js keys menu parser pty render
  shell_parser spec suggest tracker types`): empty it. All ten modules are already in `REGISTERED_MODULES`, so the plan's step
  "register all 10 modules" is done; what remains is removing the exceptions.
- Remove `#![allow(dead_code)]` from `main.rs` and delete whatever the compiler flags, including `logger::trace` and `pub` items used
  only locally. The D7 watch lists are in the Phase 7, 8, 9 and 10 notes; the diagnostics list is `CheckSeverity` accessors used only
  by tests, the `OperatingSystem::Other` payload, the `InMemory*` and `Recording*` doubles, `DiagnosticsError::DataDirectory` and
  `ToolVersion::components`. Test-only items get `#[cfg(test)]` instead of deletion (D7).
- `docs/SPEC.md` is stale and is Phase 11's only: section 4.1 (the module table, including the rows for `pty.rs`, `tracker.rs`,
  `doctor.rs` and `cli.rs`, around lines 496-498), section 4.3 (message and type names), section 5.1 (it still documents
  `has_seen_prompt` and the `ShellPath`/`TTY`/`User` context fields) and section 10 (test locations).
  `docs/native-install-auto-update-plan.md` line 66 also names `doctor.rs`. The README needs no change.
- `debug_shell_post_script` is still a third wiring place of `RenderInitScriptUseCase`.

## Waiting on the user

**The manual smoke has never been run** (agents cannot drive a terminal). Ask the user to run, with
`TABSMITH_HOME=$(mktemp -d) cargo run -- debug shell zsh` (and bash, and fish) the items of the plan's section 8 (the menu on `git ch`,
Tab, Esc, the ghost text, a prompt on the bottom row, a resize, `vim`, `exit 3`), and `tabsmith doctor` passes, plus:
1. **P10-S1** `tabsmith doctor` in a real terminal shows the `Running checks...` spinner on stderr and clears it; Ctrl-C during it
   restores the cursor and exits 1.
2. **P10-S2** `tabsmith doctor --all` colours (bold headers, green and red marks) in a real terminal.
3. **P10-S3** (CI only) on Windows: `Windows Console Check`, `Windows version` and the Ctrl-C handler.
4. Phase 8's P8-S1 (a spec whose generator runs `sleep 1` shows the spinner after about 200 ms) and P8-S2 (`tabsmith theme light` and
   back from a second terminal recolours the menu; `system` follows a late OSC 11 reply), icons in kitty, ghostty and tmux.
5. Phase 9's P9-S1 (closing the terminal window of a `debug shell` ends it cleanly), P9-S2 (`kill -TERM` of the wrapper exits 129),
   P9-S3 (typing during start-up reaches the shell), P9-S4 (a settings change from a second terminal takes effect within about 2 s)
   and, CI-only, P9-S5 (pwsh on Windows).

`doctor-diff.sh` already pins the spinner clear bytes and Ctrl-C under a pty on Linux, not Windows, not macOS `sw_vers`. `session-diff.py`
and `pty-diff.py` pin bash under tmux and the real-pty start-up, not zsh, fish, pwsh, the icons or Windows.

## Open questions and the defaults applied (the user did not answer; record them in a PR)

Phase 10's (`doctor-diff.sh` stays out of the gate; `cli` and `doctor` stayed in `LEGACY_MODULES` until Phase 11; the PR is opened
after the close) are in the plan's section 9 "Phase 10 notes"; Phase 9's O1-O18 and Phase 8's O1-O8 are in their notes.

## Open risks

- **Windows compile status:** `cargo check --locked --all-targets --target x86_64-pc-windows-gnu` (inside the gate) prints
  `windows: ok`, so the `cfg(windows)` code of `os_system_facts_probe.rs` and `cursor_restorer.rs` compiles; nothing of it ran. The
  Windows console check, the Windows version check and the `SetConsoleCtrlHandler` Ctrl-C handler are covered by CI and the
  smoke only (P10-S3). pwsh is not installed locally either.
- Tests that only assert when `bash` is on PATH (`doctor_script_bins_skip_orphans` and the `script_binaries` tests of
  `bootstrap/diagnostics_bridges.rs`) pass vacuously without it; the `which` calls read the real PATH, as the locked test did.
- The diagnostics tests that compare OS-dependent io text (`Not a directory (os error 20)`) compute the expectation from the same
  std call; they were not run on Windows.
- Environmental flakes under gate load, never caused by the code: `integrations_zsh_powerlevel10k_prompt_wrapped` and every test
  that spawns an exec child (`specs::infrastructure::adapters::process_executor::tests::*`, a 100 ms spawn timeout). Rerun a lone
  failure by itself with `cargo test --locked <full path> -- --exact`, then the gate.
- Clippy: 3 warnings against a default budget of 3, so no headroom (the Linux budget of 4 has one lint). Prune imports a move leaves
  unused in the same commit.
- The hash tests bake FNV values generated on macOS aarch64; they read no clock, env, file or platform code, so ubuntu and windows
  should agree, but that is reasoned, not run, until CI runs.
- Large files left for the next touch: `bootstrap/diagnostics_bridges.rs` (645 lines, about 400 of tests),
  `run_doctor_use_case/tests.rs` (about 600 lines), `session_runner/tests.rs` (800 lines) and the 150-line body of `launch_session`.
- Commit `7a81d7f` (8.5a) alone has 4 clippy warnings; `5783c4b` fixes it, so a bisect landing on `7a81d7f` sees a red gate.
- rust-analyzer reports errors from half-edited files while a workflow runs: trust only cargo and the gate.
