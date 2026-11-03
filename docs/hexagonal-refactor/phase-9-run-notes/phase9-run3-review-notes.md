# Phase 9 run 3: accepted deviations and watch list

Run 3 (9.6a, 9.6b, 9.3a-9.3i) verified at `c8629c2`: gate 1304 passed, clippy 4 (budget 4), golden identical, windows ok,
testlock vs phase9-start NEW 213 / CHANGED 50 / MISSING 0, vs ef807c1 NEW 1022 / CHANGED 207 / MISSING 0, fixtures
identical, diff scripts empty, `all_bundle_modules` 1 passed, 20 serial runs 0 failures.

## Accepted (do not re-litigate)
- `types::Msg` is now `infrastructure/pty/session_message.rs::SessionMessage` (positional `Resize(cols, rows)` kept; the
  `PtySize { rows, cols }` swap risk stays, R29). Four locked pty.rs test bodies were renamed `Msg::` -> `SessionMessage::`
  (path-only).
- `platform_unix.rs` / `platform_windows.rs` hold the old `sys` modules (dedented, `pub` -> `pub(crate)`); pty.rs selects
  one with a cfg'd `use .. as sys`. `platform_windows.rs` is compiled only by the windows check.
- `Session::start` takes `&WrappedShellCommand`; `SessionId` is built once; `KeyDecoder` is a domain entity with `reset`.
- The `TerminalSession` aggregate keeps `splitter` and `decoder`; its fields are private; only `TerminalSessionParts` is
  `pub(crate)`. The legacy `Session` keeps thin delegates (`next_deadline`, `shell`, `suppressed_in`, `drawable`) and a thin
  `Session::insert` wrapper (a locked test calls it). `Session::alt` and `Session::basis` were deleted.
- Clock reads collapsed as designed (microsecond differences, unobservable): one `now` per refresh / apply_completion,
  `Instant::now()` for `ReloadWatch` and `changed_at` taken once in `TerminalSessionParts`.
- Test-only items are `#[cfg(test)]` (D7) and must survive the Phase 11 dead-code sweep: `expire_*`, `set_spinner_at`,
  `schedule_llm_request_at`, `has_seen_pty_eof`, `exit_deadline`, `tracker_mut`, `unechoed`, `reported_exit_code`,
  `request_id`, `sent`, `edit_buffer`, `has_insertion_lock`, `ShellExit::code`, `ReloadWatch::context_modified`,
  `Session::drawable`.
- Interim: `Session::start` still reads `env!("CARGO_PKG_VERSION")`, `TMUX`, writes the window title, runs the probe loop,
  the writer thread and the child-exit thread (until 9.6e and the launcher). `MarkerTracker::new(..).expect(..)` stays.
- New tests were written as the brief's named set before GREEN (RED shown with stubs); no separate REFACTOR commits.

## Phase 11 watch list (report only)
- Query-looking methods that mutate: `ReloadWatch::poll_due`, `context_changed`, `settings_changed`, `ShellExit::output_ended`
  and the aggregate forwarders (`poll_due`, `context_changed`, `settings_changed`, `shell_output_ended`). Candidate names:
  `take_due_poll`, `note_context_time`, `note_settings_time`, `note_output_ended`. Step 9.5 rewrites their call sites.
- The architecture test does not forbid std I/O, env, clock, threads in domain/application, nor `anyhow`/`println!`/`pub`
  fields outside domain/application; `KeyFacts` and `EditBufferContext` have `pub(crate)` fields (value objects).
  Run `phase9-greps.sh` (B8, B14-B17) by hand before closing Phase 9.
- `docs/git-convention.md`, referenced by AGENTS.md, does not exist in the repo.
- `INSERT_LOCK` couples `InsertionLock` and `EchoHold` (same 100 ms, by design).
