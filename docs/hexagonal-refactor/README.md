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
| 6-11 | not started | |

At `d7079b9`, `scripts/hexagonal-refactor/gate.sh` passes. `cargo test` has 851 passed, 0 failed, 1 ignored (the
baseline was 283). clippy shows 3 warnings (baseline 3). The 131-command golden CLI capture is byte-identical to
the baseline. The test lock has 0 baseline tests missing. 104 baseline tests are CHANGED, each one a reviewed
restructuring (46 against the `phase5-start` tag, each in Phase 5's "Test moves" table, which has 0 missing too).
No test name is DUPLICATE, against either baseline.
`REGISTERED_MODULES` = `history`, `settings`, `shared`, `shell_integration`, `specs` (terminal_session waits for
Phase 9).

## Files here

- [`agent-brief.md`](agent-brief.md): the contract every implementer and reviewer agent reads first. It covers the
  one rule (behaviour must not change, tests locked), how to move code, decisions D1-D9, the gate and the commit
  format.
- [`phase-workflow.js`](phase-workflow.js): the Workflow script used for Phases 1 and 2. Each phase runs in this order:
  - one implementer agent per plan step, in order; each step is gated and committed, and the run stops on red;
  - four reviewer lenses in parallel: behaviour, test lock, architecture, completeness;
  - one skeptic per finding, who tries to refute it;
  - a fixer, then another review round, up to 3 rounds.
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
  most 3 warnings). It diffs the golden CLI output and runs the test-lock check. `--fast` skips the golden capture.
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
  - Phase 9: on_keys versus the decoder; one write per batch.
  - Phase 10: doctor checks evaluated over facts with re-probing after fixes.

  Each gap is written up in the matching `source-map/*.md` under "Plan gaps".
