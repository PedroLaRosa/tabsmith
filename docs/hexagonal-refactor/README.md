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
| 8 dropdown | prepared (research, 19 step briefs in three runs; branch `refactor/hexagonal-dropdown-terminal-session`, tag `phase8-start` at `7aafcef`), see [`phase-8-dropdown.md`](phase-8-dropdown.md) | |
| 9-11 | not started | |

At `353faaa` (the registration; the docs commit that follows changes no code), `scripts/hexagonal-refactor/gate.sh`
passes. `cargo test` has 986 passed, 0 failed, 1 ignored (the baseline was 283). clippy shows 3 warnings (baseline 3).
The 131-command golden CLI capture is byte-identical to the baseline. The test lock has 0 baseline tests missing. 154
baseline tests are CHANGED, each one a reviewed restructuring (5 against the `phase7-start` tag, each in Phase 7's
"Test moves" table: `cd_validation`, `ghost_keys_and_words`, `ghost_with_control_chars_is_never_shown_or_accepted`,
`profiles_and_source` and `request_bodies`; `cd_validation` was already CHANGED against `ef807c1`, so the count is 150
plus four). Against `phase7-start` there are 43 NEW names, 0 missing and 0 DUPLICATE; against `ef807c1` too.
`inline-diff.sh`, `integrations-diff.sh` and `precmd-diff.sh` are empty against the baseline binary.
`REGISTERED_MODULES` = `completion`, `ghost_text`, `history`, `settings`, `shared`, `shell_integration`, `specs`
(terminal_session waits for Phase 9).

## Files here

- [`agent-brief.md`](agent-brief.md): the contract every implementer and reviewer agent reads first. It covers the
  one rule (behaviour must not change, tests locked), how to move code, decisions D1-D9, the gate and the commit
  format.
- [`phase-workflow.js`](phase-workflow.js): the Workflow script used for Phases 1 and 2. Each phase runs in this order:
  - one implementer agent per plan step, in order; each step is gated and committed, and the run stops on red;
  - four reviewer lenses in parallel: behaviour, test lock, architecture, completeness;
  - one skeptic per finding, who tries to refute it;
  - a fixer, then another review round, up to 3 rounds.
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
- `inline-diff.sh <binary>` (Phase 7): a scrubbed transcript of what the golden capture never shows of `tabsmith
  inline` and doctor's Inline check: valid and invalid `inline.profiles.<name>` (every reason), a selected profile,
  `set-customization` by name and through the numbered chooser on a pty, a non-bool `inline.enabled`, and every inline
  command over a corrupted `settings.json`, without a home and with only `TABSMITH_SETTINGS_PATH`. Diff it the same
  way as the two above; the diff must be empty.

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
