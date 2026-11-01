# Phase 9 run 1: review notes for later steps

Recorded after the run 1 review (guards 9.0a-9.0d). Nothing here changes behaviour or a frozen fixture.

## Whole-ghost accept is not pinned by ghost.log

The test `should_accept_the_ghost_whole_and_by_word_exactly_as_at_the_phase_9_start` ends with a Right key
(`Right: the rest`). In the frozen `tests/fixtures/terminal_session/characterisation/ghost.log` that key is forwarded
raw to the PTY (`pty=\x1b[C`, no stdout): the by-word accept has already echoed `lo`, so no ghost applies to
`echo hello` any more. The by-word accept is pinned by ghost.log. The whole-ghost accept is pinned only by:

- `lock.log` (Right writes ` feature-login` to the PTY), and
- the unit test `ghost_keys_and_words_keys` (key classification only, not the accept itself).

Steps 9.3d, 9.3f and 9.5c, which touch ghost accept, must rely on those two and not on ghost.log. The fixture is frozen,
so a stronger pin means a new scenario, never an edit of ghost.log.

## The resize test opens a real pty

`should_resize_only_the_pty_while_passing_through` (9.0d) opens a real pseudo-terminal through
`portable_pty::native_pty_system().openpty(80x24)` and reads the size back with `get_size()`, where the 9.0d brief names a
small fake `MasterPty` that records `PtySize`. A fake `MasterPty` carries `#[cfg(unix)]` methods, which would change the
C1 cfg multiset. The test is positional (queue_resize(100, 30) gives rows 30, cols 100), so a swapped pair fails it.
It is not gated by `#[cfg(unix)]`, so it also runs against ConPTY on windows-latest CI (only compile-checked locally).
If it fails there or in a container without a pty device, gate it with `#[cfg(unix)]` or switch to a fake `MasterPty` in a
step that may add the cfg lines. The 9.6 rewrite of `LoopRig` keeps the real pty or adds the fake deliberately.
