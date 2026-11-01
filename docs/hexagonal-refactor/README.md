# Hexagonal refactor: working notes and tools

Companion to [`../hexagonal-refactor-plan.md`](../hexagonal-refactor-plan.md) (the plan, with its section 9
decisions) and [`../ubiquitous-language.md`](../ubiquitous-language.md) (the names). This folder holds what an
implementation session needs that the plan does not.

## Status

| Phase | State | Commits |
|---|---|---|
| 0 baseline | done | `af1b304` |
| 1 composition root, fitness test, shared kernel | done | `af1b304..52dcf85` |
| 2 settings | done | `52dcf85..c7b796e` |
| 3 history | done | `a3953cf..f485ea7`, see [`phase-3-history.md`](phase-3-history.md) |
| 4 shell_integration | done (also starts terminal_session's session context) | `6f38da1..ab7052b`, see [`phase-4-shell-integration.md`](phase-4-shell-integration.md) |
| 5 specs | done, run in five chunks (5.1, 5.2, 5.3a-d, 5.3e-g, 5.4-5.5) | `1a466e2..d7079b9` (tag `phase5-start` at `1a466e2`), see [`phase-5-specs.md`](phase-5-specs.md) |
| 6 completion | done, run in six chunks (6.0a-6.2b; 6.3a-e; 6.2c-f and 6.6a; 6.4a-d; 6.4e-f and 6.5a-b; 6.5c, 6.4g, 6.6b and 6.7) | `c38596c..7f48161`, registration at `f3fc30a` (tag `phase6-start` at `c38596c`, whose `src/` equals `ff2db5a`), see [`phase-6-completion.md`](phase-6-completion.md) |
| 7 ghost_text | done, run in three chunks (7.0-7.4, 7.5-7.7, 7.8-7.10) | `c38dc4b..ab50884`, registration at `353faaa` (tag `phase7-start` at `eef4c88`, whose `src/` equals `7f48161`), see [`phase-7-ghost-text.md`](phase-7-ghost-text.md) |
| 8 dropdown | done, run in three chunks (8.0a-8.2c and 8.4a; 8.4b; 8.3a-8.7) | `bb7c6b7..1bbe27b`, registration at `da2f950` (tag `phase8-start` at `7aafcef`), see [`phase-8-dropdown.md`](phase-8-dropdown.md) |
| 9 terminal_session | done, run in six runs (9.0a-9.0d; 9.1a-9.2f; 9.6a-b and 9.3a-i; 9.4a-c and 9.6c; 9.5a-h; 9.6d-f and 9.7) | `858b7b5..b898db1`, registered first at 9.0a (tags `phase9-start` at `858b7b5` and `phase9-guards` at `3fdb6e5`), see [`phase-9-terminal-session.md`](phase-9-terminal-session.md) |
| 10-11 | not started | |

At `b898db1` (the last code commit of Phase 9; the docs commits that follow change no code), `scripts/hexagonal-refactor/gate.sh`
passes (on Linux as `BASELINE_CLIPPY=4 scripts/hexagonal-refactor/gate.sh`). `cargo test` has 1414 passed, 0 failed, 1 ignored (the
baseline was 283; 1091 at the end of Phase 8). clippy shows 3 warnings (the Linux container's clippy 1.97 counts one more
lint than macOS, so the Linux budget is 4). The 131-command golden CLI capture is byte-identical to the baseline and
`cargo check --all-targets --target x86_64-pc-windows-gnu` prints `windows: ok`. The test lock against `ef807c1` has 0 baseline
tests missing, 0 DUPLICATE, 210 CHANGED (each one a reviewed restructuring; 168 at the start of Phase 9) and 1133 NEW
names. Against `phase9-start`: 0 missing, 0 DUPLICATE, 59 CHANGED (all in Phase 9's allowed list), 324 NEW names and 12
PATH-ONLY; against `phase9-guards`: 0 missing, 0 DUPLICATE, 61 CHANGED (the 59 plus two guard tests: the session-id test and a Rig-helper rename), 290 NEW. The 17
characterisation fixtures are byte-identical to `phase9-guards`. `theme-diff.sh`, `inline-diff.sh`,
`integrations-diff.sh`, `precmd-diff.sh`, `pty-diff.py` and `session-diff.py` are empty against the baseline binary.
`REGISTERED_MODULES` = `completion`, `dropdown`, `ghost_text`, `history`, `settings`, `shared`, `shell_integration`,
`specs`, `terminal_session` (`diagnostics` waits for Phase 10). `src/pty.rs` and `src/tracker.rs` are gone and `src/types.rs`
holds only `now_ms` and `now_secs`. The manual smoke (plan section 8) has never been run and is the user's.

## Files here

- [`agent-brief.md`](agent-brief.md): the contract every implementer and reviewer agent reads first. It covers the
  one rule (behaviour must not change, tests locked), how to move code, decisions D1-D9, the gate and the commit
  format.
- [`phase-workflow.js`](phase-workflow.js): the Workflow script used for Phases 1 and 2. Each phase runs in this order:
  - one implementer agent per plan step, in order; each step is gated and committed, and the run stops on red;
  - four reviewer lenses in parallel: behaviour, test lock, architecture, completeness;
  - one skeptic per finding, who tries to refute it;
  - a fixer, then another review round, up to 3 rounds.
- [`phase-9-terminal-session.md`](phase-9-terminal-session.md) and
  [`phase-9-terminal-session.args.json`](phase-9-terminal-session.args.json): Phase 9 research (traps P9-T1..T101, decisions
  R1-R46, 30 deviations, the module map, signatures, test moves, the guard design, open questions O1-O18), the 42 step
  briefs (`9.0a` ... `9.7`, six runs) and, in [`phase-9-run-notes/`](phase-9-run-notes/), the binding preambles of every
  run (`phase9-common-since.md`, `phase9-run<N>-since.md`, `-scope.md`), the review notes of runs 1, 3, 4 and 5 (accepted
  deviations and watch lists), the list of the 168 `ef807c1` CHANGED names at the launch and the run-end list.
- [`phase-8-dropdown.md`](phase-8-dropdown.md) and [`phase-8-dropdown.args.json`](phase-8-dropdown.args.json):
  Phase 8 research (traps P8-T1..., decisions R1-R30, rechecks K1-K22, deviations, test moves, the `theme-diff.sh` and
  byte-hash guard design) and the 19 step briefs (`8.0a` ... `8.7`, three runs: 8.0a-8.2c + 8.4a, 8.4b alone, 8.3a-8.7), each
  audited by five read-only lenses against `7aafcef`.
- [`phase-7-ghost-text.md`](phase-7-ghost-text.md) and [`phase-7-ghost-text.args.json`](phase-7-ghost-text.args.json):
  Phase 7 research (traps P7-T1..., decisions R1-R24, rechecks K1-K16, RC1-RC8), the 11 step briefs (`7.0` ... `7.10`, three
  runs: 7.0-7.4, 7.5-7.7, 7.8-7.10) and their review notes. Written at `ff2db5a`, re-checked at `058bebd`; the binding
  corrections are the last section of the `.md`.
- [`phase-6-completion.md`](phase-6-completion.md) and [`phase-6-completion.args.json`](phase-6-completion.args.json):
  Phase 6 research (decisions Q1-Q33, deviations 1-38, the traps, test moves), the 28 step briefs (`6.0a` … `6.7`)
  and their review notes, re-checked at `ff2db5a`; it was run in six chunks (6.0a-6.2b; 6.3a-e; 6.2c-f and 6.6a;
  6.4a-d; 6.4e-f and 6.5a-b; 6.5c, 6.4g, 6.6b and 6.7).
- [`phase-5-specs.md`](phase-5-specs.md) and [`phase-5-specs.args.json`](phase-5-specs.args.json): Phase 5
  research (decisions P1-P18, deviations 1-26), the 17 step briefs (`5.1a` … `5.5`) and their review notes,
  re-checked at `0a0a74e`; it was run in five chunks (5.1, 5.2, 5.3a-d, 5.3e-g, 5.4-5.5).
- [`phase-4-shell-integration.md`](phase-4-shell-integration.md) and
  [`phase-4-shell-integration.args.json`](phase-4-shell-integration.args.json): Phase 4 research (decisions P1-P18,
  traps T1-T57), step briefs and launch arguments; it is run in three chunks (steps 1-8, 9-12b, 13-18).
- [`phase-3-history.md`](phase-3-history.md) and [`phase-3-history.args.json`](phase-3-history.args.json): Phase 3
  research, step briefs and the exact launch arguments.
- [`source-map/`](source-map/): a read-only pass over all 21 legacy files at `af1b304`. For each file it gives:
  - where every item goes;
  - where every test goes;
  - **behaviour traps**, the code-cited subtleties a refactor silently breaks;
  - **plan gaps**, the places where the plan is silent or would change behaviour if followed literally.

  Read the file for every legacy file a phase touches before writing the step briefs. The settings, shared and
  bootstrap parts are already done. Line numbers refer to `af1b304`.

## Tools (`scripts/hexagonal-refactor/`)

Run everything from the repo root. Output goes to `target/hexagonal-refactor/` (git-ignored).

- `make-baseline.sh`: run this once per machine. It adds a worktree of `af1b304` at
  `target/hexagonal-refactor/baseline-tree`, builds it and captures the golden baseline.
- `gate.sh`: run before every commit. It builds, runs the tests (at least 283 passed, 0 failed) and runs clippy (at
  most `BASELINE_CLIPPY` warnings: 3 by default, `BASELINE_CLIPPY=4` on Linux). It compile-checks the `cfg(windows)` code
  (`cargo check --all-targets --target x86_64-pc-windows-gnu`, must print `windows: ok`), diffs the golden CLI output and runs
  the test-lock check. `--fast` skips the golden capture. On Linux it puts the `compat/` `stat` and `sed` shims on PATH.
  Failing test output is kept in `target/hexagonal-refactor/test-failures/`.
- `golden.sh <binary> <out>`: runs 131 non-interactive commands in an isolated `HOME`, with sandbox, cwd and binary
  paths scrubbed. The commands cover help, init, settings, theme, inline, integrations with the written dotfiles
  and their modes, completion, diagnostic, `doctor --all`, error paths and no-`HOME` runs. It is macOS only
  (`stat -f`).
- `testlock.py <repo> ef807c1 [--diff]`: compares every `#[test]` body with the baseline, ignoring paths. It lists
  MISSING, NEW, PATH-ONLY and CHANGED tests, and every test name defined twice (DUPLICATE).
- `integrations-diff.sh <binary>` and `precmd-diff.sh <binary>` (Phase 4): scrubbed transcripts of the integration
  commands and of `_ pre-cmd`. Diff each one run on the baseline binary
  (`target/hexagonal-refactor/baseline-tree/target/debug/tabsmith`) against `target/debug/tabsmith`; the diff must
  be empty.
- `inline-diff.sh <binary>` (Phase 7): a scrubbed transcript of what the golden capture never shows of `tabsmith
  inline` and doctor's Inline check: valid and invalid `inline.profiles.<name>` (every reason), a selected profile,
  `set-customization` by name and through the numbered chooser on a pty, a non-bool `inline.enabled`, and every inline
  command over a corrupted `settings.json`, without a home and with only `TABSMITH_SETTINGS_PATH`. Diff it the same
  way as the two above; the diff must be empty.
- `theme-diff.sh <binary>` (Phase 8): a scrubbed transcript of what the golden capture never shows of `tabsmith theme`:
  the listing over every kind of file and folder, `--folder`, valid custom themes (the banner and the saved
  `settings.json`), every `invalid theme file` text of the strict parser, built-in names against theme files of the same
  name, every `settings.json` state, the save-after-banner failure, the display of non-string values, the bold escapes on
  a pty and the clap conflicts. Diff it the same way as the three above; the diff must be empty.
- `pty-diff.py <abs binary>` (Phase 9): drives `tabsmith _ pty` on a real pty through ten scenarios (probe answered or not,
  typeahead during the probe, env and cwd, a deleted cwd, SIGTERM to 129, `exit 3`, no shell, not a tty, a corrupt
  settings file warning, the `-vv` info line) and prints scrubbed bytes, run files and log lines. About 12 s. Diff the
  baseline binary against the new one (`diff <(python3 -I pty-diff.py $OLD) <(python3 -I pty-diff.py $NEW)`); empty.
- `session-diff.py <abs binary> [scenario ...]` (Phase 9): drives `debug shell bash` inside a detached tmux server (80x24)
  with scripted keystrokes and prints what a user would see after each step (startup, menu, ghost, bottom row and resize,
  `vim` and `exit 3`, settings reload, key tester, failures, files). Needs python3, tmux, bash, vim and git; about 85 s.
  Diff it the same way; empty. It does not cover zsh, fish, pwsh, kitty/ghostty/tmux icons or Windows (the smoke).
- `phase9-greps.sh [base-rev]` (Phase 9): the acceptance greps A-F and Z of the Phase 9 research, one OK / FAIL / REVIEW
  line each (the legacy files and names are gone, layers and dependencies of `terminal_session`, the cfg multiset against
  `phase9-start`, the panic injection and the one `#[ignore]`, the registration, names that live only inside the module,
  the file list). Read every FAIL and REVIEW line.
- `fitness-mod.sh [--worktree] [module ...]` (Phase 9): runs the architecture test on a copy of HEAD or of the working
  tree with other modules registered or a candidate file dropped into a layer folder. After Phase 9 `diagnostics` is the
  one module left to register (Phase 10 or 11).
- The local tags `phase9-start` (`858b7b5`) and `phase9-guards` (`3fdb6e5`) are the second and third `testlock.py` baselines of
  Phase 9 (`testlock.py . phase9-start --diff`, `. phase9-guards --diff`); `git diff phase9-guards --
  tests/fixtures/terminal_session/characterisation` must print nothing. Both tags are local.

## Lessons from Phases 1-2

- Do not `SendMessage` an agent that runs inside a workflow. It resumes a second copy that writes to the same tree.
- Only `aarch64-apple-darwin` is installed, so `cfg(windows)` and `cfg(target_os = "linux")` code is checked by hand
  and by CI only.
- One gate run in Phase 2 reported a single failing test that could not be reproduced: 0 failures in 20 sequential
  runs and in 40 runs under 8-way parallel load. The gate now keeps failing output, so the next occurrence names the
  test.
- The source map found plan gaps that later phases must settle before writing their briefs:
  - Phase 4: `Ctx` has no home in the plan; `debug shell` needs shell_integration from terminal_session; the
    `When`/`IntegrationTarget` clap mirrors.
  - Phase 5: the typed `SpecRuntime` versus JSON results; `RuntimeEventSource` ownership of the deferred queue.
  - Phase 6: the engine's ghost text recompute on every reply; `disableForCommands` stays on every spec load.
  - Phase 8: two theme parsers; icon bytes versus icon ids across modules.
  - Phase 9: on_keys versus the decoder; one write per batch (as built: one `emit` per event, not one buffer).
  - Phase 10: doctor checks evaluated over facts with re-probing after fixes.

  Each gap is written up in the matching `source-map/*.md` under "Plan gaps".

## Lessons from Phase 9

- Pin the behaviour before moving anything: 17 text-fixture characterisation tests (written once, then frozen), 14 loop and
  id tests, and two real-session diff scripts (`pty-diff.py` on a pty, `session-diff.py` under tmux) held every later step
  to the old binary's output. They caught what 1400 unit tests could not.
- A refactor of about 4700 lines and 74 tests ran as 42 steps in six runs of four to eleven steps, each gated, committed and
  reviewed by the orchestrator between runs with a binding preamble that carried the as-built facts forward. Code that
  moves in several steps needs interim composites (the legacy `Tracker`, public aggregate fields, `Session` delegates);
  name each one, the step that kills it, and a grep that proves it died. Two did not die on schedule (the delegates went at
  9.6d, not 9.5h; the four `#[cfg(test)]` accessors of `SessionUseCases` stayed), so check the death list at the last step.
- Registering the module in the architecture test at the first step, not the last, made every later move rule-checked. Rule
  scanners read string and char literals as code (a `b"\x1b["` literal shifts the scan): write `\x5b`.
- Test support belongs in one harness. The 14 session scenarios ran against a hand-rolled rig until 9.6f rebuilt them over
  `TerminalSessionHarness`; the swap needed `SessionPortsParts: Clone` (kept under `#[cfg(test)]`), a cost to price in first.
- A test module over about 500 lines is sliced into `<file>/tests.rs`; two modules both named `tests` cannot share one
  file, which reordered two commits of 9.6f.
- The agent that drives the commits should write messages with `git commit -F <file>`, not `-m`: the RTK command hook
  rewrites words inside a quoted `-m` string.
- The cfg(windows) code cannot run here, but `cargo check --target x86_64-pc-windows-gnu` compiles it inside the gate: add
  that check before the first move of platform code, not after.
- Known flakes under gate load: `integrations_zsh_powerlevel10k_prompt_wrapped` and every test that spawns an exec child
  (`specs::...::process_executor::tests::*`); rerun a lone failure by itself with `--exact`, then the gate.
