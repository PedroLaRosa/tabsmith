# Phase 9 run 5: accepted deviations and watch list

Run 5 (9.5a-9.5h) verified at `b7338e1`: gate 1404 passed, clippy 3 (budget 4), golden identical, windows ok, testlock vs
phase9-start NEW 314 / CHANGED 59 / MISSING 0 / DUPLICATE 0, vs ef807c1 NEW 1123 / CHANGED 210 / MISSING 0, fixtures identical,
diff scripts empty, `all_bundle_modules` 1 passed, 20 serial runs 0 failures. phase9-greps FAIL only A1-A4, B1, F4 (all close
in run 6).

## As built
- Application: `OverlayPainter`, `CompletionRequester`, `SessionFilePoller`, `KeystrokeHandler`, `ShellOutputFeeder` and the use
  cases `HandleKeystrokesUseCase`, `ProcessShellOutputUseCase`, `ApplyCompletionResultUseCase`, `ApplyLlmGhostTextUseCase`,
  `ResizeTerminalUseCase`, `HandleShellExitUseCase`, `HandleTimerTickUseCase`, `EndSessionUseCase`,
  `ApplyTerminalBackgroundUseCase`, `PrepareSessionLaunchUseCase`; `SessionPortsParts`, `SessionUseCases::new(ports)`;
  `TerminalSessionHarness` (`new`, `with_clock`, `with_engine`, `with_session_contexts`, `input`, `output`, `resize`, `end`,
  `tick`, `background`, `end_session`, `prompt`, `redraw_prompt`, `complete`, `reply`, `llm_reply`, `redraw`, `sent`,
  `engine_calls`, `drawable`, `ghost_text`, `written`, `clear_written`).
- `handle_keystrokes_use_case/tests.rs` exists (the test module was over 500 lines): list it in the as-built tree at 9.7.
  `PanickingCompletionEngine` lives in `ports/completion_engine.rs` next to `RecordingCompletionEngine`.
- `phase9-greps.sh` F5 ignores a `catch_unwind` that follows a `#[cfg(test)]` line + `mod tests` (design F5: inline
  `#[cfg(test)]` modules are read by eye).
- The `session()` fixture in pty.rs is NOT rebuilt over the harness (it keeps its channel-backed `PtyWrappedShell` and `Taps`);
  the Rig re-wires the use cases after swapping the engine / live settings (`rewire_use_cases`).

## Still alive at launch of run 6 (9.6d kills them)
- `SessionUseCases` has four `#[cfg(test)]` interim accessors (`painter()`, `requester()`, `poller()`, `keystroke_handler()`)
  and the four `#[cfg(test)]` `Rc` fields behind them; the tests of the services, the harness and the Rig use them. They
  were announced to die at 9.5g and did not; 9.6d removes them (move those tests onto use-case drivers or harness methods).
- The legacy `Session` still has delegates `on_timers`, `on_resize`, `on_completion`, `on_output`, `on_keys`, the `step` exit
  arms, `release_held`, test-only `insert`, test-only `drawable`, `cleanup`, and still holds `engine`, `modes`,
  `generator_processes`, `session_contexts`, `themes`, `screen`, `output`, `live_settings` which the use cases also hold.

## Phase 11 watch list (report only)
- `SessionPortsParts` fields that nothing reads after the wiring; `Session` fields redundant with the use cases' copies.
- The repeated shell-prints / feed-loop arrangement in four test `Scene` helpers (test-only; fold into the 9.6f harness work if
  cheap, otherwise leave).
- The F5 awk heuristic only recognises `#[cfg(test)]` on its own line directly followed by `mod tests`.
