# Hexagonal refactor: handoff (2026-10-30)

Where the refactor stands after Phase 7, and how to start Phase 8. Read this first, then [`README.md`](README.md),
[`agent-brief.md`](agent-brief.md), [`phase-7-ghost-text.md`](phase-7-ghost-text.md) and the plan's section 9
"Phase 7 notes".

## The task

The user asked for phases 6 and 7 of [the plan](../hexagonal-refactor-plan.md), using the `hexagonal-architecture`
skill as reference (Phase 7 in a second session: "continue working on the plan (so far phases 0-6 are done),
implements the refactor for phase 7"). It is a refactor: behaviour must not change, and tests change only when they
have to be moved or restructured. During Phase 6 the user said: "once you finish the phase 6, stop."
**Phase 7 is done. Phase 8 is not started and has no research yet** (see "Next: Phase 8"). Start Phase 8 only when the
user asks for it.

The user turned ultracode on for the session: every substantive task runs as a Workflow.

## Binding rules

- Load the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture` before working
  (AGENTS.md). Commit after each finished task.
- Work on branch `refactor/hexagonal-architecture`, never `main`. **Never push**; ask the user first. Nothing is
  pushed, including the local tags `phase4-start` (`cec000e`), `phase5-start` (`1a466e2`), `phase6-start` (`c38596c`)
  and `phase7-start` (`eef4c88`).
- Never rewrite committed history: no amend, rebase, reset of a commit, filter-branch or force. Fix forward.
- Commit subjects: `refactor(<module>): <why>`, `test(<module>): …`, `docs: …` / `docs(<module>): …` or
  `chore(scripts): …`, at most 72 characters, ending with the trailer of the model that writes the commit. Phases 1-6
  used `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; Phase 7 used
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (the session ran Sonnet 5.5; the Phase 7 preambles say
  so, and agent-brief.md's Opus line is superseded for them). docs/git-convention.md (named in AGENTS.md) does not
  exist; agent-brief.md's commit rules apply.
- **Commit with `git commit -F <file>`, not `-m`.** The RTK command hook rewrites words inside a quoted `-m` string
  (`f8e6055`). Let a long subject stop the commit:
  `[ "$(head -1 <file> | awk '{print length}')" -le 72 ] && git commit -F <file>`. Phase 7 still let one 76-character
  subject through (`fe4b320`, a review fix): measure it before the commit, in every agent's preamble.
- Exact dependency versions only; no new crates.
- Tests are locked against `ef807c1`: never weaken, delete, rename or `#[ignore]` one. MISSING must stay 0 and
  DUPLICATE 0 (testlock keys tests by fn name, so a duplicate name hides one copy).
- Do not edit or commit in the main working tree while a workflow's agents are committing there, and never
  `SendMessage` an agent that runs inside a workflow.
- zsh is the shell here: an unquoted `=====` or `--include=*.rs` in a command fails with "not found" / "no matches".

## Tools

- `scripts/hexagonal-refactor/gate.sh` (`--fast` skips the golden capture): build, tests, clippy (budget 3 warnings,
  all 3 used), the 131-command golden CLI capture, and the test lock against `ef807c1`.
- `python3 scripts/hexagonal-refactor/testlock.py . <rev> [--diff]`. Delete `scripts/hexagonal-refactor/__pycache__`
  afterwards. Its loose form strips `Type::` path prefixes, so a rename confined to an enum path shows as PATH-ONLY,
  not CHANGED: judge restructurings by reading the diff.
- `integrations-diff.sh <bin>`, `precmd-diff.sh <bin>` and, from Phase 7, `inline-diff.sh <bin>` (scripts/hexagonal-refactor):
  diff each one's output for the baseline binary `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith`
  against `target/debug/tabsmith` (after `cargo build --locked`). All three diffs must be empty. `inline-diff.sh` covers
  what the golden capture misses of `tabsmith inline` and doctor's Inline check (valid and invalid profiles, the numbered
  chooser on a pty, a corrupted settings.json, no home).
- `cargo test --locked --release -- --ignored all_bundle_modules` (must print `1 passed`) and
  `cargo check --locked --no-default-features --all-targets`.
- The failing-extraction sandbox on both binaries (phase-5-specs.md, "Error texts reach three sinks").
- Fitness: `completion`, `ghost_text`, `history`, `settings`, `shared`, `shell_integration` and `specs` are registered
  in the committed `src/architecture_tests.rs`, so `cargo test --locked architecture` checks them at HEAD. To check a
  module before its registration step, copy the tree (`git archive HEAD | tar -x -C <dir>` under
  `target/hexagonal-refactor/review-tmp/`), add it to `REGISTERED_MODULES` there, and run the test in the copy;
  `target/hexagonal-refactor/fitness.sh` does this for `ghost_text` (edit the module name for the next one).
- [`phase-workflow.js`](phase-workflow.js): the per-phase Workflow. Args are `{phase, startRev, steps:[{id,title,
  prompt}], reviewNotes}`. One implementer per step (gate + commit each, stopping on red), then four review lenses
  (behaviour, test-lock, architecture, completeness), a skeptic per finding, and fix rounds (up to 3). The args can stay
  small: a short pointer per step to its brief, and a `reviewNotes` that tells the reviewers to `cat` a file.
- Local, git-ignored helpers in `target/hexagonal-refactor/tools/` (they survive the session but not `cargo clean`):
  - `verify-run.sh <label> [guard full paths…]`, with `PHASE_TAG=phaseN-start` and `GUARD20=<suite filter>`: build,
    integrations/precmd diffs, the sandbox, the release bundle test, the no-default-features check, both test locks
    (writes `target/hexagonal-refactor/testlock-<label>.diff`), 50 runs of each guard, 20 serial runs of the suite,
    and the fitness test on a copy. Run `gate.sh` first, and `inline-diff.sh` by hand (it is not in the script). It
    takes a while: start it detached and wait on `== DONE` in its log.
  - `make_p6run.py` and `make_p7run.py <run> <startRev> <out.json>`: build a run's Workflow args (a pointer per step
    to its brief in the args file, plus `target/hexagonal-refactor/phase<N>-run<K>-since.md` as a binding preamble and
    `phase<N>-run<K>-review-full.md` as the review notes). Copy and adapt `make_p7run.py` for Phase 8.

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
| 7 ghost_text | `eef4c88..fe4b320` (registration at `353faaa`) | gate green, 986 passed |

### Phase 7 (recorded in plan section 9, "Phase 7 notes")

The 11 step briefs ran in three Workflow runs, after a re-check of the parked research against `058bebd` (four read-only
auditors; no claim blocked a step, and the binding corrections are the last section of `phase-7-ghost-text.md`).
After each run I re-ran the gate and every check myself, and read the riskiest moves against the old code (the
`GhostText` accept rules, the `Llm` request path's ordering, the moved inline command).

| Run | Steps | Range | Gate |
|---|---|---|---|
| 1 | 7.0-7.4 (guards, the `ghost_text` domain) | `eef4c88..612d3a0` | 950 passed |
| 2 | 7.5-7.7 (ports, adapters, the engine switch, `inline.rs` deleted) | `612d3a0..83e9728` | 970 passed |
| 3 | 7.8-7.10 (inline use cases, the inline command, registration, docs) | `83e9728..fe4b320` | 986 passed |

State at `fe4b320`: gate green (986 passed, 1 ignored, clippy 3, golden identical); testlock vs `ef807c1` MISSING 0,
DUPLICATE 0, CHANGED 154 (150 plus 4 new: `cd_validation` was already CHANGED); vs `phase7-start` MISSING 0,
DUPLICATE 0, CHANGED 5 (`ghost_keys_and_words`, `ghost_with_control_chars_is_never_shown_or_accepted`,
`profiles_and_source`, `request_bodies`, `cd_validation`), NEW 43. integrations, precmd and inline diffs empty; the
sandbox identical; release `all_bundle_modules` 1 passed; `bootstrap::engine_server::tests` 20/20; the two inbox
tests, `debug_shell_survives_closed_terminal`, `inline_show_customizations` and the seven ghost guards 50/50;
`REGISTERED_MODULES` = completion, ghost_text, history, settings, shared, shell_integration, specs. `src/inline.rs`
is gone and `cli.rs` holds no inline code; the engine server dispatches to ghost_text's use cases through
`GhostTextModule`, and the inline command is `ghost_text/infrastructure/cli/inline_command.rs` over
`InlineCommandModule`, with the settings repository injected by bootstrap.

History deviations, recorded in the plan, not rewritten: `fe4b320` has a 76-character subject; the Phase 7 trailer is
the Sonnet 5.5 one. Phase 6's two 73-character subjects (`0d1ea43`, `87dc339`) and `1f18684` (typed `test(completion)`,
rewrites a production doc comment) stand as deviation 38.

## Waiting on the user

- **Manual smoke** (plan section 8, items 1-9, plus the three Phase 7 checks in the plan's Phase 7 notes): due now,
  never run, and agents cannot drive a terminal. With `TABSMITH_HOME=$(mktemp -d)`, run `cargo run -- debug shell zsh`
  (and bash and fish): `echo hi` then `ec` shows the ghost `ho hi` and → accepts it (Alt-f one word); with
  `tabsmith settings inline.source '"specs"'`, `git ch` ghosts a subcommand; with an `inline.profiles.fake` on
  `127.0.0.1:8765`, `zzz sta` shows `tus` after the debounce.
- **Push**: nothing is pushed. The plan's end step pushes the branch for the CI matrix (ubuntu, macos, windows); ask
  first. The `cfg(windows)` code (`cd_target_validator`'s two `cfg!(windows)`, `platform_windows`) is checked only by
  hand until CI runs.

## Next: Phase 8 (dropdown)

Not started and not researched. Phases 3-7 each had a research pass first: the source maps in
[`source-map/`](source-map/) (`keys`, `menu`, `render`, `icons`, `cli`) hold the traps and plan gaps for it (the two
theme parsers, icon bytes versus icon ids, `comp`/`item` test fixtures that move to
`completion/domain/test_fixtures.rs`, the theme command's `{:#}` print of a non-string `autocomplete.theme`). Do the
same as Phase 7: re-check the plan's Phase 8 steps against `fe4b320`, write step briefs, a research file and
`phase-8-dropdown.args.json`, commit them as `docs: hand Phase 8 over with its research`, tag `phase8-start`, then run
it in a few runs with a binding preamble per run (`make_p7run.py` is the template). Phase 8's guards will include the
golden `theme` captures and a new `theme-diff.sh` in the style of `inline-diff.sh`.

## After each run, verify it yourself

- `git status` clean. Every subject is at most 72 characters and carries the trailer.
- Full `gate.sh`: GATE PASS, the pass count only grows, clippy 3, golden identical, MISSING 0, DUPLICATE 0.
- `verify-run.sh` (above) with the phase tag and the guards, plus the phase's output-diff script.
- Every new CHANGED test against the phase tag is in the run's briefs; every NEW test is in the briefs or reported.
- Read the run's deviations, concerns and review findings, hand-check the riskiest moves against the old code, and
  carry what a later step must know into the next run's preamble (and correct any earlier instruction a step proved
  wrong: Phase 7 run 1 showed that an import-removal rule I had written was too broad).

## Open risks

- pwsh is not installed locally: the pwsh tests and every `cfg(windows)` path are checked only by hand and in CI.
- `integrations_zsh_powerlevel10k_prompt_wrapped` is flaky under parallel load; rerun it alone with `-- --exact`.
- Clippy is at its budget of 3 with no headroom. `CompleteCommandLineUseCase::new`, `GeneratorRun::new` and
  `LlmGhostTextJob::new` are at clippy's 7-parameter limit.
- Fitness rule 3 does not flag a use case naming a sibling use case of its module, nor another module's application
  service (completion's `CommandLineParser`, `GeneratorRunner`). Both are covered by hand greps, recorded in the plan
  (the `completion::application::(command_line_parser|generator_runner|test_fixtures)` grep stays on every step of
  Phases 8-9). The older ceilings (`as` renames, globs, `super::` paths, `pub use` outside mod.rs, the anyhow rule,
  std::io output) still stand.
- `LEGACY_MODULES` still lists modules that no longer exist (`engine`, `inline`, `js`, `parser`, `shell_parser`,
  `spec`, `suggest`, `integrations`): harmless, for the Phase 11 sweep.
- `CompletionEngine::request_llm_ghost_text` (terminal_session's port, `ChannelCompletionEngine`) and `doctor.rs`'s
  `inline_outcome` (with its `Err(_)`/`Ok(_)` catch-alls) still name ghost text through legacy paths: Phases 9 and 10.
- `StdinSourceChooser` and the Enable/Disable/Status/SetCustomization/ShowCustomizations arms of `run_inline_command`
  have no unit test of their own: golden, `inline-diff.sh` and its pty groups cover them.
- `EngineInbox::next_msg`'s doc still ends "`None` when all senders are gone", which cannot happen (the inbox keeps
  its own sender). Reword it when the inbox is next touched.
- `git log --follow src/completion/domain/entities/generator_cache.rs` walks into `suggest.rs`'s history (git pairs
  them at 56%).
- While a workflow runs, rust-analyzer reports errors from half-edited files; trust only the gate.
- The worktrees `target/hexagonal-refactor/prep-tree` (`a3953cf`) and `prep6-tree` (`c2f0825`) are old research
  copies and can be removed; `baseline-tree` (`af1b304`) is needed by the diffs.
