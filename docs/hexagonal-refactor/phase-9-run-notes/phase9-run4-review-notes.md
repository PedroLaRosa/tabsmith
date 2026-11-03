# Phase 9 run 4: accepted deviations and watch list

Run 4 (9.4a, 9.4b, 9.4c, 9.6c) verified at `6780aea`: gate 1326 passed, clippy 3 (budget 4; the `Modes::default()` lint died
at 9.4c), golden identical, windows ok, testlock vs phase9-start NEW 235 / CHANGED 55 / MISSING 0, vs ef807c1 NEW 1044 /
CHANGED 210 / MISSING 0, fixtures identical, diff scripts empty, `all_bundle_modules` 1 passed, 20 serial runs 0 failures.
phase9-greps still FAIL A1-A4 and B1 by design (pty.rs and the new `pty_command.rs`/`debug_shell_command.rs` still touch the
legacy `crate::pty` until 9.6d/9.6e/9.6f).

## Accepted (do not re-litigate)
- `build_terminal_session_module(&deps, settings, themes, generator_processes, start_completion_engine)`; getters
  `settings_repository()` (interim, dies with the LiveSettings port use in 9.5), `themes()`, `generator_processes()`,
  `session_contexts()` (`Rc<dyn>` over `FileSessionContextRepository`), `start_completion_engine()`, `clock()` (unused yet:
  Phase 11 must keep it), `live_settings()` (`LenientLiveSettings`), `working_directory()` (`ProcessWorkingDirectory`),
  `locate_data_root()`. `bootstrap::cli::terminal_session_module()` takes the themes from `build_dropdown_module(..).themes`
  (R28) and is built for every `debug`, `pty` and `pre-cmd` invocation (lazy, no I/O, no thread).
- Ports: `GeneratorProcessReaper` (+ `RecordingGeneratorProcessReaper`), `TerminalWriter` (+ `CapturingTerminalWriter`),
  `WrappedShell` (+ `CapturingWrappedShell`, unused until 9.5a: Phase 11 must keep it), `TerminalModes`, `LiveSettings`,
  `WorkingDirectory`; adapters `StdoutTerminalWriter`, `PtyWrappedShell` (`Session.pty` is the concrete `Rc<PtyWrappedShell>`),
  `ConsoleTerminalModes`, `LenientLiveSettings`, `ProcessWorkingDirectory`, `run_fallback_shell`. `IoJournal` has `note(Io)`.
  `LenientLiveSettings` duplicates the 3-line warning text of `load_or_defaults` on purpose (D5); a test pins the wording.
- `Session` field order is rx, engine, credits, pty, output (credits before the master: the ConPTY rule; SessionRunner must
  keep it, P9-T89). `Session::emit` is `&self`. `session()`/`session_named()` return `(Session, Taps)`. The `session()`
  fixture injects recording/fixed doubles directly, not through a module, so tests no longer reach the global
  `kill_live_children()`.
- cfg accounting (phase9-greps C1): 28 lines now against 23 at phase9-start: +3 (9.4b `RecordingMasterPty` and test cfgs),
  -1 (removed `if cfg!(test)` in `emit`), +2 (`console_terminal_modes.rs` platform aliases), plus the earlier 9.4b unix methods.
- `DebugCmd` is a private enum in bootstrap/cli.rs; `debug_logs` stays `pub fn` in the legacy cli.rs until Phase 10.
- `Session::start` still calls `std::env::current_dir()`, `user_home_from_env()` for the shell cwd (not a rule-3 hit; the dry run
  reports 0), builds `Rc::new(StdoutTerminalWriter)` itself, and reads `env!("CARGO_PKG_VERSION")`/`TMUX`.
- The `locate_data_root` test folds two outcomes in one assertion and `should_ignore_input_once_the_writer_is_gone` asserts the
  resize count (review nits, left: the brief fixes the NEW count).

## Phase 10/11 watch list (report only)
- cli.rs still carries `//! OWNER: cli agent` and the `// debug (SPEC §2.10)` comment; `diagnostic*` and `debug_logs` stay there.
- `debug_shell_post_script` builds a fresh `RenderInitScriptUseCase` per call (a third wiring place for Phase 10).
- The compat shim must be on PATH for the four older diff scripts on Linux (the gate adds it itself; do not export it before
  calling gate.sh).
