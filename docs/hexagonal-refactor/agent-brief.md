# Brief for every agent working on the tabsmith hexagonal refactor

Repo: `/Users/pedro-la-rosa/the-clean-coders-harness-with-claude-code/cli-autocompletion` (Rust, one crate).
Branch: `refactor/hexagonal-architecture`. Never commit on `main`. Never push.
Tools: `scripts/hexagonal-refactor/` (gate, golden capture, test-lock check). Their output, the golden baseline and
the baseline worktree live in `target/hexagonal-refactor/` (git-ignored). Research: `docs/hexagonal-refactor/`.
All paths below are relative to the repo root.

## Source of truth
- `docs/hexagonal-refactor-plan.md` — the plan. Section 1 (Rust conventions), section 2 (module map + allowed
  dependencies), section 4 (target layout per module), section 5 (phases and steps). Follow it.
- `docs/ubiquitous-language.md` — every type name must match it.
- `.claude/skills/hexagonal-architecture/` — the reference architecture (adapted to Rust by plan section 1).
- Load the skills `xp-tdd`, `testing-standard`, `coding-standards`, `hexagonal-architecture` with the Skill tool
  before touching code (AGENTS.md protocol).

## THE ONE RULE: behaviour must not change
This is a refactor. The user's words: "tests must be locked. If a test needs to be modified or split go ahead. But
only for restructuring. NEVER FOR CHANGING BEHAVIOUR."
- Never change what a test asserts, the inputs it feeds, or the scenario it sets up. Allowed test edits: import
  paths, renamed type/function names (glossary renames), moving a test to another file, splitting one test into
  several that together assert exactly the same things, replacing an ad-hoc temp-dir helper with the shared one, and
  building the same scenario through a new constructor/fake that produces the same state.
- Every existing test name is kept (a split keeps the original name on one of the parts).
- Never weaken, delete or `#[ignore]` a test to get green. If a test fails after your change, your change is wrong.
- Production code: move it verbatim, then rename, then extract pure logic. Every observable output (stdout, stderr,
  exit codes, files written, file modes, the dropdown bytes) stays byte-identical. When the plan says "dedupe" or
  "unify" two helpers, first prove they behave identically for every caller; if they differ, keep the difference
  (e.g. a parameter or two functions) and say so in your result. Never "fix" a bug you notice: report it instead.
- No new crates. Do not touch `Cargo.toml` / `Cargo.lock` dependency versions.

## How to move code safely
- Move code with a script (python slicing by line ranges, or `git mv` for whole files), never by retyping it from
  memory. After moving, diff the moved block against the original (`git show HEAD:src/x.rs | sed -n 'a,bp'`) to
  prove only paths/visibility/names changed.
- Directory modules use `mod.rs` that only declares children (`pub(crate) mod x;`), no glob re-exports.
- During a phase the old file may become a shim of `pub(crate) use` re-exports so callers keep compiling; the
  phase's last step deletes the shim. Shims never cross phases (exception: `src/types.rs`, deleted in Phase 11).
- `#![allow(dead_code)]` stays in `main.rs` until Phase 11.
- Visibility: `pub(crate)` at most. Domain struct fields private (accessors when other code reads them).
- Tests stay in the same file (`#[cfg(test)] mod tests` at the bottom); a test module over ~500 lines goes to a
  sibling `<file>/tests.rs`.
- Paths in `include_str!` / `include_bytes!` / `CARGO_MANIFEST_DIR` must be updated when a file moves.

## Source map (read the files for every legacy file you touch)
`docs/hexagonal-refactor/source-map/<legacy file>.md` (cli, settings, integrations, doctor, keys, menu, render, icons, pty, tracker,
types, shell_parser, spec, specs, parser (incl. parser/tests.rs), suggest, engine, js, inline, logger, main):
every item's destination, the tests and where they go, BEHAVIOUR TRAPS (code-cited subtleties a refactor silently
breaks) and PLAN GAPS (places where the plan is silent or would change behaviour if followed literally). Read the
traps and gaps for every file you move code out of before you start. Line numbers there are at `af1b304`
(before the refactor); use `git show af1b304:<file>` to read the code they cite.

## Cross-cutting decisions (orchestrator, binding for every phase)
- D1 Behaviour beats the plan's letter. Where following the plan literally would change behaviour (examples the
  source map found: deduping `which`/`find_exe`, merging the two `script_timeout`s, one theme parser instead of a
  strict and a lenient one, doctor reusing `LaunchPolicy`, moving `disableForCommands` out of the loader, deduping
  the two generatorContext builders, `ReinstallIntegrations` for doctor's install fix), keep the behaviour, keep
  the two variants explicit, and report it as a deviation. Never pick a "cleaner" semantics.
- D2 Fitness test scope: the architecture rules govern production code. Items under `#[cfg(test)]` (test modules,
  test-only helpers/fakes) are exempt from rules 1-3, so tests can keep serde_json fixtures and wire real adapters
  without being rewritten. Rule 3 exempts `shared`: every module may use the shared kernel, including
  `crate::shared::infrastructure::*` from its own infrastructure (domain/application still may not reference any
  `::infrastructure`, by rules 1-2).
- D3 Data directory resolution is lazy. Today `init`, `_ should-launch`, `_ get-shell`, `--help`, `version`,
  `completion`, `settings` with only TABSMITH_SETTINGS_PATH set, and the `_ pty` fallback work without HOME, and
  each command fails at the same moment with the same message when it does need HOME. `SharedDependencies` /
  `DataDirectory` must keep that: resolve paths when a command needs them (or carry the deferred error inside the
  VO and surface it from the accessor), never fail eagerly at startup. Same for the logger: it must not depend on
  `crate::settings` (shared imports no other module) and must behave exactly as today when HOME is missing.
- D4 clap identifiers are frozen: variant names, field names, `#[arg]`/`#[command]` attributes, doc comments,
  aliases and the order of subcommands. The shell scripts and the golden help capture depend on them. Unplaced
  clap types that span modules (e.g. `DebugCmd`) live in `bootstrap/cli.rs`.
- D5 Code that moves into domain/application and logs today returns the warning/text to its infrastructure caller,
  which logs it with the exact same text, level and condition.
- D6 A test that mixes two modules is split: the part in the module the plan assigns keeps the original name, the
  other part gets `<original name>_<aspect>`; together they assert exactly what the original asserted.
- D7 Test-only production items (used only by tests) get `#[cfg(test)]` (or stay used), never deleted, so the
  Phase 11 dead-code sweep cannot break tests.
- D8 Exhaustive `match` on domain enums: replace a catch-all arm with explicit arms that map exactly as the
  catch-all did.
- D9 Tests that hard-code their own module path (e.g. self re-exec with `--exact "js::tests::…"`) must have that
  string updated when they move; that is restructuring.

## Code we cannot compile here
Only the `aarch64-apple-darwin` target is installed (no rustup), so `#[cfg(windows)]`, `#[cfg(target_os = "linux")]`
and `cfg(not(target_os = "macos"))` code is never compiled locally, but CI builds it on ubuntu and windows. When you
move such code, move it verbatim and check by hand that every path it uses (`crate::…`, `super::…`, `use` lines,
helper fns it calls) still resolves from its new location. `grep -n "cfg(windows)\|cfg(target_os\|cfg(not(unix" `
on every file you touch.

## New code (seams: traits, InMemory fakes, port doubles, VOs with validation)
Test-first, xp-tdd cycle: RED → GREEN → commit → REFACTOR → commit. Test names `should_…`, AAA layout, no mocks
(InMemory fakes / real objects). One file per type, snake_case file name.

## The gate — run before EVERY commit, from the repo root
    scripts/hexagonal-refactor/gate.sh          # build, tests (>= 283 passed, 0 failed), clippy (<= 3 warnings), golden CLI output, test-lock
    scripts/hexagonal-refactor/gate.sh --fast   # same without the golden CLI capture (use while iterating; run the full gate before commit)
    python3 scripts/hexagonal-refactor/testlock.py . ef807c1 --diff    # diff of every baseline test whose body changed (paths ignored)
`GATE PASS` is required to commit. The gate needs the baseline once per machine (`scripts/hexagonal-refactor/make-baseline.sh`, builds `af1b304`).
The golden check runs 131 CLI commands in an isolated HOME and diffs against the
Phase 0 capture; any difference is a behaviour change and must be fixed, not re-baselined.
The test-lock report lists `CHANGED` tests (body differs beyond paths). Each one must be a pure restructuring;
justify every one in your result. `MISSING` must stay 0 (a moved test keeps its fn name).

## Commits
- Conventional Commits like the log: `refactor(<module>): <why, not what>`; `test(<module>): …` for a test-only
  commit. The subject line is at most 72 characters (like the existing log); put any longer reasoning in the
  body. Always end with the trailer line:
      Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
- Commit after each plan step (and after each GREEN / REFACTOR for test-first seams). Stage explicit paths
  (`git add -A src docs` is fine; never add `target/`).
- `cargo fmt` is not used by this repo's CI; keep the surrounding style (run `rustfmt --edition 2021` only on new
  files if you like; do not reformat moved code).

## Reporting
Your final answer is data for the orchestrator, not prose for a human. Report: what you did per step, the commit
hashes, the final gate output lines, every CHANGED test with its justification, every deviation from the plan and
why, and anything suspicious you noticed but did not fix.
