# Hexagonal refactor: handoff (2026-10-30, Phase 8 in progress: runs 1 and 2 done, run 3 next)

Where the refactor stands, and how to continue in a new session. Read this first, then [`README.md`](README.md),
[`agent-brief.md`](agent-brief.md), [`phase-8-dropdown.md`](phase-8-dropdown.md) (the Phase 8 research, decisions R1-R30,
deviations, test moves, guard design) and the plan's section 9 "Phase 7 notes" for the conventions. This file replaces the
Phase 7 handoff (Phases 0-7 are in the plan's section 9).

## The task

The user's words this session: "continue working on the plan @docs/hexagonal-refactor-plan.md (so far phases 0-7 are done).
implements the refactor for phase 8 and 9. create a new branch off this one and open a different PR for this one. use the
hexagonal architecture skill as reference. (Remember, this is a refactor. The test shouldn't change unless you need
restructure or move them. This MUST not change behavior)."

Later the user narrowed it: **"Once you finish with phase 8 stop there."** So the scope of this branch is **Phase 8 only**.
**Phase 9 (terminal_session) is NOT started and must not be started without the user asking for it.** Then, near the usage
limit: "stop when finish the run 2 and create a handoff to continue in a different session" (this file).

The user turned ultracode on: every substantive task runs as a Workflow (the Workflow tool, script `phase-workflow.js`).

## Where the work lives

- Branch **`refactor/hexagonal-dropdown-terminal-session`**, created off `refactor/hexagonal-architecture` (PR #1
  `refactor/hexagonal-architecture` → `main` is still OPEN; PR #2 was merged). Stay on the new branch; never commit on `main`.
- Local tags (not pushed): `phase4-start` … `phase7-start`, and **`phase8-start` = `7aafcef`** (the end of Phase 7 plus the
  CI-only commits; its `src/` equals the hand-over commit's). `phase8-start` is the testlock baseline of every Phase 8 step.
- **Nothing is pushed and there is no PR for this branch yet.** The user asked for a separate PR: when Phase 8 is done and
  verified, push THIS branch only and open the PR with base `refactor/hexagonal-architecture` (stacked on PR #1, so it shows
  only Phase 8). See "Finish".

## Binding rules

- Load the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture` before touching code
  (AGENTS.md). Commit after each finished task.
- Never rewrite committed history (no amend, rebase, reset of a commit, filter-branch, force). Fix forward.
- Commit subjects: `refactor(<module>): <why>`, `test(<module>): …`, `docs: …`, `chore(scripts): …`, at most 72 characters,
  ending with the trailer `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (the model that writes the commit:
  Phases 1-6 used Opus 5.5, Phase 7 and 8 Sonnet 5.5; agent-brief.md's Opus line is superseded). `docs/git-convention.md`
  (named in AGENTS.md) does not exist.
- **Commit with `git commit -F <file>`, not `-m`**: the RTK command hook rewrites words inside a quoted `-m` string. Measure
  the subject first: `[ "$(head -1 <file> | awk '{print length}')" -le 72 ] && git commit -F <file>`.
- Exact dependency versions only; no new crates.
- **Tests are locked** against `ef807c1`: never weaken, delete, rename or `#[ignore]` one. MISSING must stay 0 and DUPLICATE 0
  (testlock keys tests by fn name). Only restructuring is allowed in a test: paths, glossary renames, moving it, splitting it
  into parts that assert the same, building the same scenario through a new constructor.
- **Behaviour must not change.** Where the plan's letter would change it, keep the behaviour and record a deviation (D1).
- **Never edit or commit in the main working tree while a workflow's agents are committing there**, and never `SendMessage` an
  agent that runs inside a workflow. Draft docs under `target/` (git-ignored) meanwhile.
- zsh is the shell here: an unquoted `=====` or `--include=*.rs` in a command fails; in zsh `git show $BASE:src/x.rs` reads
  `$BASE:s` as a modifier, so write `"${BASE}:src/x.rs"`.
- Never run cargo from outside the repo root with a relative `CARGO_TARGET_DIR`, and never leave cargo output outside
  `target/` (a brief writer once created a 660 MB `target/` next to the repo; it was deleted).
- Pushing: the user asked for a PR for this branch, so pushing `refactor/hexagonal-dropdown-terminal-session` and opening
  that PR is authorised; pushing anything else (or tags) is not.

## Tools (all run from the repo root)

Committed, in `scripts/hexagonal-refactor/`:
- `gate.sh` (`--fast` skips the golden capture): build, tests, clippy (budget 3 warnings, all 3 used), the 131-command golden
  CLI capture, the test lock against `ef807c1`. `GATE PASS` is required to commit.
- `testlock.py . <rev> [--diff]` (use `ef807c1` and `phase8-start`); delete `scripts/hexagonal-refactor/__pycache__` afterwards.
  Its loose form strips only `a::b` paths, so a struct-literal type rename (`Mods {` → `Modifiers {`) counts as CHANGED.
- `integrations-diff.sh`, `precmd-diff.sh`, `inline-diff.sh` and, new in Phase 8 (8.0a), **`theme-diff.sh`**: each prints a
  scrubbed transcript; diff the baseline binary `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` against
  `target/debug/tabsmith` (after `cargo build --locked`): all four diffs must be empty.
- `cargo test --locked --release -- --ignored all_bundle_modules` (`1 passed`) and `cargo check --locked --no-default-features
  --all-targets`.

Committed, in `docs/hexagonal-refactor/`: `phase-workflow.js` (the per-run Workflow: one implementer per step with gate +
commit, four review lenses, a skeptic per finding, fix rounds up to 3; it now takes `args.branch`), `phase-8-dropdown.md`
(research) and `phase-8-dropdown.args.json` (the 19 step briefs, each step has a `run` field), `source-map/`.

Local, git-ignored, under `target/hexagonal-refactor/` (they survive the session but NOT `cargo clean`, which deletes the whole
`target/`, the baseline tree included: rebuild it with `scripts/hexagonal-refactor/make-baseline.sh`):
- `baseline-tree/` (the `af1b304` worktree and its binary, needed by every diff script) and `baseline/` (the golden capture).
- `fitness-mod.sh <module>…`: runs the architecture test on a COPY of HEAD with the modules added to `REGISTERED_MODULES`
  (run it after committing, since it archives HEAD).
- `verify-p8.sh <label>`: the orchestrator's full verification of a run, log in `verify-<label>.log` (gate, theme/inline/
  integrations/precmd diffs, the sandbox, the release bundle test, the no-default-features check, both test locks, 20 serial
  runs of the dropdown tests, fitness with `dropdown` registered). It runs detached; wait with a background
  `until grep -q '== VERIFY DONE' <log>` loop. It takes about 20 minutes.
- `tools/make_run.py <phaseNum> <run> <startRev> <out.json> <branch>`: builds a run's Workflow args (a pointer per step to its
  brief in `phase-8-dropdown.args.json`, the binding preamble `phase8-run<K>-since.md` and the reviewer notes
  `phase8-run<K>-review-full.md`). Print the JSON and pass it as `args` to `Workflow({scriptPath:
  'docs/hexagonal-refactor/phase-workflow.js', args})`.
- `phase8-review-notes-base.md`, `phase8-run1-since.md`, `phase8-run2-since.md`, `phase8-run{1,2}-scope.md`,
  `phase8-run{1,2}-review-full.md`. **Copies are committed in `docs/hexagonal-refactor/phase-8-run-notes/`**; if the
  `target/` ones are gone, copy them back.
- `p89/`: the seven researchers' raw notes (`p8-*.md`), the plan JSON and the 19 brief sources (`p8-step-<id>.md`; the briefs are
  also inside the args JSON).

## Done

| Phase | Range | Result |
|---|---|---|
| 0-7 | up to `7aafcef` | see the plan, section 9 |
| 8, hand-over | `cfc5f80` | research doc + 19 briefs audited by 5 lenses (30 findings, all fixed or declined with reasons) |
| 8 run 1 (8.0a, 8.0b, 8.1a-c, 8.2a-c, 8.4a) | `cfc5f80..73bf7de` | **verified by the orchestrator**: gate 1013 passed, 1 ignored, clippy 3, golden identical, MISSING 0 / DUPLICATE 0, CHANGED 163 vs `ef807c1` and 18 vs `phase8-start` (as the briefs list), NEW 27; theme/inline/integrations/precmd diffs empty; sandbox identical; release `all_bundle_modules` `1 passed`; fitness passes with `dropdown` registered on a copy |
| 8 run 2 (8.4b) | `73bf7de..5a19bda` | **reported green, NOT yet re-verified by the orchestrator** (the usage limit stopped the session before `verify-p8.sh run2` ran). 8.4b is one commit `768a252` (`icons.rs` deleted; ids and the resolver in `dropdown`, `icon_assets.rs` and `kitty_graphics.rs` in `terminal_session`) plus the review-fix commit `5a19bda` (3 non-blocking findings: `PNGS`/`TEMPLATE_RGBA` private again, the guard test given its Act phase, and the test-only `terminal_session -> specs` edge recorded in the research doc). Reported by the step: gate 1016 passed, clippy 3, golden identical; testlock MISSING 0 / DUPLICATE 0, CHANGED 21 vs `phase8-start` (the 18 plus `resolves_like_q`, `tmux_uses_placeholders`, `transmit_chunks`) and 164 vs `ef807c1`, NEW 30; theme/inline/integrations/precmd diffs empty; release `all_bundle_modules` `1 passed`; the 92-row icon hash and the overlay hash unchanged in value |

What run 1 built: `src/dropdown/` (key VOs with private fields and builders, `Keymap`, `KeystrokeParser`, `Menu` +
`MenuVisibility`/`MenuEffect`/`CommonPrefix` service, `IconId`/`IconSlot`), `terminal_session`'s `StdinKeyDecoder` and
`KeyEvent`; `keys.rs` and `menu.rs` are gone; `Menu::on_completion/loading` take `now`; the fixtures `item/comp/settings` live
in `completion/domain/test_fixtures.rs`; the guards `theme-diff.sh` and the byte-hash tests (8.0b: 92 icon rows, 10 overlay
scenarios; their VALUES must never change) exist.

## Next: run 3 (steps 8.3a, 8.3b, 8.3c, 8.5a, 8.5b, 8.5c, 8.5d, 8.6, 8.7)

1. `git status` clean, `git log --oneline 73bf7de..HEAD`, confirm run 2 ended green (see see the table above) and verify it:
   `nohup target/hexagonal-refactor/verify-p8.sh run2 >/dev/null 2>&1 &` then wait for `== VERIFY DONE` in
   `target/hexagonal-refactor/verify-run2.log`. Expect after 8.4b: `icons.rs` gone, `grep -rn 'crate::icons' src` empty, the
   icon and overlay hash tests unchanged in value, exactly `resolves_like_q`, `tmux_uses_placeholders` and `transmit_chunks`
   newly CHANGED (21 against `phase8-start`), `all_bundle_modules` `1 passed`.
2. Write `target/hexagonal-refactor/phase8-run3-since.md` (model: `phase8-run2-since.md`: state at launch with the verified
   numbers, what runs 1-2 built as built, the specifics of run 3, flakes) and `phase8-run3-scope.md`; cat
   `phase8-review-notes-base.md` + the scope into `phase8-run3-review-full.md`; build the args with `make_run.py 8 3 <HEAD>
   target/hexagonal-refactor/phase8-run3.args.json refactor/hexagonal-dropdown-terminal-session`; launch the Workflow.
   Run 3 is the longest (nine steps; 8.5c switches the `theme` command, 8.5d the live theme, 8.6 moves the renderer by `git mv`,
   8.7 registers `dropdown` and writes ALL the Phase 8 docs: the plan's sections 4, 5, 8 and 9 "Phase 8 notes", the glossary
   rows, README status, this handoff). Everything about the steps is in the briefs; the research doc's "Steps" tables list
   each step's guards and expected CHANGED tests.
3. Verify run 3 with `verify-p8.sh run3` (the fitness part there registers `dropdown` on a copy; after 8.7 it is registered
   in the committed test, so `cargo test --locked architecture` checks it directly). Read the run's deviations, concerns and
   review findings and hand-check the riskiest moves against the old code (the theme parsers: strict vs lenient, ~16 differing
   inputs; the `theme` command's output order; `OverlayRenderer`'s byte output).

## Finish (after run 3 is verified)

- The manual smoke (plan section 8, plus Phase 8's P8-S1/P8-S2, see the research doc "Manual smoke") has never been run; agents
  cannot drive a terminal. Ask the user to run it, with `TABSMITH_HOME=$(mktemp -d) cargo run -- debug shell zsh` (and bash,
  fish): the menu draws and hides, Tab inserts, ghost text, themes (`tabsmith theme`, a custom file, `system`), kitty/ghostty
  direct icons and tmux placeholder icons, the spinner after about 200 ms on a slow generator, resizing.
- Push the branch and open the PR (authorised by the user, only this branch):
  `git push -u origin refactor/hexagonal-dropdown-terminal-session` then
  `gh pr create --base refactor/hexagonal-architecture --head refactor/hexagonal-dropdown-terminal-session` with a title like
  "refactor: Phase 8, move the dropdown (keys, menu, themes, icons) into its module" and a body that says what moved, the test
  numbers, the deviations (research doc "Deviations from the plan's letter"), that Phase 9 is NOT in this PR, that the manual
  smoke is pending, and that it is stacked on PR #1. End the body with the line
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)`. Let the CI matrix (ubuntu, macos, windows) run: the
  `cfg(windows)` and `cfg(target_os = "linux")` code cannot be compiled locally.
- **Then stop.** Phase 9 only if the user asks; its research would be written after Phase 8 lands (pty.rs and tracker.rs are
  unchanged by Phase 8 apart from imports, the key builders, `now`, the theme resolution and the renderer import).

## Phase 9 (not started; for the record)

The plan's Phase 9 (terminal_session: `pty.rs` 2521 lines with a 44-field `Session`, `tracker.rs` 2210 lines) has no research
yet. What Phase 8 leaves for it is in the research doc, "Hand-off notes for Phase 9" (`FrameInput` stays in
`overlay_renderer.rs` and gets `now`; `types::Model` is imported by the overlay renderer through its legacy path; the
decoder is a `Session` field and its port-or-service shape is Phase 9's; the lenient settings load and the
`Rc<dyn ThemeRepository>` come from bootstrap). Do the same as for Phase 8: recon agents per legacy area, a planner, per-step
briefs, five audit lenses, then runs.

## Open questions and the defaults applied (the user did not answer; record them in the PR)

O1 private key fields (done); O2 the renderer reads `Instant::now()` itself, Phase 9 adds `FrameInput.now`; O3 `tinted` stays in
`icon_assets.rs`; O4 `keymap_defaults_and_overrides` kept whole in the decoder tests (a recorded D6 deviation); O5
`Rc<dyn ThemeRepository>` in the session; O6 `Image`/`Source` renamed `KittyImage`/`KittyPixels`; O7 `FrameInput` stays in
`overlay_renderer.rs`; O8 the manual smoke is asked once, after 8.7.

## Open risks

- pwsh is not installed locally: pwsh tests and every `cfg(windows)` path are checked only by hand and in CI.
- Environmental flakes under gate load, never caused by the code: `integrations_zsh_powerlevel10k_prompt_wrapped` (once failed
  even alone for a window of time during run 1; rerun alone with `-- --exact`) and
  `specs::infrastructure::adapters::process_executor::tests::exec_status_output_and_timeout` (a 100 ms spawn timeout). Rerun
  alone before treating the gate as red.
- Clippy is at its budget of 3 with no headroom: prune imports a move leaves unused in the same commit.
- The hash tests bake FNV values generated on macOS aarch64; they read no clock, env, file or platform code, so ubuntu and
  windows should agree, but that is reasoned, not run, until CI runs.
- Deferred review nits of run 1: one 8.0b keymap test asserts two unrelated outcomes; `pty.rs` has two spellings of the empty
  modifier set. `LEGACY_MODULES` in the architecture test still lists modules that no longer exist (Phase 11 sweep).
- rust-analyzer reports errors from half-edited files while a workflow runs (and stale `pty.rs` E0308 noise): trust only the
  gate.
- `terminal_session` is not registered in the architecture test until Phase 9, so its new adapters (`stdin_key_decoder`,
  `kitty_graphics`, `icon_assets`, `overlay_renderer`) are not rule-checked yet; the decoder has unpaired `[` in byte literals
  to check then.
