# Hexagonal refactor: handoff (2026-10-29)

Where the refactor stands, how it got there, and what is left of Phase 5. Read this first, then
[`README.md`](README.md), [`agent-brief.md`](agent-brief.md) and [`phase-5-specs.md`](phase-5-specs.md).

## The task

The user asked for phases 3, 4 and 5 of [the plan](../hexagonal-refactor-plan.md), using the `hexagonal-architecture`
skill as reference. It is a refactor: behaviour must not change, and tests change only when they have to be moved or
restructured (moves, renames, splits, constructors that build the same state). Phases 3 and 4 are done. Phase 5 is
two runs out of five in.

The user turned ultracode on for the session (`/effort ultracode on`): every substantive task runs as a Workflow.

## Binding rules

- Load the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture` before working
  (AGENTS.md). Commit after each finished task.
- Work on branch `refactor/hexagonal-architecture`, never `main`. **Never push**; ask the user first. Nothing is
  pushed, including the local tags `phase4-start` (`cec000e`) and `phase5-start` (`1a466e2`).
- Never rewrite committed history: no amend, rebase, reset of a commit, filter-branch or force. Fix forward.
- Commit subjects: `refactor(<module>): <why>`, `test(<module>): …`, `docs: …` or `chore(scripts): …`, at most 72
  characters, ending with the trailer `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
  docs/git-convention.md (named in AGENTS.md) does not exist; agent-brief.md's commit rules apply.
- **Commit with `git commit -F <file>`, not `-m`.** The RTK command hook rewrites words such as `make` even inside a
  quoted `-m` string: `f8e6055`'s subject reads `rtk make specs …` because of it.
- Exact dependency versions only; no new crates.
- Tests are locked against `ef807c1`: never weaken, delete, rename or `#[ignore]` one. MISSING must stay 0 and
  DUPLICATE 0 (testlock keys tests by fn name, so a duplicate name hides one copy).
- Do not edit or commit in the main working tree while a workflow's agents are committing there, and never
  `SendMessage` an agent that runs inside a workflow.

## Tools

- `scripts/hexagonal-refactor/gate.sh` (`--fast` skips the golden capture). It checks build, tests, clippy (budget 3
  warnings), the 131-command golden CLI capture, and the test lock against `ef807c1` (MISSING must be 0; it prints a
  DUPLICATE line but does not fail on it).
- `python3 scripts/hexagonal-refactor/testlock.py . <rev> [--diff]`. Delete `scripts/hexagonal-refactor/__pycache__`
  afterwards.
- `integrations-diff.sh <bin>` and `precmd-diff.sh <bin>` (scripts/hexagonal-refactor). Diff each one's output for the
  baseline binary `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` against `target/debug/tabsmith`
  (after `cargo build --locked`). Both diffs must be empty.
- `cargo test --locked --release -- --ignored all_bundle_modules` (must print `1 passed`).
- The failing-extraction sandbox, once per binary, in a fresh sandbox: `SB=$(mktemp -d); mkdir -p "$SB/home"
  "$SB/data"; : > "$SB/data/specs"`. Then `env -i HOME="$SB/home" TABSMITH_HOME="$SB/data" PATH=/usr/bin:/bin <bin>
  setup --no-confirm </dev/null` must print `error: could not extract the completion specs: creating $SB/data/specs:
  File exists (os error 17)`. With the same env, `<bin> doctor </dev/null` must print `Failed to fix: creating
  $SB/data/specs: File exists (os error 17)`.
- The local fitness check with `specs` registered: copy the tree (`git archive HEAD | tar -x -C <dir>` under
  `target/hexagonal-refactor/review-tmp/`), add `"specs"` to `REGISTERED_MODULES` in its `src/architecture_tests.rs`,
  run `cargo test --locked architecture` there, then delete the copy.
- [`phase-workflow.js`](phase-workflow.js): the per-phase Workflow. Args are `{phase, startRev, steps:[{id,title,prompt}],
  reviewNotes}`. It runs one implementer per step (gate + commit each, stopping on red), then four review lenses
  (behaviour, test-lock, architecture, completeness), a skeptic per finding, and fix rounds (up to 3). Implementers get
  agent-brief.md plus their step prompt; only reviewers get `reviewNotes`.

## Done

| Phase | Range | Result |
|---|---|---|
| 0 baseline | `af1b304` | |
| 1 composition root, fitness test, shared kernel | `af1b304..52dcf85` | |
| 2 settings | `52dcf85..c7b796e` | |
| 3 history | `a3953cf..f485ea7` (docs `b103200`) | gate green |
| 4 shell_integration | `cec000e..d72a4f6`, plus `0a0a74e` | gate green, 761 passed |
| 5 specs, prep | `b6772b7`, `1a466e2` | the research and 17 briefs, re-checked at `0a0a74e` |
| 5 specs, run 1 (5.1a-5.1e) | `1a466e2..00510b3` | gate green, 765 passed |
| 5 specs, run 2 (5.2a-5.2b) | `00510b3..e038e57` | gate green, 794 passed |

### Phase 4 (recorded in plan section 9, "Phase 4 notes")

- Three runs: A (steps 1-8) `cec000e..845ed28`, B (9-12b) `845ed28..2410bd0`, C (13-18) `2410bd0..d72a4f6`.
  `integrations.rs` is deleted; `REGISTERED_MODULES` = history, settings, shared, shell_integration. terminal_session
  exists but stays unregistered until Phase 9. `EngineMsg::Session { env, alias }`.
- The reviews added fitness rules 6 (a mod.rs holds only child `mod` lines and one-line attributes), 7 (main.rs holds
  only `mod` lines, attributes and `main`, with no `use`) and 8 (no `#[path]`, `include!`, `#[macro_export]` or
  `#[macro_use]`). They also added `shared/infrastructure/error_chain.rs::display_chain`, the one renderer of anyhow's
  `{:#}`.
- After the phase, `0a0a74e` renamed the duplicate settings test to
  `should_not_save_a_deletion_over_settings_that_cannot_be_loaded` (DUPLICATE is now 0).
- Deviations recorded in the plan:
  - 16 subjects over 72 characters, not rewritten: f5b1b99, 9f5ff3a, c6cede1, 2cfa53b, d78fd05, d39adaa, a9aa205,
    b33b9e6, b78bf3b, bc3929d, 2875954, 8191e36, c6b6533, ea2dc6b, 10a30a5, 140fad1.
  - `29fce49` is over the clippy budget (4 warnings); the next commit fixes it.
  - In run A, an agent ran `filter-branch` over its own fresh commits, and step 13 amended its own fresh commit.
    Since then the briefs forbid any history rewrite.

### Phase 5 run 1: the spec model (`1a466e2..00510b3`)

- 5.1a `0fcc06c`: `ItemType` became `SuggestionKind` in `specs/domain/value_objects` (serde derives dropped, no
  alias). `src/specs.rs` (still the legacy bundle file) declares the layer modules. The tag `phase5-start` was created
  at `1a466e2`.
- 5.1b `5b80c48`: `spec.rs` was split into one file per type under `specs/domain/{entities,value_objects}`, plus
  `SpecError { Invalid }` and `specs/infrastructure/adapters/fig_spec_mapper.rs`. `spec.rs` is deleted.
- 5.1c `8d43aee`, `31b808a`: `GeneratorScript::Object(SpecValue)`, an exact std-only JSON tree (P5). The engine's
  JSON readers and `template_json` moved into the mapper.
- 5.1d `0988dea`, 5.1e `fffca44`: private fields behind accessors and `with_*` builders. Accessors of Option/Vec fields
  return references. `Subcommand::merged_with_generated` owns the generateSpec merge.
- Review fixes:
  - `ffd9b65`: a list/bool round-trip test.
  - `aa16a20`, `1cfe712`: fitness rule 3 widened. Another module's `*UseCase` type or `*_use_case` file is flagged on
    any path under it.
  - `47b14a1`: `suggestion_from_json` is private, which departs from P8's table. Nothing outside the mapper calls it,
    and 5.3f's `function_result` lives in the same file.
  - `36f6576`: documents the adapter back door rule 3 cannot see.
  - `00f603a`: a doc comment.
  - `00510b3`: plan section 1 now states rule 3 as the checker reads it.
- My checks at `00510b3`: gate PASS (765 passed, 1 ignored, clippy 3, golden identical, MISSING 0, DUPLICATE 0).
  integrations-diff and precmd-diff are empty. testlock against `phase5-start`: CHANGED 25, every one a
  builder/accessor/rename restructuring (spot-checked).

### Phase 5 run 2: location, versions, bundle (`00510b3..e038e57`)

- 5.2a (`bfc789b` … `887c420`):
  - `SpecLocation::cache_key` replaces `loc_key`.
  - `SpecIndex` is a domain VO (private fields, `new`, `completions()`, `diff_versioned()`).
  - `services/spec_locator.rs` holds `spec_location`, `spec_location_table` and a pure `find_global_spec` with an
    injected `is_file`.
  - `SemanticVersion`, plus the free fns `semver_clean` and `semver_cmp`.
  - `services/spec_version_selector.rs` (`versions_to_apply`, cumulative, P9).
  - `services/bundle_retention_policy.rs` (`is_stale`).
  - Plan section 4 now says the selector is cumulative.
- 5.2b (`c6299b0` … `f8e6055`):
  - `SpecError::BundleUnavailable(String)` is rendered with `display_chain` and has no `source()`.
  - `application/ports/spec_bundle.rs` (`SpecBundle { install }`, plus a `#[cfg(test)] RecordingSpecBundle`) and
    `application/install_spec_bundle_use_case.rs`.
  - `infrastructure/adapters/embedded_spec_bundle.rs` holds the old `specs.rs` code; `testutil` became `testing`.
  - `infrastructure/specs_module.rs` holds `SpecsModule` and `build_specs_module`.
  - Callers switched: engine.rs, parser.rs (the Global arm builds `EmbeddedSpecBundle::new(..)` per call until 5.4b),
    doctor.rs, js.rs tests, `bootstrap/bridges.rs` (`SpecBundleBridge` holds the use case;
    `spec_bundle_failure` and its test are unchanged) and `bootstrap/cli.rs`.
  - `src/specs.rs` → `src/specs/mod.rs`, which holds only the three `pub(crate) mod` lines.
- Review fix `e038e57`: the version-sort stability test now uses 40 keys, so an unstable sort fails it. The same commit
  fixed a comment in settings_catalog.rs that still named `specs.rs`.
- My checks at `e038e57`, all passing:
  - Gate PASS: 794 passed, 1 ignored, clippy 3, golden identical, MISSING 0, DUPLICATE 0.
  - integrations-diff and precmd-diff are empty.
  - testlock against `phase5-start`: MISSING 0, NEW 34, PATH-ONLY 29, CHANGED 28, DUPLICATE 0. The CHANGED tests are
    run 1's 25 plus `index_merges_user_stems`, `embedded_index_shape` and `resolve_order` (accessors, the free
    `load_from`, `SpecIndex::new`).
  - The release `all_bundle_modules` run: 1 passed.
  - The failing-extraction sandbox prints identical lines on both binaries.
  - The local fitness check with `specs` registered: 54 passed and reports nothing.

## What is missing for Phase 5

Three more phase-workflow runs, each followed by my own verification, then the docs.

| Run | Steps | Titles |
|---|---|---|
| 3 | 5.3a, 5.3b, 5.3c, 5.3d | Runtime value objects and the runtime error; process executor, native ls and module wrapper; QuickJsSpecRuntime (js.rs is deleted); RuntimeEventSource and the engine inbox (`EngineMsg::Timer` is deleted) |
| 4 | 5.3e, 5.3f, 5.3g | Typed spec function calls and the SpecRuntime port; QuickJsSpecRuntime implements SpecRuntime; the parser calls spec functions through SpecRuntime |
| 5 | 5.4a, 5.4b, 5.5 | SpecRepository, SpecLookup and the version-diff merge; QuickJsSpecRepository and the parser/engine wiring; register `specs` and record the phase |

### How to launch a run

The launch form is the same as runs 1 and 2. To get their exact args, read the transcript of
`wf_b5f15cf9-3ea` (run 1) or `wf_5eefea78-63b` (run 2) under the session's `subagents/workflows/`.

- `scriptPath`: `docs/hexagonal-refactor/phase-workflow.js`.
- `phase`: for example `Phase 5 (specs), run 3: steps 5.3a-5.3d`.
- `startRev`: HEAD at launch (`e038e57` for run 3).
- `steps`: `{id, title, prompt}`, with the titles from `phase-5-specs.args.json`. Each prompt is a "SINCE RUNS …"
  preamble plus this pointer, with the step's id:

  > Your full step brief is the `prompt` of the step with id "5.3a" in docs/hexagonal-refactor/phase-5-specs.args.json.
  > Print it with `python3 -c "import json;print([s for s in json.load(open('docs/hexagonal-refactor/phase-5-specs.args.json'))['steps'] if s['id']=='5.3a'][0]['prompt'])"`,
  > read all of it and follow it exactly: it is binding, as if it were written here. Then read
  > docs/hexagonal-refactor/phase-5-specs.md as the brief says. The brief predates runs 1-2: where the code at HEAD
  > differs from what the brief cites (names, files, line numbers, accessors and helpers added by earlier steps and
  > their reviews), follow HEAD and say so in your report.

- `reviewNotes`: a SCOPE note, a summary of the earlier runs, and an order to print the binding review notes first
  (`python3 -c "import json;print(json.load(open('docs/hexagonal-refactor/phase-5-specs.args.json'))['reviewNotes'])"`).
  - The SCOPE note names the five runs, says this run judges only its own steps, says later steps' work is not
    missing, and says the plan's "done when" is checked only in run 5.
  - The earlier-runs summary gives their ranges, their gate numbers, and what they accepted (see below).

### Facts for the run 3 "SINCE" preamble (runs 1-2)

1. **Gate:** runs 1-2 are done and reviewed. The gate is green at `e038e57`: 794 passed, 1 ignored, clippy 3. The
   integration and pre-cmd diffs are empty. testlock against `phase5-start` shows MISSING 0, DUPLICATE 0, CHANGED 28.
2. **Specs module:** `src/specs/mod.rs` holds only `pub(crate) mod domain; application; infrastructure;`. `spec.rs` and
   `specs.rs` are gone; js.rs stays until 5.3c.
   - The model lives in `specs/domain/{entities,value_objects}`.
   - The domain services are `spec_locator`, `spec_version_selector` and `bundle_retention_policy`.
   - The adapters are `fig_spec_mapper` and `embedded_spec_bundle` (with a `testing` submodule), plus `specs_module`.
3. **Model shape:** spec model fields are private behind accessors (Option/Vec accessors return references) and `with_*`
   builders. `SpecError` has `Invalid(String)` and `BundleUnavailable(String)`; 5.3a adds `Aborted` and `Timeout`.
4. **Mapper visibility:** `suggestion_from_json` is private in the mapper. `spec_value_from_json` and
   `spec_value_to_json` are `pub(crate)`. `spec_value_to_json` calls `.expect` on `Number::from_f64`, so a computed
   NaN or inf in a `SpecValue::Float` would panic on the engine thread. Build floats only from parsed JSON.
5. **Fitness rule 3:** another module's `*UseCase`/`*_use_case` is flagged on any path under it. A re-export under
   another name, an `as` rename, a `super::` path, and an adapter re-exported by a legacy file are not flagged, so
   add no `pub(crate) use` shim and grep for one by hand. With `specs` registered locally, the fitness test reports
   nothing at `e038e57`; keep it that way in every step.
6. **Commits:** commit with `git commit -F <file>` (the RTK hook rewrote `f8e6055`'s `-m` subject). Never rewrite
   history, and keep subjects at most 72 characters.
7. **Flaky test:** `integrations_zsh_powerlevel10k_prompt_wrapped` is pty-timing flaky under the gate's load. Rerun it
   alone with `-- --exact` before treating it as red.
8. **Legacy stays legacy:** engine.rs still imports `fig_spec_mapper::{self, …}` for `template_json`. The parser's
   Global arm builds `EmbeddedSpecBundle::new(data_directory_from_env())` per call until 5.4b injects the bundle. Doctor
   names `build_specs_module` and `embedded_spec_bundle::version` directly (legacy until Phase 10).

Runs 4 and 5 add run 3's (and run 4's) review additions to this list, as runs 1 and 2 did.

### After each run, verify it yourself

- `git status` clean. Every subject is at most 72 characters and carries the trailer.
- Full `gate.sh`: GATE PASS, the pass count only grows, clippy 3, golden identical, MISSING 0, DUPLICATE 0.
- integrations-diff and precmd-diff against the baseline binary: both empty.
- `testlock.py . phase5-start --diff`: MISSING 0, DUPLICATE 0. Read every new CHANGED entry and check that it is a
  restructuring listed in phase-5-specs.md's "Test moves".
- The release `all_bundle_modules` run, at every 5.3/5.4 step end and at 5.5.
- The failing-extraction sandbox on both binaries, and the local fitness check with `specs` registered (from run 5's
  5.5, the committed test registers it).
- Read the run's deviations, concerns and `skipped` findings, and carry anything a later step must know into the next
  run's "SINCE" preamble.

### Closing Phase 5 (5.5 does most of it; check it after run 5)

- `specs` is in `REGISTERED_MODULES`. Check by hand that the specs domain and application import no legacy module:
  `specs` is not in `LEGACY_MODULES`, so the fitness test cannot see that edge.
- Plan section 9 gets "Phase 5 notes". They must record:
  - the rule 3 widening (`aa16a20`, `1cfe712`), and an update to the Phase 4 note that says "Rule 3 flags only
    `*_use_case` and `infrastructure` paths";
  - `suggestion_from_json` private (a departure from P8);
  - the garbled subject of `f8e6055` (the RTK hook, not rewritten);
  - the 73-character subjects of `9ba2510` and `e1f1dd2` (run 4, over the 72 limit, not rewritten);
  - `should_report_a_build_without_embedded_specs_like_anyhow` runs only under `--no-default-features` (neither the
    gate nor CI runs it);
  - the accepted rule 3 ceilings;
  - the hand-offs to Phase 6 (the engine's JSON API, EngineInbox, the five session-context tests) and to Phase 10
    (doctor's direct use of specs).
- Reword the awkward plan section 9 phrase "should drop specs.rs from the list's story once it is deleted".
- Mark Phase 5 done in the plan header, its section 5 heading, the README table and state paragraph (test counts,
  CHANGED against `ef807c1` and against `phase5-start`, `REGISTERED_MODULES`), and the Status line of
  phase-5-specs.md. Also update the glossary rows that 5.5 owns (`SpecValue`, `GeneratorScript`).
- Then give the user the final report for phases 3, 4 and 5:
  - commits per phase and gate results;
  - deviations: long subjects, `29fce49` over the clippy budget, the widened fitness rules, the run-A filter-branch
    and the step-13 amend, `f8e6055`, and the 73-character `9ba2510` and `e1f1dd2`;
  - CHANGED tests;
  - open risks.
- Ask the user before pushing.

## Open risks

- pwsh is not installed locally: the pwsh tests and every `cfg(windows)` path are checked only by hand and in CI.
- `integrations_zsh_powerlevel10k_prompt_wrapped` is flaky under parallel load.
- Accepted checker gaps:
  - std::io output;
  - `as` renames, globs and `super::` paths;
  - `pub use` outside mod.rs;
  - the anyhow rule;
  - rule 3's ceilings above.
- `LEGACY_MODULES` still lists `integrations` (harmless).
- The version-sort stability test relies on today's `sort_unstable` algorithm (rustc 1.95) to catch an unstable sort.
  It can never fail falsely.
- While a workflow runs, rust-analyzer reports errors from half-edited files; trust only the gate.
