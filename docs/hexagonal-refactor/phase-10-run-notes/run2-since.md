# Phase 10, run 2 preamble: what run 1 built (binding; where a step brief and this file differ, this file wins)

Run 1 (10.0-10.6) is committed: `phase10-start..HEAD` (tag `phase10-start` = b0fe04d). HEAD at the start of run 2 passes the
gate (1645 tests, clippy 3, windows ok, golden identical), testlock vs `phase10-start`: MISSING 0, CHANGED 0, DUPLICATE 0.
The step briefs of run 2 were written BEFORE run 1 existed: read the real domain code in `src/diagnostics/domain/**`
(signatures, constructors, accessors, derives) and the design errata (`target/p10/design-errata.md`) before you write
anything that uses them. If a brief names a method that run 1 spelled differently, use the code's spelling and report it.

## Real domain API facts that differ from or add to the design text
- `TerminalEnvironmentParts` (derives Default) builds `TerminalEnvironment`; the probe (10.12a) MUST set `tabsmith_version`
  to `env!("CARGO_PKG_VERSION")`: an empty version makes an empty `TABSMITH_TERM` count as Ok.
- `CheckSubject::Environment(EnvironmentCheck, Rc<SystemFacts>)`: Check / CheckGroup are not Send. Never move them across threads.
  PartialEq on them compares the facts by VALUE.
- `DoctorRun` derives Clone, Debug, PartialEq. `failed()` equals the legacy exit rule ONLY because `restore(counts)` is called
  after the system group: `RunDoctorUseCase` (10.10b) must do `let counts = run.counts(); run_group(system); run.restore(counts)`.
  The spinner is NOT in DoctorRun (the reporter owns it).
- `dotfile_outcome` has no catch-all arm (all seven `ShellIntegrationError` variants are listed). Report and new rules follow D8.
- `diagnostics_error.rs` had been dropped from `domain/mod.rs` for five steps and its tests did not run: after EVERY step run
  `scripts/hexagonal-refactor/phase10-greps.sh | grep E4` (new check: every .rs under src/diagnostics must be declared by a mod) and
  compare the test count with the count before your step (a new test file must raise it).
- `is_local` (inline source) is a SUBSTRING test (`https://localhost.example.com` counts as local): legacy behaviour, pinned, do not fix.
- `UsernameScrubber` rewrites raw substrings (`USER=sb` rewrites `/usr/sbin`): legacy, pinned, do not fix.
- `doctor_check_catalog/tests.rs` is ~1200 lines (design said sibling tests.rs over ~500 lines; accepted). Do not grow it in run 2.
- `RestartReminderRepository::is_pending` / `clear_pending` exist in shell_integration (10.1) and have no production caller yet.
- `CheckFix::marks_restart_reminder` exists (errata item 2). Nothing consumes it before 10.16.

## Process rules learned in run 1
- Commit with `git commit -F <file>` (message file under the scratchpad or target/p10, never `-m`). The subject MUST be at most 72
  characters: check with `git log -1 --format=%s | awk '{print length}'` and use a short subject (five run-1 subjects were 73-80; history is not rewritten).
  Trailers exactly: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: https://claude.ai/code/session_01D36FSrEYWhahJbdyhds8F8`.
- RED first for real: write the failing test, SEE it fail for the right reason, then the implementation. One commit after the gate is
  green is fine (a red tree is never committed), but say in the report which tests you saw fail.
- Test names are unique tree-wide (`git grep -n "fn <name>\b"` before you add one). Test doubles: each item and each `use` that only it
  needs carries its own `#[cfg(test)]`, never `#![cfg(test)]`.
- Clippy has ONE spare warning (3 of 4). Run `cargo clippy --all-targets --locked` before the commit in 10.7a, 10.7b, 10.10a, 10.10b.
- `docs/git-convention.md` does not exist; agent-brief.md and design section 14 are the commit rules. Never run `cargo clean`.
