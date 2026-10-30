# Hexagonal refactor: handoff (2026-10-30)

Where the refactor stands after Phase 6, and how to start Phase 7. Read this first, then [`README.md`](README.md),
[`agent-brief.md`](agent-brief.md), [`phase-6-completion.md`](phase-6-completion.md) and the plan's section 9
"Phase 6 notes".

## The task

The user asked for phases 6 and 7 of [the plan](../hexagonal-refactor-plan.md), using the `hexagonal-architecture`
skill as reference. Phases 0-5 were already done. It is a refactor: behaviour must not change, and tests change only
when they have to be moved or restructured. During Phase 6 the user said: "once you finish the phase 6, stop."
**Phase 6 is done. Phase 7 is not started**: its research is parked (see "Next: Phase 7"). Start Phase 7 only when
the user asks for it.

The user turned ultracode on for the session: every substantive task runs as a Workflow.

## Binding rules

- Load the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture` before working
  (AGENTS.md). Commit after each finished task.
- Work on branch `refactor/hexagonal-architecture`, never `main`. **Never push**; ask the user first. Nothing is
  pushed, including the local tags `phase4-start` (`cec000e`), `phase5-start` (`1a466e2`) and `phase6-start`
  (`c38596c`).
- Never rewrite committed history: no amend, rebase, reset of a commit, filter-branch or force. Fix forward.
- Commit subjects: `refactor(<module>): <why>`, `test(<module>): …`, `docs: …` / `docs(<module>): …` or
  `chore(scripts): …`, at most 72 characters, ending with the trailer
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. docs/git-convention.md (named in AGENTS.md) does not
  exist; agent-brief.md's commit rules apply.
- **Commit with `git commit -F <file>`, not `-m`.** The RTK command hook rewrites words inside a quoted `-m` string
  (`f8e6055`). Let a long subject stop the commit:
  `[ "$(head -1 <file> | awk '{print length}')" -le 72 ] && git commit -F <file>` (a bare `awk … && git commit`
  let two 73-character subjects through in Phase 6).
- Exact dependency versions only; no new crates.
- Tests are locked against `ef807c1`: never weaken, delete, rename or `#[ignore]` one. MISSING must stay 0 and
  DUPLICATE 0 (testlock keys tests by fn name, so a duplicate name hides one copy).
- Do not edit or commit in the main working tree while a workflow's agents are committing there, and never
  `SendMessage` an agent that runs inside a workflow.

## Tools

- `scripts/hexagonal-refactor/gate.sh` (`--fast` skips the golden capture): build, tests, clippy (budget 3 warnings,
  all 3 used), the 131-command golden CLI capture, and the test lock against `ef807c1`.
- `python3 scripts/hexagonal-refactor/testlock.py . <rev> [--diff]`. Delete `scripts/hexagonal-refactor/__pycache__`
  afterwards. Its loose form strips `Type::` path prefixes, so a rename confined to an enum path shows as PATH-ONLY,
  not CHANGED: judge restructurings by reading the diff.
- `integrations-diff.sh <bin>` and `precmd-diff.sh <bin>` (scripts/hexagonal-refactor): diff each one's output for
  the baseline binary `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` against `target/debug/tabsmith`
  (after `cargo build --locked`). Both diffs must be empty.
- `cargo test --locked --release -- --ignored all_bundle_modules` (must print `1 passed`) and
  `cargo check --locked --no-default-features --all-targets`.
- The failing-extraction sandbox on both binaries (phase-5-specs.md, "Error texts reach three sinks").
- Fitness: `completion` is registered in the committed `src/architecture_tests.rs`, so `cargo test --locked
  architecture` checks it at HEAD. To check a module before its registration step, copy the tree
  (`git archive HEAD | tar -x -C <dir>` under `target/hexagonal-refactor/review-tmp/`), add it to
  `REGISTERED_MODULES` there, and run the test in the copy.
- [`phase-workflow.js`](phase-workflow.js): the per-phase Workflow. Args are `{phase, startRev, steps:[{id,title,
  prompt}], reviewNotes}`. One implementer per step (gate + commit each, stopping on red), then four review lenses
  (behaviour, test-lock, architecture, completeness), a skeptic per finding, and fix rounds (up to 3).
- Local, git-ignored helpers in `target/hexagonal-refactor/tools/` (they survive the session but not `cargo clean`):
  - `verify-run.sh <label> [guard full paths…]`, with `PHASE_TAG=phaseN-start` and `GUARD20=<suite filter>`: build,
    integrations/precmd diffs, the sandbox, the release bundle test, the no-default-features check, both test locks
    (writes `target/hexagonal-refactor/testlock-<label>.diff`), 50 runs of each guard, 20 serial runs of the suite,
    and the fitness test on a copy. Run `gate.sh` first.
  - `make_p6run.py <run> <startRev> <out.json>`: builds a run's Workflow args (a short pointer per step to its brief
    in the args file, plus `target/hexagonal-refactor/phase6-run<N>-since.md` as a binding preamble and
    `phase6-run<N>-review.md` as the review notes). Copy and adapt it for Phase 7 (args path, run map).

## Done

| Phase | Range | Result |
|---|---|---|
| 0 baseline | `af1b304` | |
| 1 composition root, fitness test, shared kernel | `af1b304..52dcf85` | |
| 2 settings | `52dcf85..c7b796e` | |
| 3 history | `a3953cf..f485ea7` | gate green |
| 4 shell_integration | `6f38da1..ab7052b` | gate green |
| 5 specs | `1a466e2..d7079b9` | gate green |
| 6 completion | `c38596c..7f48161` (registration at `f3fc30a`) | gate green, 943 passed |

### Phase 6 (recorded in plan section 9, "Phase 6 notes")

The 28 step briefs ran in six Workflow runs. After each run I re-ran the gate and every check myself, and read the
riskiest moves against the old code.

| Run | Steps | Range | Gate |
|---|---|---|---|
| 1 | 6.0a, 6.1a-c, 6.2a-b | `c38596c..7602525` | 863 passed |
| 2 | 6.3a-e (the parser) | `7602525..a268787` | 868 passed |
| 3 | 6.2c-f, 6.6a | `a268787..e9f247b` | 876 passed |
| 4 | 6.4a-d | `e9f247b..292af32` | 898 passed |
| 5 | 6.4e-f, 6.5a-b | `292af32..bbe7539` | 921 passed |
| 6 | 6.5c, 6.4g, 6.6b, 6.7 | `bbe7539..7f48161` | 943 passed |

State at `7f48161`: gate green (943 passed, 1 ignored, clippy 3, golden identical); testlock vs `ef807c1` MISSING 0,
DUPLICATE 0, CHANGED 150; vs `phase6-start` MISSING 0, DUPLICATE 0, CHANGED 65, NEW 92 (the phase doc's table counts
66 CHANGED: `aborts_propagate` changed only an enum path, which testlock shows as PATH-ONLY). integrations/precmd
diffs empty; the sandbox identical; release `all_bundle_modules` 1 passed; `REGISTERED_MODULES` = completion,
history, settings, shared, shell_integration, specs. `src/engine.rs`, `parser.rs`, `shell_parser.rs` and
`suggest.rs` are gone; the engine thread is `src/bootstrap/engine_server.rs` (+ `engine_server/tests.rs`),
`engine_message.rs` and `engine_inbox.rs`.

History deviations, recorded as deviation 38 and in the plan, not rewritten: `0d1ea43` and `87dc339` have
73-character subjects; `1f18684` is typed `test(completion)` but also rewrites a production doc comment.

## Waiting on the user

- **Manual smoke** (plan section 8, items 1-9): due now, never run. The Phase 6 plan asked for items 1-2 after 6.4d
  and the whole list after 6.7 (O6). Agents cannot drive a terminal. With `TABSMITH_HOME=$(mktemp -d)`, run
  `cargo run -- debug shell zsh` (and bash and fish).
- **Push**: nothing is pushed. The plan's end step pushes the branch for the CI matrix (ubuntu, macos, windows); ask
  first.

## Next: Phase 7 (ghost_text)

The research and the 11 step briefs (`7.0` … `7.10`, three runs: 7.0-7.4, 7.5-7.7, 7.8-7.10) are parked, untracked,
in `target/hexagonal-refactor/prep7/pending/`: `phase-7-ghost-text.md` and `phase-7-ghost-text.args.json`. The
underlying research is beside them in `target/hexagonal-refactor/prep7/` (`ghost-text.md`, `decisions.md`,
`recheck.md`, `review-notes.md`, `briefs-run1..3.json`). They were written and re-checked at `ff2db5a` (the end of
Phase 5) and predict the post-Phase-6 state. Before run 1:

1. Re-check the "Re-check at `phase7-start`" table (K1-K16) and every HEAD citation in the briefs against `7f48161`,
   and correct the briefs where the code differs. Known differences:
   - The engine server's tests live in the sibling `src/bootstrap/engine_server/tests.rs` (14 tests, path
     `bootstrap::engine_server::tests`); the two inbox tests are `bootstrap::engine_inbox::tests::…`.
   - The ghost cache field is `ghost_cache: Arc<Mutex<GhostCache>>` with `use crate::inline::{self, GhostCache,
     Source}` (the docs write `inline::GhostCache`; same type).
   - K6's wording in the prep research is known to be stale.
   - The plan's Phase 6 hand-off adds a hand grep to every Phase 7-9 step:
     `grep -rnE 'completion::application::(command_line_parser|generator_runner|test_fixtures)' src
     --exclude-dir=completion` must stay empty outside `#[cfg(test)]`.
2. Move the two pending files into `docs/hexagonal-refactor/`, update the README status row and Files list, commit
   `docs: hand Phase 7 over with its research`, and tag that commit `phase7-start` (local only).
3. Run it like Phase 6: per run, a binding preamble (state at launch, commit rules, checks, the cfg(windows) hand
   check, what earlier runs left) and review notes (scope, accepted earlier runs, the evidence to rerun). Then verify
   each run yourself.

Phase 7 guards: `bootstrap::engine_server::tests` 20 times with `--test-threads=1`; the two inbox tests and
`cli::tests::debug_shell_survives_closed_terminal` 50 times each by full path; `inline-diff.sh`, which 7.0 creates.

## After each run, verify it yourself

- `git status` clean. Every subject is at most 72 characters and carries the trailer.
- Full `gate.sh`: GATE PASS, the pass count only grows, clippy 3, golden identical, MISSING 0, DUPLICATE 0.
- `verify-run.sh` (above) with the phase tag and the guards.
- Every new CHANGED test against the phase tag is in the run's briefs; every NEW test is in the briefs or reported.
- Read the run's deviations, concerns and review findings, hand-check the riskiest moves against the old code, and
  carry what a later step must know into the next run's preamble.

## Open risks

- pwsh is not installed locally: the pwsh tests and every `cfg(windows)` path are checked only by hand and in CI.
- `integrations_zsh_powerlevel10k_prompt_wrapped` is flaky under parallel load; rerun it alone with `-- --exact`.
- Clippy is at its budget of 3 with no headroom. `CompleteCommandLineUseCase::new` and `GeneratorRun::new` are at
  clippy's 7-parameter limit.
- Fitness rule 3 does not flag a use case naming a sibling use case of its module, nor another module's application
  service (completion's `CommandLineParser`, `GeneratorRunner`). Both are covered by hand greps, recorded in the plan.
  The older ceilings (`as` renames, globs, `super::` paths, `pub use` outside mod.rs, the anyhow rule, std::io
  output) still stand.
- `LEGACY_MODULES` still lists modules that no longer exist (`engine`, `js`, `parser`, `shell_parser`, `spec`,
  `suggest`, `integrations`): harmless, for the Phase 11 sweep.
- `EngineInbox::next_msg`'s doc still ends "`None` when all senders are gone", which cannot happen (the inbox keeps
  its own sender). Reword it when the inbox is next touched.
- `git log --follow src/completion/domain/entities/generator_cache.rs` walks into `suggest.rs`'s history (git pairs
  them at 56%).
- While a workflow runs, rust-analyzer reports errors from half-edited files; trust only the gate.
- The worktrees `target/hexagonal-refactor/prep-tree` (`a3953cf`) and `prep6-tree` (`c2f0825`) are old research
  copies and can be removed; `baseline-tree` (`af1b304`) is needed by the diffs.
