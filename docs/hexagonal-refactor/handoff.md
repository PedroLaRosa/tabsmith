# Hexagonal refactor: handoff (2026-11-01, after Phase 9: how to start Phase 10)

Where the refactor stands, and how to continue in a new session. Read this first, then [`README.md`](README.md),
[`agent-brief.md`](agent-brief.md), [`phase-9-terminal-session.md`](phase-9-terminal-session.md) (the Phase 9 research: traps,
decisions R1-R46, deviations, module map, hand-off notes) and the plan's section 9 "Phase 9 notes" for the conventions.
**Phase 9 is done (last code commit `b898db1`); Phase 10 is not started.** This file replaces the earlier Phase 8 handoff
(Phases 0-9 are in the plan's section 9).

## The task

The Phase 9 request was to implement the plan's Phase 9 (`terminal_session`: `pty.rs`, `tracker.rs`, `types::{Msg, Model}`
and the pty parts of `cli.rs`) on the existing branch, as a refactor: tests locked, behaviour unchanged. **That request names
no push and no pull request, so none is authorised.** The 9.7 close pushed nothing: the local remote-tracking ref
`origin/refactor/hexagonal-dropdown-terminal-session` is at `d1fdd67` (the end of run 5), so run 6 and the three docs commits
are unpushed, and no tag is pushed. Whether a PR exists for the branch was not checked (the `gh` GraphQL route is blocked
in these sessions).
**Phase 10 (diagnostics) is NOT started and must not be started without the user asking for it.** Before it, the user owes the
manual smoke below.

The Phase 8 request asked for a separate PR for this branch, stacked on PR #1 (`refactor/hexagonal-architecture` → `main`,
still open when it was written). The branch now carries Phases 8 and 9. Ask the user before any push or PR, and say in the PR
that the manual smoke is pending and that Phase 9 is in it.

## Where the work lives

- Branch **`refactor/hexagonal-dropdown-terminal-session`**, created off `refactor/hexagonal-architecture`. Stay on it; never
  commit on `main`.
- Local tags (not pushed): `phase4-start` … `phase8-start`, **`phase9-start` = `858b7b5`** (the testlock baseline of every
  Phase 9 step; `src/` equals the Phase 8 end) and **`phase9-guards` = `3fdb6e5`** (the end of run 1, the third baseline).
  A Phase 10 tag `phase10-start` would be set at the commit that starts it.
- Code ends at `b898db1`; three `docs:` commits follow (the plan, the glossary, this hand-off with the README and the
  research's Status line). `git diff b898db1 HEAD -- src` is empty.

## Binding rules

- Load the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture` before touching code
  (AGENTS.md). Commit after each finished task.
- Never rewrite committed history (no amend, rebase, reset of a commit, filter-branch, force). Fix forward.
- Commit subjects: `refactor(<module>): <why>`, `test(<module>): …`, `docs: …`, `chore(scripts): …`, at most 72 characters,
  ending with the two trailer lines `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and
  `Claude-Session: <the session link>` (agent-brief.md's Opus line is superseded). `docs/git-convention.md` (named in
  AGENTS.md) does not exist.
- **Commit with `git commit -F <file>`, not `-m`**: the RTK command hook rewrites words inside a quoted `-m` string. Measure
  the subject first: `[ "$(head -1 <file> | awk '{print length}')" -le 72 ] && git commit -F <file>`. Two Phase 9 subjects
  are over 72 (`21f1de7`, `c8629c2`); history is not rewritten.
- Exact dependency versions only; no new crates.
- **Tests are locked** against `ef807c1`: never weaken, delete, rename or `#[ignore]` one. MISSING must stay 0 and DUPLICATE 0
  (testlock keys tests by fn name). Only restructuring is allowed in a test: paths, glossary renames, moving it, splitting it
  into parts that assert the same, building the same scenario through a new constructor.
- **Behaviour must not change.** Where the plan's letter would change it, keep the behaviour and record a deviation (D1).
  Known behaviours kept in Phase 9 and reported, not fixed: P9-N1 to P9-N6 (the plan's section 9 "Phase 9 notes").
- Never edit or commit in the main working tree while a workflow's agents are committing there, and never `SendMessage` an
  agent that runs inside a workflow. Draft docs under `target/` (git-ignored) meanwhile.
- Never run cargo from outside the repo root with a relative `CARGO_TARGET_DIR`, and never leave cargo output outside
  `target/`.
- **Pushing:** nothing is authorised. A push, a tag push or a PR needs the user's explicit word.

## Tools (all run from the repo root)

Committed, in `scripts/hexagonal-refactor/`:
- `gate.sh` (`--fast` skips the golden capture): build, tests, clippy, the `cfg(windows)` check, the 131-command golden CLI
  capture and the test lock against `ef807c1`. **On Linux run `BASELINE_CLIPPY=4 scripts/hexagonal-refactor/gate.sh`** (the
  container's clippy 1.97 counts one lint more than macOS; the tree has 3 now). It needs `make-baseline.sh` once per machine.
  `GATE PASS` is required to commit.
- `testlock.py . <rev> [--diff]` (use `ef807c1`, `phase9-start` and, until Phase 10 starts, `phase9-guards`); delete
  `scripts/hexagonal-refactor/__pycache__` afterwards (or run `python3 -B`).
- `integrations-diff.sh`, `precmd-diff.sh`, `inline-diff.sh`, `theme-diff.sh`, and, new in Phase 9, **`pty-diff.py`** and
  **`session-diff.py`**: each prints a scrubbed transcript; diff the baseline binary
  `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` against `target/debug/tabsmith` (after `cargo build
  --locked`): all six diffs must be empty. On Linux put `scripts/hexagonal-refactor/compat` on PATH (the gate does it
  itself, so do not export it before calling `gate.sh`).
- `phase9-greps.sh [base-rev]` (the Phase 9 acceptance greps; a model for a `phase10-greps.sh`) and `fitness-mod.sh`.
- `cargo test --locked --release -- --ignored all_bundle_modules` (`1 passed`), `cargo check --locked --no-default-features
  --all-targets`, and `cargo test --locked debug_shell_survives_closed_terminal` alone x3 (the only automated start-up of
  the wrapper).

Committed, in `docs/hexagonal-refactor/`: `phase-workflow.js` (the per-run Workflow: one implementer per step with gate +
commit, four review lenses, a skeptic per finding, fix rounds up to 3; it takes `args.branch`), the research files and
`*.args.json` of Phases 3-9, `phase-8-run-notes/` and `phase-9-run-notes/` (the binding preambles of every run and the
review notes), `source-map/`.

Local, git-ignored, under `target/hexagonal-refactor/` (they survive the session but NOT `cargo clean`, which deletes the
whole `target/`, the baseline tree included: rebuild it with `scripts/hexagonal-refactor/make-baseline.sh`): `baseline-tree/`
(the `af1b304` worktree and its binary, needed by every diff script), `baseline/` (the golden capture), `p9/` (the Phase 9
researchers' raw notes `recon-*.md` with their compiled prototypes, the step gate logs and commit message files).

## Done

| Phase | Range | Result |
|---|---|---|
| 0-8 | up to `da2f950` | see the plan, section 9 |
| 9, guards (run 1: 9.0a-9.0d) | up to `3fdb6e5` | `terminal_session` registered, four scripts, 17 frozen fixtures, 14 loop and id tests; tag `phase9-guards` |
| 9, tracker (run 2: 9.1a-9.1e, 9.2a-9.2f) | up to `e9bf107` | `MarkerTracker`, the pure tracker domain, `OutputFilter`, `CellTagger`, `EditBufferExtractor`, the `TerminalScreen` port, `AlacrittyTerminalScreen`, `ShellOutputFeeder`; `tracker.rs` and `types::Model` gone |
| 9, session domain (run 3: 9.6a, 9.6b, 9.3a-9.3i) | up to `c8629c2` | `types::Msg` gone (`SessionMessage`), the reader, probe parsers and platform code moved; the `TerminalSession` aggregate and its entities |
| 9, ports and CLI (run 4: 9.4a-9.4c, 9.6c) | up to `6780aea` | the six runtime ports with adapters, the module injects the collaborators, the three commands out of `cli.rs` and `bootstrap/cli.rs` |
| 9, services and use cases (run 5: 9.5a-9.5h) | up to `b7338e1` | five services, ten use cases, `SessionUseCases`, the harness |
| 9, runner and close (run 6: 9.6d-9.6f, 9.7) | `f5921cb..b898db1`, then the docs | `pty.rs` became `session_runner.rs`; `probe_terminal`, `run_pty_session`, `launch_session`; tests moved; docs written from the as-built tree |

State at `b898db1` (the docs commits after it change no code; numbers measured in 9.7): full gate `GATE PASS`, 1414 passed,
0 failed, 1 ignored (`all_bundle_modules`, release run `1 passed`), clippy 3 warnings (Linux budget 4), build warnings 0, golden
identical, `windows: ok`; testlock against `ef807c1` MISSING 0, DUPLICATE 0, CHANGED 210, NEW 1133, PATH-ONLY 31; against
`phase9-start` MISSING 0, DUPLICATE 0, CHANGED 59, NEW 324, PATH-ONLY 12; against `phase9-guards` MISSING 0, DUPLICATE 0, CHANGED
61 (the 59 plus `should_generate_a_session_id_of_32_lowercase_hex_digits` and the Rig-helper rename `step_ghost` -> `llm_reply` in `should_debounce_and_show_the_llm_ghost_text_exactly_as_at_the_phase_9_start`), NEW 290; the 17 fixtures identical to `phase9-guards`;
the six diff scripts empty; `debug_shell_survives_closed_terminal` x3 green; 20 serial runs of `pty::` and
`exactly_as_at_the_phase_9_start` (44 tests) 0 failures; `cargo check --locked --no-default-features --all-targets` passes;
`phase9-greps.sh`: every A, B1-B3, B5-B7, B9-B13, E1 and F label OK, Z lists 118 files (the design's 116 plus two sliced test
modules). The dead-code scan (the `#![allow(dead_code)]` line deleted in a copy) reports 7 warnings (the 7 of the start) and 0 under
`terminal_session`.

## Next: Phase 10 (diagnostics), only if the user asks

Phase 10 has no research file yet. Its inputs are the plan's section 5 "Phase 10" and section 4 `diagnostics`,
[`source-map/doctor.md`](source-map/doctor.md) and `source-map/cli.md`, the plan's section 9 "Phase 9 notes" (the Phase 10
hand-off at its end) and the research's "Hand-off notes". Do what was done for Phases 8 and 9: recon agents per legacy area, a
planner, per-step briefs, five audit lenses, then runs, each with a binding SINCE preamble (`phase-9-run-notes/` holds the model
files). What Phase 10 starts from:
- `doctor.rs` is the one caller of `settings::infrastructure::load_or_defaults` and of `types::now_secs`. Phase 10 step 2 repeats
  `LenientLiveSettings` (terminal_session's adapter) in diagnostics' own infrastructure and deletes `load_or_defaults`; the
  `now_secs` caller may take the `Clock` port.
- `cli.rs` holds `DiagnosticArgs`, `diagnostic`, `diagnostics`, `to_toml`, `Section`, `debug_logs` (now `pub fn`, called from
  `bootstrap/cli.rs`) and the test `diagnostic_toml`; `main.rs` declares `mod cli;` and `mod doctor;` until Phase 10 deletes
  them. `debug_logs` reads `data_directory_from_env().logs_dir()`: the `logs/{cli,pty}.log` contract with
  `shared/infrastructure/logger.rs` is unchanged.
- `bootstrap::cli::terminal_session_module()` and `GeneratorProcessBridge` exist; doctor needs neither. The doctor checks that
  read the session context or the `run/` files keep their own raw checks (Phase 4 hand-off). `debug_shell_post_script`
  builds a fresh `RenderInitScriptUseCase` per call (a third wiring place).
- `terminal_session` is registered; `diagnostics` is the one module still to register (`REGISTERED_MODULES`).
- The stale rows of `docs/SPEC.md` (§4.1 `pty.rs` and `tracker.rs`, §4.3, §5.1 `has_seen_prompt` and the `ShellPath`/`TTY`/
  `User` fields, §10) are Phase 11's: do not edit SPEC.md before it.

## Waiting on the user

**The manual smoke has never been run** (agents cannot drive a terminal). Ask the user to run, with
`TABSMITH_HOME=$(mktemp -d) cargo run -- debug shell zsh` (and bash, and fish):
1. `git ch` shows the menu; Tab inserts and branches appear, a shared prefix inserts; Esc hides it.
2. `echo hi` then `ec` shows the ghost `ho hi`; Right accepts it, Alt-f one word.
3. A prompt on the bottom row draws the menu above; a resize leaves no artefacts.
4. `vim` draws no overlay; `exit 3` returns 3; `tabsmith doctor` passes.
5. Phase 8's P8-S1 (a spec whose generator runs `sleep 1` shows the spinner after about 200 ms) and P8-S2 (with a session
   running, `tabsmith theme light` and back from a second terminal with the same `TABSMITH_HOME` recolours the menu; with
   `system` in a terminal that answers OSC 11 late, the theme follows the late reply), icons in kitty or ghostty and in tmux.
6. Phase 9's P9-S1 (closing the terminal window of a `debug shell` ends it cleanly and removes the rc dir), P9-S2 (`kill -TERM`
   of the wrapper exits 129), P9-S3 (typing during start-up reaches the shell), P9-S4 (`tabsmith settings autocomplete.height
   3` from a second terminal takes effect within about 2 s) and, CI-only, P9-S5 (pwsh on Windows: the prompt-marker deferral
   and the ConPTY exit).

`session-diff.py` and `pty-diff.py` already pin bash under tmux and the real-pty start-up; the smoke covers zsh, fish, pwsh,
the icons and Windows. Phase 7's own smoke (item 4 and the LLM fake-server check) is listed in the plan's section 9 "Phase 7 notes".

## Open questions and the defaults applied (the user did not answer; record them in a PR)

Phase 9's O1-O18 (services are not use cases, one aggregate of 728 lines, `Rc` ports, no module-edge rule, `TerminalModes` as a
port, the screen built after the probe, `FrameInput.now`, a bootstrap callback for the debug shell's post script, the diff scripts
outside the gate, `LEGACY_MODULES` keeping `pty` and `tracker`, the smoke asked once, ...) are listed with their defaults in the
plan's section 9 "Phase 9 notes". Phase 8's O1-O8 are in the Phase 8 notes.

## Open risks

- pwsh is not installed locally: pwsh tests and every `cfg(windows)` path are checked by the gate's compile check and by CI only.
  `should_resize_only_the_pty_while_passing_through` opens a real pty and is not gated by `#[cfg(unix)]`: if it fails on
  windows-latest, gate it or switch to a fake `MasterPty` (that adds cfg lines, phase9-greps C1).
- Environmental flakes under gate load, never caused by the code: `integrations_zsh_powerlevel10k_prompt_wrapped` and every test
  that spawns an exec child (`specs::infrastructure::adapters::process_executor::tests::*`, a 100 ms spawn timeout). Rerun
  a lone failure by itself with `cargo test --locked <full path> -- --exact`, then the gate.
- Clippy: 3 warnings on Linux against a budget of 4, so one lint of headroom here; on macOS the baseline is one lower.
  Prune imports a move leaves unused in the same commit.
- The hash tests bake FNV values generated on macOS aarch64; they read no clock, env, file or platform code, so ubuntu and
  windows should agree, but that is reasoned, not run, until CI runs.
- Phase 11 dead-code watch list (D7): the plan's section 9 "Phase 9 notes", hand-off "Phase 11": the four `#[cfg(test)]`
  accessors and `Rc` fields of `SessionUseCases`, `SessionPortsParts: Clone` (`#[cfg(test)]`), `TerminalSession`
  (728 lines), the query-named methods that mutate, plus Phase 8's list. `#![allow(dead_code)]` stays in `main.rs` until
  Phase 11, and `LEGACY_MODULES` keeps every dead name.
- Commit `7a81d7f` (8.5a) alone has 4 clippy warnings; `5783c4b` fixes it, so a bisect landing on `7a81d7f` sees a red gate.
- `session_runner/tests.rs` is 800 lines and `launch_session` about 150 body lines (a line-for-line port by design); neither
  is split.
- rust-analyzer reports errors from half-edited files while a workflow runs: trust only cargo and the gate.
