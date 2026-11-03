# Hexagonal refactor: handoff (2026-11-02, after Phase 11: the plan is complete)

Where the refactor stands, and how to continue in a new session. Read this first, then [`README.md`](README.md),
[`agent-brief.md`](agent-brief.md) and the plan's section 9 "Phase 11 notes" (earlier phases' notes hold the conventions).
**Phases 0 to 11 are done (last code commit `5694ed1`); nothing in the plan is left to build.** This file replaces the earlier
Phase 10 handoff.

## The task

The Phase 11 request was to implement the plan's Phase 11 (delete `types.rs`, remove `#![allow(dead_code)]` and what the compiler
then flags, update `docs/SPEC.md`) as a refactor, tests locked and behaviour unchanged, and to open a new PR for it off PR #1's
branch (`refactor/hexagonal-architecture`). It ran on the new branch `refactor/hexagonal-cleanup`, cut from `a2cd91c` (the merge of
PR #4, which carries Phases 8 to 10). Nothing else is authorised: ask the user before any other push, tag push or PR.

## Where the work lives

- Branch **`refactor/hexagonal-cleanup`**, tag `phase11-start` = `a2cd91c` (a local tag, not pushed; its `src/` equals the end of Phase
  10). Never commit on `main`.
- Code ends at `5694ed1`; `c0fdccb` is the SPEC commit; one `docs:` commit follows (plan, glossary, README, this hand-off).
  `git diff 5694ed1 HEAD -- src` is empty.
- The sandbox clone was shallow: `git fetch --unshallow origin` restores `ef807c1` for the test lock. Tags `phase4-start` to
  `phase10-start` exist only on the machine of those phases.

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
- **Pushing:** only the Phase 11 PR above (`refactor/hexagonal-cleanup`, base `refactor/hexagonal-architecture`) was authorised.

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
| 11, cleanup and SPEC (one run) | `5eb4305..c0fdccb` | `types.rs` deleted; the crate-wide allow removed after 21 findings on six build configurations; SPEC 4.1, 4.3, 5.1, 10; reviewed by `panel-review`; B15 broken and fixed forward (`555acf3`) |

State at `5694ed1`: full gate `GATE PASS`, build warnings 0, 1995 passed, 0 failed, 1 ignored, clippy 3 (budget 3), golden identical,
`windows: ok`; testlock against `ef807c1` MISSING 0, DUPLICATE 0, CHANGED 213; against `phase11-start` 2001 tests before and after, all
five categories 0. Seven behaviour-diff transcripts byte-identical to the `af1b304` binary. `src/` changed by +27 and -74 lines.

## Next: nothing in the plan; follow-ups for the user to choose
**The only open acceptance item is the manual smoke** (below). The rest are optional, each recorded in the plan's Phase 11 notes:
- `LEGACY_MODULES` to an allow-list rule ("a domain or application path may only name a registered module"): stricter, and it
  needs one locked test fixture line edited (`crate::bootstrapping` in `should_match_whole_path_segments_only`). Deviation 1.
- A SPEC rename sweep: 17 lines in sections 3, 4.4, 4.5, 5.1.1 and 7 to 9 still say `Msg::`, `EngineMsg`, `on_event`, `ExecDone`.
- `DataDirectory::shell_dir` and `backups_dir` are `#[cfg(test)]` and duplicate the installer's own `data_root().join("backups")`:
  delete them with their tests, or use them. The lock keeps the tests, so it is the user's call.
- `docs/native-install-auto-update-plan.md` proposes a flat `src/update.rs` "like `doctor.rs`"; with this layout it should be an
  `update/` module with the three layers.
- Design calls left open since earlier phases: `rgb_to_256` against `dropdown`'s `xterm256` (O4), the `CompletionSession` regrouping,
  the 728-line `TerminalSession`, `debug_shell_post_script` as a third wiring place of `RenderInitScriptUseCase`, the large test files.
- CI runs only build and test (no clippy, no `-D warnings`, no `--no-default-features`, no Windows warning count). The six-
  configuration dead-code check of the README's "Lessons from Phase 11" is the substitute until it does.

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
