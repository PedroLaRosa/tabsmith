# figterm (`qterm`) — the PTY wrapper behind Kiro / Amazon Q / Fig autocomplete

Research notes for **tabsmith**. Everything below comes from reading the reference sources. Anything I worked out rather than read directly is marked **(inferred)**.

Citation roots:
- `qac/` = `scratchpad/ref/qac` (aws/amazon-q-developer-cli-autocomplete)
- `ish/` = `scratchpad/ref/ish` (microsoft/inshellisense), used only for comparison

Main files:
- `qac/crates/figterm/src/*`: the wrapper binary. The crate is named `figterm` and the shipped binary is `qterm`.
- `qac/crates/alacritty_terminal/src/*`: a forked headless VT emulator that knows about shell-integration OSCs.
- `qac/proto/{figterm,remote,local,fig_common}.proto` and `qac/crates/fig_proto/src/{figterm.rs,remote_hooks.rs,lib.rs}`: the wire protocol.
- `qac/crates/fig_integrations/src/shell/scripts/{pre.sh,pre.fish,pre.nu,post.zsh,post.bash,post.fish,post.nu}`: the shell side.
- `qac/crates/q_cli/src/cli/internal/should_figterm_launch.rs`: the launch decision.
- `qac/crates/fig_util/src/{terminal.rs,consts.rs,directories.rs}` and `qac/crates/fig_os_shim/src/env.rs`: terminal and environment detection.

---

## 0. Architecture in one picture

```
 real terminal emulator (iTerm, kitty, VSCode, conhost/WT...)
        |  stdin (keys)                        ^ stdout (exact same bytes the shell wrote)
        v                                      |
 +--------------------------- qterm (figterm) ---------------------------+
 |  stdin reader (raw mode) -> InputParser -> KeyInterceptor --forward--> PTY master (write)
 |                                              |  swallow + InterceptedKeyHook -> app
 |  PTY master (read) --bytes--> alacritty_terminal::Processor/Term (shadow grid,
 |                     |            OSC 697 shell integration, FigFlags per cell)
 |                     +--------------------------------------> stdout (pass-through)
 |  after every read: get_current_buffer() -> EditBufferHook -> app (remote.sock)
 |  local socket  $SOCKETS/t/<session>.sock  <- `q _ ...` CLI helpers (inline, pre-cmd, ...)
 |  remote socket $SOCKETS/remote.sock       <-> desktop app (hooks out, InsertText/Intercept in)
 +---------------------------------------------------------------------------+
        |  PTY slave = controlling tty of the child
        v
   user shell (zsh/bash/fish/nu) with "post" integration printing ESC ] 697 ; ... BEL
```

figterm **never changes** what the shell prints. Every byte read from the PTY master goes to stdout unchanged (`main.rs:872-873`). In Fig/Q the autocomplete UI is a separate GUI window drawn by the desktop app. figterm only:
1. reports the edit buffer, cursor and shell context,
2. swallows keys while the UI is focused,
3. types text into the shell on request.

For tabsmith, the overlay will be drawn by the wrapper itself (inshellisense-style). That means our wrapper **will** write extra bytes to stdout. figterm has no prior art for that (see §20).

---

## 1. Names, constants, paths

| Thing | Value | Source |
|---|---|---|
| PTY binary name | `qterm` (old name: `cwterm`; the crate is still called figterm) | `fig_util/src/consts.rs:18,40` |
| CLI binary | `q` | `consts.rs:15` |
| Data dir name | `amazon-q` (unix), `AmazonQ` (windows) | `consts.rs:29-32` |
| Runtime/socket dir name | `cwrun` | `consts.rs:26` |
| Session id | `uuid::Uuid::new_v4().simple()` (32 lowercase hex chars), or `$MOCK_QTERM_SESSION_ID` if set | `figterm/src/main.rs:484-491` |
| Per-session socket | `sockets_dir()/t/<session_id>.sock`. On macOS `$TMPDIR/cwrun/t/…`; on Linux `$XDG_RUNTIME_DIR/cwrun/t/…`; on Windows `%TEMP%\AmazonQ\sockets\t\…` | `fig_util/src/directories.rs:238-246,382-384` |
| App ("remote") socket | `host_sockets_dir()/remote.sock`. On Linux over SSH this is `$Q_PARENT` instead | `directories.rs:353-374` |
| Log file | `logs_dir()/"qterm{pty_name}.log"`, where `logs_dir` = `runtime_dir()/qlog`. `pty_name` is e.g. `/dev/ttys003`, so this creates nested dirs **(inferred)** | `main.rs:510-515`, `directories.rs:295-304` |
| History DB | `fig_data_dir()/data.sqlite3`, where `fig_data_dir` = `dirs::data_local_dir()/amazon-q` (`~/Library/Application Support/amazon-q`, `~/.local/share/amazon-q`) | `fig_settings/src/sqlite/mod.rs:75`, `directories.rs:146-150` |
| Legacy history file | `fig_data_dir()/history`, mode 0600, append | `fig_settings/src/history.rs:131-175` |
| Main read buffer | 16384 bytes (`BUFFER_SIZE`) | `main.rs:143` |

### Environment variables figterm reads or writes

| Var | Meaning | Where |
|---|---|---|
| `Q_TERM` | Set **by figterm for the child** to figterm's version. Its presence means "already inside qterm". | `main.rs:434`, `consts.rs:121` |
| `Q_TERM_TMUX` | Set for the child (= version) only if `TMUX` is set in figterm's env. Lets a new qterm launch inside each tmux pane. | `main.rs:435-437` |
| `QTERM_SESSION_ID` | figterm `set_var`s it in its own env before building the child command, so the shell inherits it. The integration scripts echo it back in `NewCmd=`/`OSCLock=`/`OSCUnlock=`. | `main.rs:489-491` |
| `Q_SHELL` | Shell to spawn (falls back to `$SHELL`). Removed from the child env. | `main.rs:391-401,440` |
| `Q_IS_LOGIN_SHELL` | `"1"` means pass `--login`. Removed from the child env. | `main.rs:418-420,441` |
| `Q_EXECUTION_STRING` | If non-empty, pass `-c <string>`. Removed. | `main.rs:422-424,444` |
| `Q_SHELL_EXTRA_ARGS` | Split on whitespace; every `--login` dropped; appended. Removed. | `main.rs:426-428,443` |
| `Q_START_TEXT` | Type-ahead captured by pre.sh. Written to the PTY + `\n` once the first prompt is seen. Removed from the child env. | `main.rs:620-632,442` |
| `Q_PARENT` | Parent socket (SSH remote mode). Sent in the handshake as `parent_id`. | `main.rs:493`, `ipc.rs:272-277` |
| `Q_DISABLE_AUTOCOMPLETE` | Non-empty means never send edit buffers. | `main.rs:348-354,364-366` |
| `Q_PROMPT_OFFSET_WORKAROUND` | Integer N, subtracted from the cmd-start column at `NewCmd` (emoji-width workaround). | `alacritty_terminal/src/term/mod.rs:835-848`, `fig_os_shim/src/env.rs:230-232` |
| `Q_LOG_LEVEL` | Log level. Also settable live through the `Log=` OSC. | `main.rs:980`, `term/mod.rs:1910-1919` |
| `Q_INLINE_SHELL_COMPLETION_CACHE_DISABLE`, `Q_INLINE_SHELL_COMPLETION_HISTORY_COUNT` (default 49), `Q_INLINE_SHELL_COMPLETION_DEBOUNCE_MS` (default 300) | Inline ghost-text tuning | `inline/mod.rs:55-81`, `env.rs:214-224` |
| `PROCESS_LAUNCHED_BY_Q` | Set on processes figterm spawns (RunProcess). The pre script refuses to launch qterm when it is set. | `message.rs:140-146`, `pre.sh:35` |

### Settings and state keys figterm reads

- `qterm.enabled` (state, default true). If false, figterm prints `[NOTE] qterm is disabled. Autocomplete will not work.` and **returns without launching a shell** (`main.rs:982-986`). Because pre.sh `exec`ed qterm, the terminal session ends. This is a bug; don't copy it.
- `user.enabled-shells` (state, `Vec<String>`). Extra shell names allowed to send edit buffers (`main.rs:152-157`).
- `qterm.csi-u.enabled` (settings, default false). See §6.4 (`event_handler.rs:49`).
- `autocomplete.onlyShowOnTab` (settings, default false). See §12 (`interceptor.rs:23-24`).
- `ai.terminal-hash-sub` (settings, default true). The "`# question` + Enter → `q translate`" feature. **Out of scope; drop it** (`main.rs:606,730-753`).
- `pty.suppress_log_error` (state) (`main.rs:518`).
- `app.disableAutoupdates`. Used in update.rs, see §17.
- `autocomplete.keybindings.<binding>` = `<actionId>` (settings). Read by the **app**, which forwards them to figterm as actions (`fig_desktop/src/request/properties.rs:33-57`, `fig_settings/src/keybindings.rs:65-89`).
- `inline.enabled` (settings, default true). Read by `q init` to decide whether to source the zsh ghost-text plugin, and by `q inline` (`q_cli/src/cli/init.rs:155-174`, `q_cli/src/cli/inline.rs:58-75`).

---

## 2. How figterm gets launched

### 2.1 Shell rc integration ("pre" block)

`q integrations install dotfiles` adds `eval "$(q init zsh pre)"` at the top of the rc file and `… post` at the bottom. The `pre` output first assigns `SHOULD_QTERM_LAUNCH=<exit code of should_figterm_launch_exit_status(quiet)>` (`q_cli/src/cli/init.rs:146-153`), then includes `pre.sh`. Templates use `{{CLI_BINARY_NAME}}` = `q` and `{{PTY_BINARY_NAME}}` = `qterm`.

`pre.sh` is used for bash and zsh (`fig_integrations/src/shell/mod.rs:197-207`). In order (`pre.sh:1-77`):
1. Prepend `~/.local/bin` to `PATH`.
2. If `Q_NEW_SESSION` is set: unset `QTERM_SESSION_ID`, `Q_TERM` and `Q_NEW_SESSION`. This forces a fresh qterm.
3. If `Q_SET_PARENT_CHECK` is unset: if `Q_PARENT` is empty and `Q_SET_PARENT` is set, export `Q_PARENT=$Q_SET_PARENT` and unset `Q_SET_PARENT`. Then export `Q_SET_PARENT_CHECK=1`.
4. If `SHOULD_QTERM_LAUNCH` is still empty, run `q _ should-figterm-launch` and use its exit code. The codes are **0 = launch, 1 = don't, 2 = fall back to the `Q_TERM` check**.
5. Launch only if **all** of these hold:
   - `[[ -t 1 ]]` (stdout is a tty)
   - `PROCESS_LAUNCHED_BY_Q` is empty
   - `qterm` is on PATH
   - `SHOULD_QTERM_LAUNCH == 0`, **or** (`== 2` **and** (`Q_TERM` is empty **or** (`Q_TERM_TMUX` is empty **and** `TMUX` is non-empty)))

   This is the nesting guard. A shell already under qterm has `Q_TERM`, so it doesn't launch again, except inside tmux panes, which get their own qterm.
6. `Q_SHELL=$(q _ get-shell)` if unset. `get-shell` prints the **parent process exe path**, i.e. the shell that is sourcing the rc (`fig_proto/src/util.rs:9-13`).
7. `Q_IS_LOGIN_SHELL=1` if bash `shopt -q login_shell` or zsh `-o login`.
8. Only for interactive shells (`$- == *i*`):
   - `Q_TERM_NAME="$(basename $Q_SHELL) (qterm)"`, e.g. `zsh (qterm)`.
   - `Q_TERM_PATH` = `~/.local/bin/zsh (qterm)` if that file is executable, else `command -v qterm`.
   - `Q_EXECUTION_STRING=${BASH_EXECUTION_STRING:=$ZSH_EXECUTION_STRING}`.
   - Drain already-typed stdin with `read -rt 0` loops into `INITIAL_TEXT` (skipped on bash 3).
9. `exec -a "$Q_TERM_NAME" "$Q_TERM_PATH"` with env `Q_EXECUTION_STRING`, `Q_START_TEXT`, `Q_SHELL` and `Q_IS_LOGIN_SHELL`.

   This **replaces the user's shell process with qterm** (same pid, same tty). `argv[0]` becomes `zsh (qterm)`, so the terminal tab title still reads like the shell. qterm also writes `ESC ] 0 ; <shell basename> BEL` to stdout at startup (`main.rs:608-616`).

The other shells:
- **fish** (`pre.fish:1-51`): same logic, using `status --is-login` / `--is-interactive`. fish's `exec` can't set argv0, so it does `exec bash -c "Q_PARENT=… Q_SHELL=… Q_IS_LOGIN_SHELL=… exec -a \"$Q_TERM_NAME\" \"$Q_TERM_PATH\""`.
- **nu** (`pre.nu`): legacy. It still references `figterm` and `~/.fig/bin`, and only checks `Q_TERM`, not tmux.

### 2.2 `q _ should-figterm-launch` decision tree

From `q_cli/src/cli/internal/should_figterm_launch.rs:206-295` (macOS/Linux). When not quiet, it prints ✅/❌ lines. Checks run in order:

1. `Q_FORCE_FIGTERM_LAUNCH` set → **0**
2. `Q_TERM_DISABLED` set → **1**
3. `PROCESS_LAUNCHED_BY_Q` set → **1**
4. `INSIDE_EMACS` set → **1**
5. `TERM_PROGRAM == "WarpTerminal"` → **1**
6. `__CFBundleIdentifier == "com.vandyke.SecureCRT"` → **1**
7. `__PWSH_LOGIN_CHECKED` set → **1**
8. `CI` or `Q_CI` set → **1**
9. In SSH (`SSH_CLIENT`, `SSH_CONNECTION` or `SSH_TTY` set) and `Q_PARENT` unset → **1**
10. In WSL (`/proc/sys/kernel/osrelease` contains `microsoft` or `wsl`) → **2**
11. Otherwise `should_launch()` (`:165-203`):
    - **Parent** = the shell running the rc. It must have an exe whose file name is in `["zsh","bash","fish","nu"]` (`:78`).
    - Shortcuts: in SSH with `Q_TERM` unset → **0**. In Codespaces (`CODESPACES`/`Q_CODESPACES`) → `Q_TERM` set ? **1** : **0** (`:80-89`).
    - **Grandparent** = whatever launched the shell. Its exe name, or on Linux its `/proc/pid/cmdline`, must match a known terminal from `MACOS_TERMINALS`/`LINUX_TERMINALS` + `SPECIAL_TERMINALS` (§18) (`:100-162`).
    - On macOS, if the grandparent is **not** a "special" terminal (ssh, tmux, vim, nvim, zellij) → **2**, the old `Q_TERM` mechanism (`:195-200`).
    - Otherwise `0` if both parent and grandparent are valid, else `1` (`:202`).

On Windows, `should_qterm_launch` returns 2 if `GetConsoleMode(stdin)` succeeds, else 1 (`:302-311`).

### 2.3 figterm `main()` sequence

From `figterm/src/main.rs:976-1005` and `478-974`.

1. clap CLI: `qterm [-- <cmd> <args>...]`. The optional trailing command replaces the shell (`cli.rs:3-8`).
2. If `qterm.enabled == false`, print the note and return (see the bug in §1).
3. `figterm_main(command)`:
   1. Create the session id and `set_var(QTERM_SESSION_ID)`.
   2. `parent_id = $Q_PARENT`.
   3. `SystemTerminal::new_from_stdio()`. **Fails unless stdin and stdout are both ttys** (`term/unix.rs:202-211`). It dups the fd and saves the original termios.
   4. `screen_size = TIOCGWINSZ(stdout dup)`. Open the PTY with that size (rows, cols, xpixel, ypixel).
   5. `build_shell_command(command)` (§2.4). Init logging.
   6. Spawn the child. A thread waits for it; the result arrives through a oneshot (`child_rx`).
   7. Build a multi-thread tokio runtime with threads named `qterm-runtime-worker-N`. Inside it:
      - `update::check_for_update`
      - **`terminal.set_raw_mode()`**
      - Main-loop channel: flume **bounded(16)**
      - Spawn the history task
      - Bind the per-session socket
      - Start the remote IPC loop
      - `Term::new(size, EventHandler, max_scroll_limit = 1, session_id)` (`main.rs:581`)
      - On Windows: `set_windows_delay_end_prompt(true)`
      - Load default key intercepts
      - A 16 ms interval
      - Start the stdin reader
      - Write the title OSC
      - Enter the select loop (§5)
   8. On exit: send the stop signal to the IPC task, flush telemetry, `runtime.shutdown_background()` (stdin reading is a blocking thread), then `cleanup::cleanup()` on Linux (a no-op, §17). Termios is restored by `UnixTerminal`'s `Drop` (`term/unix.rs:336-342`).
4. On **any error** from `figterm_main`, print `"Amazon Q had an Error!: …"` and, on Unix, **`launch_shell(command)`**. That rebuilds the same command, applies its env, and `execvp`s the shell directly, so the user still gets a shell (`main.rs:453-476,992-1003`).

### 2.4 Child command

From `main.rs:403-451`, plus `pty/cmdbuilder.rs` (copied from wezterm `portable-pty`).

- With an explicit command: argv as given.
- Otherwise: program = `$Q_SHELL || $SHELL` (error if neither), `--login` if `Q_IS_LOGIN_SHELL=1`, `-c $Q_EXECUTION_STRING`, then the extra args.
- Env:
  - The base env is a snapshot of figterm's env at `CommandBuilder::new` time, so it includes `QTERM_SESSION_ID`.
  - Adds `Q_TERM=<version>`, and `Q_TERM_TMUX=<version>` if `TMUX` is set.
  - Removes `Q_SHELL`, `Q_IS_LOGIN_SHELL`, `Q_START_TEXT`, `Q_SHELL_EXTRA_ARGS`, `Q_EXECUTION_STRING`.
- cwd = figterm's cwd. `as_command` falls back to `$HOME` if that isn't a directory (`cmdbuilder.rs:274-315`).
- Non-default program: argv0 = `args[0]`, and the program is resolved through PATH.
- figterm does **not** set `TERM` (it is inherited from the real terminal).
- figterm does **not** copy the termios of the real tty to the slave; that code is commented out (`pty/unix.rs:110-112`). The slave starts with kernel default termios, and the shell configures its own.

---

## 3. PTY and terminal I/O

### 3.1 Unix PTY

From `pty/unix.rs`.

- `posix_openpt(O_RDWR | O_NONBLOCK)` on macOS and Linux. On macOS O_NONBLOCK must be set at open time, not with fcntl later. Then `grantpt`, `unlockpt`, `ptsname`, `open(slave, O_RDWR)`.
- `ioctl(slave, TIOCSWINSZ, {rows, cols, xpixel, ypixel})`. Both fds get `FD_CLOEXEC` (`:86-141`).
- Spawn (`:144-207`): stdin/stdout/stderr = the slave fd. The `pre_exec` closure in the child:
  - resets `SIGCHLD, SIGHUP, SIGINT, SIGQUIT, SIGTERM, SIGALRM` to `SIG_DFL`
  - `setsid()`
  - `ioctl(0, TIOCSCTTY, 0)` to make the PTY the controlling tty; without it SIGWINCH is never delivered
  - `close_random_fds()`
  - optional `umask`
- Async master: `tokio::io::unix::AsyncFd<PtyMaster>` with readable/writable guards (`:214-236`).
- `resize`: `ioctl(master, TIOCSWINSZ)`. The kernel then sends SIGWINCH to the child's foreground process group (`:238-254`).

### 3.2 Windows ConPTY

From `pty/win/*.rs`.

- `CreatePseudoConsole` is loaded from a side-loaded `conpty.dll` if present, else `kernel32.dll`.
- Flags: `PSEUDOCONSOLE_RESIZE_QUIRK (0x2) | PSEUDOCONSOLE_WIN32_INPUT_MODE (0x4) | PSEUDOCONSOLE_PASSTHROUGH_MODE (0x8)` (`pseudocon.rs:50-52,66-80,103-107`).
- Spawned with `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE = 0x00020016` (`procthreadattr.rs:16,54`).
- The console host terminal (`term/windows.rs`):
  - Enables `ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN` on output (`:554-562`).
  - Raw mode clears `ECHO`, `LINE` and `PROCESSED` input flags and adds `MOUSE | WINDOW` (`:566-581`).
  - Input comes from `ReadConsoleInputW` records, decoded into the same `InputEvent`s. `WINDOW_BUFFER_SIZE_EVENT` → `Resized` (`:650-690`, `input/mod.rs:1180-1192`).
  - The cursor position comes from `GetConsoleScreenBufferInfo` and is sent as `terminal_cursor_coordinates` in edit buffer hooks, **Windows only** (`main.rs:211-230`, `term/windows.rs:617-623`).
- Note: `ipc.rs` uses `tokio::net::UnixListener` unconditionally (`ipc.rs:175`), so the Windows path is effectively legacy/unbuilt **(inferred)**.

### 3.3 Raw mode (Unix)

- `tcgetattr` on a **dup of stdout**, `cfmakeraw`, `tcsetattr(TCSAFLUSH)` (drain output and purge pending input), then `tcdrain` (`term/unix.rs:226-235`).
- Restored on `Drop` with `TCSANOW` to the termios saved at construction (`:336-342`).

### 3.4 Stdin reader, batching and SIGWINCH

From `term/unix.rs:271-333`.

- A tokio task reads `stdin.read_buf` into a `BytesMut` (capacity 16384, doubled when full). An `InputParser` turns the bytes into `Vec<Result<(Option<Bytes> raw, InputEvent)>>`, sent over flume **bounded(1)**.
- **Immediate vs batched mode.** `IMMEDIATE_MODE` is a static `AtomicBool`, initially `true`.
  - Immediate: every read is parsed and dispatched at once.
  - Batched: a 1 ms interval (`MissedTickBehavior::Delay`) is `reset()` after every read, and the buffer is parsed only when a tick fires with a non-empty buffer and immediate mode off. That is a 1 ms quiet-period debounce, which merges multi-write key sequences.
  - At a prompt (`Event::Prompt`) the handler sends `SetImmediateMode(false)`. While a command runs (`Event::PreExec`) it sends `SetImmediateMode(true)` (`event_handler.rs:82-84,102-104`).
  - On Windows `set_immediate_mode` is a no-op.
- Parsing always uses `maybe_more = false`, so a dangling ESC at the end of a batch becomes the Escape key.
- **SIGWINCH**: `tokio::signal::unix::signal(SignalKind::window_change())` produces `InputEvent::Resized` with `raw = None`. The main loop (`main.rs:799-814`):
  1. `terminal.flush()`
  2. re-reads `TIOCGWINSZ`
  3. `master.resize(PtySize)`
  4. `term.resize(SizeInfo{rows, cols})`. The grid reflows on the primary screen (`term/mod.rs:450-476`). **`cmd_cursor` is not adjusted on resize** **(inferred from code)**. It gets fixed on the next `NewCmd`; zsh re-emits the prompt on SIGWINCH.

---

## 4. The headless terminal (`alacritty_terminal` fork)

- vte `0.15` parser with `default-features = false` (`alacritty_terminal/Cargo.toml`). vte splits OSC parameters on `;`. Without `std`, OSC raw data is a fixed `ArrayVec` (I believe 1024 bytes) and longer OSCs get truncated **(inferred from vte's no_std design; unverified)**.
- The `Processor` also implements DCS synchronized updates (`ESC P = 1 s` … `ESC P = 2 s`): bytes are buffered for up to 150 ms or 2 MiB (`ansi.rs:36-48,180-264`).
- Grid: standard alacritty grid with cursor, a cursor template cell, saved cursor, alt screen and tabstops every 8 columns. **History (scrollback) is 1 line** (`Term::new(..., 1, ...)`, `main.rs:581`).
- Cell: `c: char`, fg, bg, `ShellFlags` (INVERSE, BOLD, ITALIC, UNDERLINE, WRAPLINE, WIDE_CHAR, WIDE_CHAR_SPACER, DIM, HIDDEN, STRIKEOUT, LEADING_WIDE_CHAR_SPACER, DOUBLE_UNDERLINE), zero-width extras, and the fork-specific **`FigFlags`**: `IN_PROMPT = 0b01`, `IN_SUGGESTION = 0b10` (`term/cell.rs:19-45,77-86`). A default/cleared cell is `' '` with default colors and empty FigFlags (`cell.rs:88-100,161-166`).
- `write_at_cursor` copies the **template's** fg, bg, flags and fig_flags into the cell (`term/mod.rs:631-674`). That is how prompt and suggestion marks reach cells.
- It **never answers terminal queries** (DA, DSR/CPR, …). The real terminal answers them because output passes through. Replies come back on stdin as ordinary input and are forwarded byte-exact (see §6.1).
- The emulator is purely a shadow. Its modes that figterm reads: `BRACKETED_PASTE` (for inserts) and `ALT_SCREEN` indirectly through the grid.

### 4.1 Shell state kept on the Term

From `term/mod.rs:160-246`.

```rust
struct ShellContext { pid: Option<i32>, tty: Option<String>, shell: Option<String>, shell_path: Option<PathBuf>,
                      wsl_distro: Option<String>, current_working_directory: Option<PathBuf>,
                      session_id: Option<String> /* = our QTERM_SESSION_ID */, username: Option<String> }
struct ShellState  { local_context: ShellContext, has_seen_prompt: bool, preexec: bool /* default false */,
                     cmd_cursor: Option<Point> /* where the edit buffer starts */,
                     fish_suggestion_color, zsh_autosuggestion_color, fig_autosuggestion_color, nu_hint_color: Option<SuggestionColor>,
                     color_support: Option<ColorSupport>, command_info: Option<CommandInfo>, fig_log_level: Option<String>,
                     osc_lock: bool }
struct CommandInfo { command, shell, pid, session_id, cwd, start_time, end_time, username, exit_code }
```

`Point { line: Line(i32), column: Column(usize) }`. Line 0 is the top of the viewport, negative lines are scrollback, and points order by (line, column).

---

## 5. The main select loop

From `main.rs:619-955`. The `select!` is **`biased`**, so branches are polled in this order.

0. Before every iteration: on the first time `has_seen_prompt` is true, write `Q_START_TEXT + "\n"` to the master once (`:620-632`).
1. **`main_loop_rx`** (`MainLoopEvent`):
   - `Insert{insert, unlock, bracketed, execute}` (`:639-660`):
     - If `bracketed`:
       - If the shadow term has `BRACKETED_PASTE` mode: write `ESC[200~` + insert with every `ESC` byte **removed** + `ESC[201~`.
       - Otherwise write the insert with `\r\n`→`\r` and then `\n`→`\r`.
     - Else write it raw.
     - If `execute`, then write `\r`. If `unlock`, then `key_interceptor.reset()`.
   - `UnlockInterception` → `reset()`.
   - `SetImmediateMode(b)`.
   - `SetCsiU` → write `ESC[>1u` to **stdout** (push kitty keyboard flags = 1, disambiguate) and flush. `UnsetCsiU` → `ESC[<u` (pop).
   - `PromptSSH` → no-op; that code is commented out.
2. **`input_rx`**: a batch of input events (§6). All bytes to forward are accumulated in one `BytesMut` and written to the master with a single `write_all` at the end of the batch (`:840`).
3. **`master.read(16 KiB)`** (`:848-893`):
   - `Ok(0)` (EOF) or `Err` → leave the loop.
   - Otherwise: feed **byte by byte** to `processor.advance(&mut term, b)`.
   - Windows delayed events: if `delayed_count > 0 && delayed_count == old_delayed_count` (this chunk added no new delayed events), call `flush_delayed_events()`.
   - Then `stdout.write_all(bytes)` + `flush`. The shadow state is updated **before** the bytes reach the real terminal.
   - Then, if `can_send_edit_buffer(&term)`, `send_edit_buffer` (§10.6).
4. **`remote_receiver`**: messages from the desktop app → `process_remote_message` (§14.3).
5. **`incoming_receiver`**: messages on the per-session socket → `process_figterm_message` (§14.2).
6. **16 ms interval**: if an insertion lock is set (`INSERTION_LOCKED_AT.is_some()`) and `can_send_edit_buffer`, send the edit buffer. This flushes the buffer once the lock clears (`:935-943`).
7. **`child_rx`**: the shell exited → leave the loop.

Any branch returning `Err` ends the loop with that error (`:951-954`).

---

## 6. Input path (keys → shell)

### 6.1 InputParser

This is wezterm's termwiz, vendored in `input/mod.rs` and `input/keymap.rs`.

- A trie (`KeyMap`) of byte sequences → `InputEvent::Key(KeyEvent{key, modifiers})`. The table is built in `build_basic_key_map` (`input/mod.rs:1212-1537`). Later inserts override earlier ones. Notable entries:
  - `0x01..=0x1a` → `Char('A'..='Z')` + CTRL, **uppercase** (`:1247-1257`). Overridden afterwards:
    - `0x09` → `Tab`
    - `0x0d` and `0x0a` → `Enter` (so **Ctrl-J and Ctrl-M arrive as Enter**)
    - `0x08` → `Backspace`
  - `0x7f` → `Backspace`.
  - `ESC <A-Z>` → `Char(A-Z)` + ALT.
  - `ESC [ <0..127> [;mods] u` → CSI-u chars. Modifier suffixes `;1`..`;16`: 2 = SHIFT, 3 = ALT, 5 = CTRL, 9+ add META.
  - `ESC [ A/B/C/D/H/F` → Up/Down/Right/Left/Home/End; `ESC [1;<m>X` → the same with modifiers.
  - `ESC O A/B/C/D` → **`ApplicationUpArrow/…`**, which are distinct KeyCodes (`:1334-1359`).
  - `ESC O F/H/I/M/P/Q/R/S/X` → End/Home/Tab/Enter/F1-F4/'='.
  - `ESC [ <n> ~` with `~ $ ^ @` suffixes → Insert/Delete/Home/End/PgUp/PgDn.
  - `ESC [ Z` → Tab + SHIFT.
  - `ESC` → Escape.
  - `ESC[200~` / `ESC[201~` → internal paste markers.
- State machine (`:1574-1752`):
  - If the trie gives an exact `ESC` match and more bytes follow, enter `EscapeMaybeAlt`. If the next decoded key is a Key, emit it with ALT added. Otherwise emit Escape, then the event.
  - Unknown bytes are decoded as one UTF-8 char → `Char(c)`.
  - Bracketed paste: in `Pasting` state, search for `ESC[201~`; the content becomes `InputEvent::Paste(String)`.
  - **Every event carries the exact raw bytes that produced it** (`raw_byte_stack`). The raw bytes of a paste include the `ESC[200~ … ESC[201~` markers. Because `raw` is `Some`, the main loop's `(None, Paste)` arm never matches on Unix; it falls through to the "fallback write" arm, which forwards the raw bytes verbatim (`main.rs:815-832`) **(inferred from code)**.
  - Mouse parsing is disabled (`:1660-1704`).

### 6.2 What gets forwarded

For each `Key` event (`main.rs:724-798`):
1. `preexec = term.shell_state().preexec`.
2. (Drop for tabsmith) The `#…` + Enter translate hack.
3. Choose the bytes to forward. If `csi_u_set` is true, re-encode with `KeyCode::encode(mods, {encoding: Xterm (unix) / Win32, app_cursor_keys: false, newline_mode: false}, is_down = true)`, falling back to raw. Otherwise use **raw, falling back to the encoding** (`:755-766`).
4. If **not** `preexec` (we are at a prompt), ask `key_interceptor.intercept_key(&event)` (§12). If it returns an action:
   - send `InterceptedKeyHook{context, action, key: String::from_utf8(raw bytes)}` to the app
   - if the key is `Escape`, `reset()` the interceptor
   - **do not forward the key**
5. Otherwise forward the bytes. If the key is Ctrl+C or Ctrl+D (KeyCode `Char('c')`/`Char('d')` with **exactly** CTRL), `reset()` the interceptor first. Because the parser produces uppercase `Char('C')` for `0x03`, this lowercase check only matches CSI-u-encoded keys. **Ctrl-C from a legacy terminal does not reset interception** **(inferred; looks like a bug)**.

Other events:
- `Resized`: see §3.4.
- `Paste`: see §6.1.
- Anything else with raw bytes: forwarded as is.

### 6.3 Output → shell side effects of input

None. figterm does **not** echo; the shell echoes through the PTY.

### 6.4 CSI-u (`qterm.csi-u.enabled`, default off)

On each prompt figterm writes `ESC[>1u` to the real terminal, so it reports keys like Ctrl+1 or Shift+Enter unambiguously. On preexec it writes `ESC[<u`. While it is set, figterm re-encodes keys to legacy xterm before sending them to the shell, so the shell never sees CSI-u (`event_handler.rs:90-94,110-114`, `main.rs:669-680,755-766`).

---

## 7. Output path (shell → screen)

The bytes are fed to the shadow emulator, then written unchanged to stdout. The emulator does the usual VT work (cursor movement, erase, scroll regions, insert/delete chars and lines, wide chars, alt screen…) and, on top of that, the fig-specific OSC 697 handling (§8) and per-cell semantic marks (§9).

`cmd_cursor` follows scrolling:
- `scroll_up_relative(origin, n)`, used by linefeed at the bottom and `CSI S`, does `cmd_cursor.line -= n`.
- `scroll_down_relative` does `cmd_cursor.line += n`.

Both adjust **regardless of the origin or scroll region**, and before clamping `n` (`term/mod.rs:506-539`).

---

## 8. Shell-integration protocol: OSC 697

**Wire format:** `ESC ] 6 9 7 ; <payload> BEL`, i.e. `\x1b]697;<payload>\x07`. The shell scripts always terminate with BEL; ST (`ESC \`) would also parse in vte. Parsing happens in `alacritty_terminal/src/ansi.rs:1175-1272`.

- `params[0] == "697"`. `params[1]` is the payload, which is **only the text up to the first `;`**, because vte splits OSC params on `;` and figterm reads only `params[1]`. **A path containing `;` is truncated.**
- If `params[1]` is exactly `NewCmd`, `StartPrompt`, `EndPrompt` or `PreExec`, it is a bare command.
- Otherwise it is split at the **first `=`** into key and value. If the value is empty (`val.len() <= 1`, i.e. only `=`), the OSC is **ignored**. So `Log=` and `ExitCode=` with empty values are no-ops. The value must be UTF-8; numbers are parsed as `i32`.
- Unknown keys, and `DoneSourcing` (emitted by the post scripts when `PROCESS_LAUNCHED_BY_Q` is set), are ignored.

### 8.1 Every code

| Payload | Handler | Ignored while `osc_lock`? | Effect | Source |
|---|---|---|---|---|
| `StartPrompt` | `start_prompt` | yes | `has_seen_prompt = true`; cursor template `fig_flags |= IN_PROMPT`, so every cell written from now on is marked as prompt | `term/mod.rs:1730-1738` |
| `EndPrompt` | `end_prompt` | yes | template `fig_flags &= !IN_PROMPT`. On Windows it is queued as a DelayedEvent unless forced | `:1741-1746,796-804` |
| `NewCmd` (bare) or `NewCmd=<sid>` | `new_cmd` | **no** (uses the session check instead) | If `<sid>` is non-empty **and differs from our session id**, ignore. Bare/empty is accepted (legacy). Windows: delayed. Otherwise: `cmd_cursor = grid.cursor.point`; apply `Q_PROMPT_OFFSET_WORKAROUND` (`column = max(0, column - N)`); **`preexec = false`**; emit `Event::Prompt`; if `command_info` is pending, `take()` it and emit `Event::CommandInfo(&info)` | `:806-860` |
| `PreExec` | `pre_exec` | **no** (guarded only by `if preexec { return }`) | `preexec = true`; emit `Event::PreExec`. Then snapshot `command = get_current_buffer().buffer.trim()` and create `command_info = { command, shell, pid, session_id, cwd: figterm's current_dir, start_time: now, username, exit_code: None, end_time: None }` | `:1749-1772` |
| `Dir=<path>` | `dir` | yes | `context.cwd = path`; **`std::env::set_current_dir(path)` on figterm itself**, logging an error on failure | `:1775-1785` |
| `ShellPath=<path>` | `shell_path` | yes | `context.shell_path` | `:1788-1793` |
| `WSLDistro=<name>` | `wsl_distro` | yes | `context.wsl_distro = trim(name)` | `:1796-1801` |
| `ExitCode=<i32>` | `exit_code` | yes | If `command_info` exists, set `exit_code` and `end_time = now` | `:1804-1813` |
| `Shell=<name>` | `shell` | yes | `context.shell = trim(name)`; emit `Event::ShellChanged` if it changed (that handler is a no-op) | `:1816-1830` |
| `FishSuggestionColor=<fish color spec>` | `fish_suggestion_color` | yes | `fish_suggestion_color = shell_color::parse_suggestion_color_fish(spec, color_support)` | `:1833-1842` |
| `ZshAutosuggestionColor=<zle highlight spec>` | `zsh_suggestion_color` | yes | `zsh_autosuggestion_color = Some(parse_suggestion_color_zsh_autosuggest(spec, color_support))` | `:1845-1857` |
| `FigAutosuggestionColor=<spec>` | `fig_suggestion_color` | yes (also ignored if empty) | `fig_autosuggestion_color = Some(parse_suggestion_color_zsh_autosuggest(…))`. **Takes precedence over Zsh…** | `:1860-1872` |
| `NuHintColor=<spec>` | `nu_hint_color` | yes (also ignored if empty) | `nu_hint_color = Some(parse_hint_color_nu(spec))` | `:1875-1881` |
| `TTY=<path>` | `tty` | yes | `context.tty = trim` | `:1884-1891` |
| `PID=<i32>` | `pid` | yes | `context.pid` | `:1894-1900` |
| `User=<name>` | `username` | **no** | `context.username = trim` | `:1903-1907` |
| `Log=<level>` | `log` | yes | `fig_log_level`; `event_proxy.log_level_event(level)` → `fig_log::set_log_level` (live reload) | `:1910-1919`, `event_handler.rs:141-145` |
| `OSCLock=<sid>` | `osc_lock` | — | If `sid == our session id` (or we have none), `osc_lock = true` | `:1922-1934` |
| `OSCUnlock=<sid>` | `osc_unlock` | — | If `sid == our session id`, `osc_lock = false` | `:1937-1949` |
| `Hostname=` | (commented out) | — | ignored | `ansi.rs:1242-1245` |

**Why the lock exists:** while a command runs, any nested shell's integration output also flows through this PTY: an ssh session, a nested qterm, `bash` started by hand… Those nested shells would otherwise overwrite our cwd, shell name and prompt marks. The outer shell sends `OSCLock=<our sid>` in preexec and `OSCUnlock=<our sid>` first thing in precmd. A nested shell without our session id can't unlock: an empty value is ignored and a different value is rejected. Gaps:
- `PreExec`, `NewCmd` (bare) and `User` are not lock-guarded.
- A nested plain subshell inherits `QTERM_SESSION_ID` and can therefore unlock **(inferred)**.

### 8.2 What each shell emits, in order

**zsh** (`post.zsh`; `fig_osc() { printf "\033]697;$1\007" "${@:2}"; }`):
- **precmd `fig_precmd`**, kept last in `precmd_functions`:
  `OSCUnlock=$QTERM_SESSION_ID`, `Dir=$PWD`, `Shell=zsh`, `ShellPath=${Q_SHELL:-$SHELL}`, `[WSLDistro=…]`, `PID=$$`, `ExitCode=$?` (captured first), `TTY=$TTY`, `Log=$Q_LOG_LEVEL`, `ZshAutosuggestionColor=$ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE`, `FigAutosuggestionColor=$Q_AUTOSUGGEST_HIGHLIGHT_STYLE`, `User=${USER:-root}`.
  - If the prompt was already wrapped (Ctrl-C at the prompt), it calls `fig_preexec` first, which un-wraps and emits Lock+PreExec.
  - It then saves the user prompts and wraps them (`%{…%}` makes the sequences zero-width for ZLE):
    - `PS1`/`PROMPT`/`prompt` → `%{SP%}<orig>%{EP NC%}`
    - `PS2`/`PROMPT2` → `%{SP%}<orig>%{EP%}` (**no NewCmd** for continuation lines)
    - `PS3`/`PROMPT3` → `SP…EP NC`
    - `PS4`/`PROMPT4` → `SP…EP`
    - `RPS1`/`RPROMPT` → `SP…EP`
    - `RPS2`/`RPROMPT2` → `SP…EP`
    - where SP = `\e]697;StartPrompt\a`, EP = `\e]697;EndPrompt\a`, NC = `\e]697;NewCmd=$QTERM_SESSION_ID\a`.
  - Finally it runs `q _ pre-cmd --alias "$(\alias)"` in the background (§14.2) (`post.zsh:77-171`).
- **preexec `fig_preexec`**, kept first in `preexec_functions`: restores every saved user prompt, then `OSCLock=$QTERM_SESSION_ID`, `PreExec` (`:24-75`).

**bash** (`post.bash` + bash-preexec):
- `__fig_pre_prompt`, first in `precmd_functions`: `OSCUnlock`, `Dir`, `Shell=bash`, `ShellPath`, `[WSLDistro]`, `PID`, `ExitCode`, `TTY`, `Log`, `User`, then `q _ pre-cmd` in the background. If preexec didn't run (Ctrl-C bug on CentOS), it calls preexec manually.
- `__fig_post_prompt`, appended last to `PROMPT_COMMAND`:
  `PS1="\[SP\]$Q_USER_PS1\[EP\]\[NC\]"`, `PS2="\[SP\]…\[EP\]"`, `PS3="\[SP\]…\[EP\]\[NC\]"`.
- `__fig_preexec`: `OSCLock`, `PreExec`, then restore the PS vars.
- No suggestion color is sent for bash (`post.bash:27-115`).

**fish** (`post.fish`):
- On the `fish_prompt` event: `OSCUnlock`, `Dir`, `Shell=fish`, `ShellPath`, `[WSLDistro]`, `PID=$fish_pid`, `ExitCode`, `TTY`, `Log`, `FishSuggestionColor=$fish_color_autosuggestion`, `User`.
- Wrapping: `fish_prompt` → `SP <prompt> EP NC`; `fish_right_prompt` and `fish_mode_prompt` → `SP … EP`.
- On `fish_preexec`: `OSCLock`, `PreExec`.

**nu** (`post.nu`): same scheme plus `NuHintColor=$env.config.color_config.hints`. The vi insert/normal prompt indicators end with `EP NC` (`post.nu:115-199`).

### 8.3 Concrete zsh byte stream for one prompt cycle

```
\e]697;OSCUnlock=3f2a…\a\e]697;Dir=/home/u/src\a\e]697;Shell=zsh\a\e]697;ShellPath=/bin/zsh\a\e]697;PID=4242\a
\e]697;ExitCode=0\a\e]697;TTY=/dev/ttys003\a\e]697;Log=\a(ignored)\e]697;ZshAutosuggestionColor=fg=8\a…\e]697;User=u\a
\e]697;StartPrompt\a~/src % \e]697;EndPrompt\a\e]697;NewCmd=3f2a…\a        <- cmd_cursor = cursor here
git st                                                                     <- echoed by ZLE, unflagged cells
<Enter> → \r\n \e]697;OSCLock=3f2a…\a\e]697;PreExec\a  … command output … (back to precmd)
```

---

## 9. Semantic cell marking

1. **Prompt cells.** Between `StartPrompt` and `EndPrompt` the cursor template carries `IN_PROMPT`, so every char written gets it. This covers the left prompt, right prompt, PS2 continuation prompt and the vi-mode indicator. `EndPrompt` clears the template flag. Cells cleared later (EL/ED/ECH) become default cells without flags (`term/mod.rs:1319-1393`).

2. **Suggestion (ghost text) cells.** After **every** SGR change of fg (`Foreground`), bg (`Background`) or `Reset` (SGR 0), the term recomputes `template.fig_flags.IN_SUGGESTION` (`term/mod.rs:1440-1509`):
   - Pick the suggestion color by `context.shell`:
     - `"fish"` → `fish_suggestion_color`
     - `"zsh"` → `fig_autosuggestion_color` if set, else `zsh_autosuggestion_color`
     - `"nu"` → `nu_hint_color`
     - anything else → none (so never IN_SUGGESTION)
   - `IN_SUGGESTION = fg_matches && bg_matches`, where a missing fg or bg in the spec counts as a match. A spec with neither therefore marks everything **(inferred)**.
   - `color_match(cellColor, vtermColor)`:
     - `Named(n)` vs `Indexed{idx}`: `(n as usize % 256) == idx`. NamedColor enum values 0..15 are the ANSI colors, so `fg=8` matches `Named(BrightBlack)`.
     - `Indexed(i)` vs `Indexed{idx}`: `i == idx`.
     - `Spec(rgb)` vs `Rgb{r,g,b}`: exact equality.
     - Otherwise false.
   - Bold/dim/italic SGR do **not** recompute the flag (only fg/bg/reset do).
   - The color spec parsers live in the external crate `shell-color = "1.0.0"` (workspace `Cargo.toml:133`). They parse zsh `region_highlight` syntax like `fg=8`, `fg=#aaaaaa,bg=…` and fish color syntax, using `ColorSupport` detected at startup (`term/mod.rs:321`) **(inferred: not available locally)**.

This is how zsh-autosuggestions, fish autosuggestions, nu hints and Q's own inline ghost text are kept **out** of the edit buffer.

---

## 10. Edit-buffer extraction (exact algorithm)

### 10.1 `get_current_buffer()`

From `term/mod.rs:755-790`.

```
if cmd_cursor is None → None
if topmost_line() (= -history_size, history ≤ 1 line) > cmd_cursor.line → None   // scrolled away
start = Point(cmd_cursor.line, 0); end = Point(bottommost_line = screen_lines-1, last_column)
if !(start < end) → None
tb = get_text_region(Rect{start,end}, start_col_offset = cmd_cursor.column)?
if tb.cursor_idx is Some(ci):
    tb.buffer = tb.buffer.trim_end()                   // drop trailing blanks / rest of screen
    if tb.buffer.len() < ci: pad with ' ' up to ci     // keep typed trailing spaces before cursor
return tb   // { buffer: String, cursor_idx: Option<usize> }
```

The region is **from the command start to the bottom-right corner of the screen**, not to the end of the command.

### 10.2 `get_text_region(rect, offset)`

From `term/mod.rs:681-753`.

```
start = rect.start with column += offset; end = rect.end; if start > end → None
padding = 0; cursor = grid.cursor.point; last_char_width = 0
buffer = ""; cursor_idx = (cursor == start) ? Some(0) : None
for cell in grid.iter_from_to_post_increment(start, end):
    // iterates cells [start, end) — the end cell itself is NOT visited;
    // cell.point is the position AFTER the cell (post-increment: col+1, or (line+1, 0) after last col)
    if last_char_width > 0: last_char_width -= 1; continue          // skip wide-char spacer
    if cell.c == '\0' || IN_PROMPT || IN_SUGGESTION:
        padding += 1
    else:
        push ' ' × padding; padding = 0                             // interior gaps become spaces
        push cell.c, then its zero-width chars
        last_char_width = unicode_width(cell.c).unwrap_or(1) - 1   // 1 for CJK/emoji
    if cell.point.column == rect.end.column: padding = 0            // drop pending padding at row end
    if cell.point.line == cursor.line && cursor.column in cell.point.column ..= cell.point.column + last_char_width:
        cursor_idx = Some(buffer.len())                             // BYTE offset (UTF-8)
        push ' ' × padding; padding = 0
return { buffer, cursor_idx }
```

Consequences a re-implementation must reproduce, or deliberately fix:

- **cursor_idx is a UTF-8 byte offset** (`String::len`), not a char or column index. It is sent as `EditBufferHook.cursor`.
- The cursor match uses the **post-increment** point. "Cursor is right after this cell" gives the index after the char. For a wide char at column c, a cursor at c+1 (on the spacer) or c+2 both mean "after".
- Ordinary blank cells are `' '` (not `'\0'`), so they are **pushed literally**. In a row that ends early, the rest of the row up to the right edge becomes spaces, and only the final `trim_end` removes trailing ones. Hence:
  - **Soft-wrapped long commands** come out contiguous, which is correct.
  - **Multi-line commands** (zsh PS2 continuation, `\` + Enter, multi-line paste): the rest of line 1 is spaces, the PS2 prompt cells are padding, line 2's text follows. The result is `"line1" + N spaces + "line2"`. **No `\n` is ever inserted.**
  - **Anything printed below the command line** is part of the region: zsh's native completion listing, leftover text, a menu drawn by another tool. It ends up in the buffer unless it is IN_PROMPT/IN_SUGGESTION. Shells usually clear below with `ESC[J`.
- **Right prompt** (RPROMPT, fish_right_prompt): IN_PROMPT → padding → dropped (row-end reset or trailing). The spaces between the text and the RPROMPT are literal `' '`, removed by `trim_end` when the command fits on one line.
- **Left multi-line prompts:** `NewCmd` comes after the last prompt line, so earlier lines are above `cmd_cursor` and outside the region. Nothing special is needed.
- **Inline ghost text / autosuggestions:** IN_SUGGESTION → padding → not in the buffer. If the user's cursor sits **before** the ghost text, the cursor match runs on the last real char, so cursor_idx is the end of the real text.
- `LEADING_WIDE_CHAR_SPACER` cells (a wide char that didn't fit at the end of a row) are `' '` cells and **are pushed as a space**. That puts a spurious space in the buffer (quirk).
- Tabs: a `'\t'` cell is pushed as `\t`; the following blank cells are pushed as spaces (quirk).
- **Vi mode:** no special handling. The `TermMode::VI` flag in the fork is alacritty's own UI vi mode, unrelated to the shell. The edit buffer is whatever the shell drew. Mode indicators drawn inside the wrapped prompt (fish_mode_prompt, nu vi indicators) are IN_PROMPT. A zsh `RPROMPT` vi indicator is IN_PROMPT too. A vi indicator drawn with raw escapes outside `%{…%}` would leak into the buffer **(inferred)**.
- Scrollback is 1 line. If the command start scrolls more than one line above the viewport, there is **no buffer** (`None`).
- After `NewCmd`, if the shell moves the cursor **above** `cmd_cursor` (unusual), `cursor_idx` stays `None`, and nothing is sent (§10.6).

### 10.3 Emoji / ambiguous-width workaround

`Q_PROMPT_OFFSET_WORKAROUND=N` shifts the recorded command start N columns left. It is for prompts whose glyph widths differ between the real terminal and `unicode-width` (`term/mod.rs:835-848`).

### 10.4 zsh-autosuggestions

Users of the real zsh-autosuggestions plugin are handled two ways:
- by the `ZshAutosuggestionColor` OSC above
- by **not loading** Q's own inline plugin: `guard_start.zsh` does `if command -v _zsh_autosuggest_accept …; then export Q_USING_ZSH_AUTOSUGGESTIONS=1; else <plugin>…`.

### 10.5 Insertion lock (prevents flicker / stale buffers after an insert)

- When figterm processes an `InsertText` with `insertion = Some(text)` and a buffer + cursor exist, it sets `INSERTION_LOCKED_AT = now` and `EXPECTED_BUFFER = current_buffer + text`. The raw text, including any `\b` or `ESC[D`, is concatenated verbatim (`message.rs:172-191`).
- `can_send_edit_buffer` (`main.rs:300-346`) returns `shell_enabled && !insertion_locked && !preexec`:
  - `shell_enabled`: `context.shell` with a `" (figterm)"` suffix stripped ∈ `["bash","zsh","fish","nu","dash"] ∪ user.enabled-shells`.
  - The lock is **released** when either:
    - more than **16 ms** have passed since it was set, or
    - `get_current_buffer()` is `None`, or
    - the buffer **equals** `EXPECTED_BUFFER` exactly.

    In practice only pure appends ever match, so most locks expire on the timer.
- While locked, PTY reads don't send the buffer. The 16 ms ticker re-checks and sends once unlocked.

### 10.6 Sending

From `main.rs:356-389`. If `Q_DISABLE_AUTOCOMPLETE` is empty/unset and a buffer exists **with** a `cursor_idx` that fits in i64:

`Hostbound::Request(EditBufferHook{ context: shell_state_to_context(), text: buffer, cursor: cursor_idx as i64, histno: 0, terminal_cursor_coordinates: (Windows only) })` → remote socket.

It is sent after **every** PTY read chunk, with no deduplication. If there is no buffer the function errors and the caller just logs a warning.

`shell_state_to_context()` (`main.rs:178-209`) → `fig_common.ShellContext`:
- `pid`, `ttys` = tty, `process_name` = shell, `shell_path`, `wsl_distro`, `current_working_directory`
- `session_id` (ours)
- `terminal` = `Terminal::parent_terminal()` string (§18)
- `hostname` = `"{username}@{sysinfo host_name}"` (only if a username is known)
- `environment_variables` (from the last `pre-cmd`)
- `qterm_version`, `preexec`, `osc_lock`
- `alias` (raw `alias` output from `pre-cmd`)

---

## 11. Lifecycle events (`EventHandler`)

From `event_handler.rs:54-140`.

| Event | Fired by | Actions |
|---|---|---|
| `Prompt` | each accepted `NewCmd` | Build a PromptHook. `INSERT_ON_NEW_CMD.take()`. If `context.cwd` exists, `set_current_dir(cwd)`. If there was a pending insert-on-new-cmd, send `MainLoopEvent::Insert{text, unlock: false, bracketed, execute}`. Send `SetImmediateMode(false)`. Send PromptHook to the app. If CSI-u is enabled, send `SetCsiU`. |
| `PreExec` | first `PreExec` OSC after a prompt | `UnlockInterception`, `SetImmediateMode(true)`, PreExecHook (`command: None`) to the app, `UnsetCsiU` if enabled |
| `CommandInfo(&info)` | the `NewCmd` after a command (when `command_info` was pending) | `tokio::spawn(inline::on_prompt())`, which clears the inline completion cache and flushes telemetry. `PostExecHook{context, command, exit_code}` to the app. `HistoryCommand::Insert(info)` |
| `ShellChanged` | `Shell=` changed | nothing |

All sends use `flume::Sender::send`, which **blocks** on the bounded(16) main-loop channel. These callbacks run **inside** `processor.advance` on the main loop, so more than 16 queued events in one read chunk could deadlock **(inferred risk)**. Use an unbounded channel or a local queue.

The history-relevant ordering for one command: `PreExec` creates command_info → the command runs → the precmd `ExitCode=` fills exit_code and end_time → `NewCmd` fires Prompt, then CommandInfo. An `ExitCode=` arriving while `osc_lock` is true is dropped, but precmd sends `OSCUnlock` first.

---

## 12. Key interception (`interceptor.rs`)

### 12.1 State

```rust
struct KeyInterceptor { intercept_global: bool, intercept: bool, window_visible: bool,
                        mappings: DashMap<KeyEvent{key, modifiers}, String /*action id*/> }
```

Bindings are loaded at startup from the hard-coded `fig_settings/src/actions.json` (`interceptor.rs:107-115`, `keybindings.rs:44-63`). The app can extend or replace them later.

Default bindings (`actions.json`):

| Action id | Default bindings | Availability |
|---|---|---|
| insertSelected | `enter` | WHEN_FOCUSED |
| insertCommonPrefix | `tab` | WHEN_FOCUSED |
| insertCommonPrefixOrNavigateDown, insertCommonPrefixOrInsertSelected, insertSelectedAndExecute, execute | (none) | WHEN_FOCUSED |
| hideAutocomplete | `esc` | ALWAYS |
| showAutocomplete, toggleAutocomplete | (none) | ALWAYS |
| navigateUp | `shift+tab`, `up`, `control+k`, `control+p` | WHEN_FOCUSED |
| navigateDown | `down`, `control+j`, `control+n` | WHEN_FOCUSED |
| selectSuggestion1..10 | `control+1` … `control+9`, `control+0` | WHEN_FOCUSED |
| hideDescription, showDescription | (none) | |
| toggleDescription | `command+i` | |
| toggleHistoryMode | `control+r` | |
| toggleFuzzySearch | (none) | |
| increaseSize / decreaseSize | `command+=` / `command+-` | |

Plus the pseudo-action **`ignore`**: a binding to `ignore` means "don't intercept".

### 12.2 Binding text grammar (`key_from_text`)

From `interceptor.rs:26-87`.

- Split on `+` repeatedly. A lone `+` key is handled by the `("","")` stop case.
- Modifiers:
  - `ctrl` | `control` → CTRL
  - `shift` → SHIFT
  - `alt` | `option` → ALT
  - `meta` | `command` → META
  - anything else → none
- Key names:
  - `backspace`, `enter`
  - `arrowleft`|`left`, `arrowright`|`right`, `arrowup`|`up`, `arrowdown`|`down`
  - `home`, `end`, `pageup`, `pagedown`, `tab`, `delete`, `insert`, `esc`
  - `f<N>` → Function(N)
  - otherwise exactly one char.
- `shift+<lowercase letter>` becomes the uppercase char with SHIFT removed.

`insert_binding` (`:146-187`) also inserts:
- **Application-mode arrows** for `up/down/left/right`, since a shell in DECCKM sends `ESC O A`.
- **The other letter case** for any CTRL- or ALT-modified ASCII letter. The parser reports `0x0b` as `Char('K')`+CTRL, so `control+k` must match both `k` and `K`.

Unreachable in practice with legacy encoding:
- `control+j`: `0x0a` is parsed as Enter, so it would trigger **insertSelected** instead.
- `control+0..9`: needs CSI-u.
- `command+…`: terminals don't send Cmd.

### 12.3 Decision (`intercept_key`)

From `interceptor.rs:195-226`.

```
match (intercept_global, intercept):
  (true, false):  if key == Tab && autocomplete.onlyShowOnTab → Some("showAutocomplete")   // Tab swallowed!
                  else mappings[key] is "toggleAutocomplete"|"showAutocomplete" → Some(it) ; else None
  (_, true):      if window_visible: mappings[key] (unless "ignore") → Some(action) ; else None
  (false,false):  None
```

Only called when **not** `preexec` (at a prompt). `Some(action)` means the key is **not** forwarded to the shell; `InterceptedKeyHook{context, action, key: raw bytes as UTF-8 string}` goes to the app (`main.rs:768-787`).

### 12.4 Who flips the flags

- App → figterm `InterceptRequest` (§14.3):
  - `SetFigjsIntercepts{intercept_bound_keystrokes, intercept_global_keystrokes, actions[], override_actions}` sets both flags. With `override_actions` it first clears all mappings, then adds `actions` (`message.rs:208-227`).
  - The desktop sends the app's action list plus the user's `autocomplete.keybindings.*` settings with `override_actions = true` to the **focused** session, and `intercept=false, actions=[], override=false` to every other session (`fig_desktop/src/request/properties.rs:20-90`).
  - `SetFigjsVisible{visible}` → `window_visible`.
- `reset()` (both intercept flags false; `window_visible` unchanged) happens on:
  - an intercepted **Escape**
  - Ctrl+C/Ctrl+D (see the §6.2 caveat)
  - `Event::PreExec`
  - `MainLoopEvent::Insert{unlock: true}`
  - **IPC send failure** to the app (`ipc.rs:309-334`): so keys are never swallowed when the UI is dead.

For tabsmith, the overlay lives in-process, so "intercept" is simply "menu is open and the key is bound". Keep the same safety resets (Esc, Ctrl-C, preexec, any error).

---

## 13. "Insert text into the shell": exact algorithm

### 13.1 Message

From `proto/figterm.proto:66-83`.

```
InsertTextRequest { optional string insertion=1; optional uint64 deletion=2; optional int64 offset=3;
                    optional bool immediate=4; optional string insertion_buffer=5; optional bool insert_during_command=6; }
```

### 13.2 figterm handling

From `message.rs:162-207` and `fig_proto/src/figterm.rs:3-31`.

```
if term.preexec && !request.insert_during_command.unwrap_or(false): return            // ignored while a command runs
out = ""
if let Some((buffer, Some(cursor))) = term.get_current_buffer():
    if request.insertion is Some(text):
        INSERTION_LOCKED_AT = now; EXPECTED_BUFFER = buffer + text                  // §10.5
    if request.insertion_buffer is Some(ib) and buffer != ib:                         // reconcile UI's stale view
        if buffer.starts_with(ib):  out += "\x08" × (buffer.len() - ib.len())        // user typed more → erase extra (BYTES!)
        elif ib.starts_with(buffer): out += ib[buffer.len()..]                        // shell shows less → retype missing tail
        // else: no reconciliation
out += to_term_string(request):
    if offset > 0:  "\x1b[C" × offset        // cursor right, CSI C (normal-mode arrow)
    if offset < 0:  "\x1b[D" × |offset|      // cursor left
    "\x08" × deletion.unwrap_or(0)           // BS (Ctrl-H) = backward-delete-char in zle/readline/fish
    insertion.unwrap_or("")
    if immediate == Some(true): "\r"         // execute
pty_master.write(out)                         // a single write() call, not write_all
```

Test vector: `{deletion: 3, insertion: "hello", offset: 2}` → `"\x1b[C\x1b[C\x08\x08\x08hello"` (`figterm.rs:37-49`).

Notes:
- **Order is offset → deletion → insertion → `\r`**. The proto comment says "move cursor n characters … accounting for insertion & deletion"; the code does not follow it. Copy the code if you want byte-compatible behavior.
- Reconciliation backspace counts are **byte** differences, so they are wrong for multibyte text (quirk).
- `pty_master.write` may short-write. `write_all` exists but isn't used here.

### 13.3 What the autocomplete UI actually sends

From `packages/autocomplete/src/state/insertion.ts:25-90` and `fig_desktop/src/request/figterm.rs:13-47`. The UI usually sends **only `insertion` + `insertion_buffer`**, with the deletion and cursor movement **embedded in the insertion string**:

```
valToInsert = full text for suggestion (escaped: spaces → "\ ", special chars → single-quoted,
              folders keep trailing "/"; "{cursor}" marker allowed)
moveCursor  = if "{cursor}" at index i: remove marker; "\x1b[D" × (len(valToInsert) - i)
numCharsToDelete = len(queryTerm with first whitespace→"\ ")   (0 for full auto-execute/special items)
   +1 if shortcut and searchTerm starts with "?"; +2 if rootCommand == "?"
if valToInsert.startsWith(queryTerm) && numCharsToDelete >= len(queryTerm):
     numCharsToDelete -= len(queryTerm); valToInsert = valToInsert[len(queryTerm):]    // only append the rest
final = "\b" × numCharsToDelete + valToInsert + moveCursor
Shell.insert(final, {insertionBuffer: <buffer the UI computed suggestions from>}, sessionId)
```

Extra rules on the full insertion text (`makeInsertTextForItem`, `:193-224`):
- `execute` (and not auto-execute) appends `\n`.
- Trailing newlines collapse to one.
- If the text doesn't end in `\n` and `item.shouldAddSpace` and setting `insertSpaceAutomatically` (default true), append `" "`.
- The `\n` is sent raw. In a raw-mode shell, LF is the Enter key: zle and readline accept `^J` as accept-line.

Example: buffer `git st`, the user selects `status`. queryTerm `st`, valToInsert `status ` → strip the prefix → final `"atus "`, `insertion_buffer = "git st"`.
- figterm sees buffer `git st`: equal, nothing to reconcile, writes `atus `.
- If the shell already showed `git sta`, figterm prepends one `\x08` first.

### 13.4 Related insert paths

- `InsertOnNewCmdRequest{text, execute, bracketed}` stores a pending insert, delivered at the next `Prompt` event as `MainLoopEvent::Insert{unlock: false}` (`message.rs:265-268`, `event_handler.rs:63-80`). The bracketed/execute semantics are in §5 (bracketed paste with ESC stripped, or newline→CR; then `\r`).
- Remote `InsertText` from the app goes through the same `process_figterm_request` (`message.rs:399-413`).

---

## 14. IPC

### 14.1 Framing (both sockets)

`ESC '@'` (2 bytes) + 8-byte type tag (`fig-pbuf` | `fig-json` | `fig-mpak`) + **u64 big-endian body length** + body. figterm always sends `fig-pbuf` (prost protobuf) and can decode all three (`fig_proto/src/lib.rs:62-166,168-221,252-261`).

### 14.2 Per-session local socket (`spawn_figterm_ipc`)

From `ipc.rs:153-238`.

- Path `sockets_dir/t/<session>.sock`. Parent dir created and **chmod 0700**. Any stale file is removed, then bound.
- Unlimited connections. Each connection reads a stream of `FigtermRequestMessage` and gets `FigtermResponseMessage`s written back on the same connection, through a per-connection response channel.

Requests (`proto/figterm.proto:7-21`, handled in `message.rs:301-349` → `process_figterm_request` `:154-297`):

| oneof field | Handling |
|---|---|
| `intercept=1` (InterceptRequest) | §12.4 |
| `insert_text=2` | §13 |
| `set_buffer=3` | error: "SetBuffer is not supported in figterm" |
| `diagnostics=4` | → `DiagnosticsResponse{shell_context, fish_suggestion_style, zsh_autosuggestion_style (TermStyle fg/bg as TermColor{Rgb{r,b,g} or indexed}), edit_buffer, cursor_position}` (used by `q doctor`) |
| `insert_on_new_cmd=5` | §13.4 |
| `update_shell_context=6` | `{update_environment_variables, environment_variables[], update_alias, alias}` → stored globally, included in every ShellContext. Sent by **`q _ pre-cmd --alias "$(\alias)"`** on each prompt with `std::env::vars()` of that subprocess, which equals the shell's exported env (`q_cli/src/cli/internal/mod.rs:873-910`) |
| `notify_ssh_session_started=7` | → `MainLoopEvent::PromptSSH` → no-op |
| `inline_shell_completion=8` | spawned `inline::handle_request` → `InlineShellCompletionResponse{insert_text}` (§15) |
| `inline_shell_completion_accept=9` | spawned `inline::handle_accept` (telemetry only) |
| `telemtety=10` (sic) | JSON telemetry passthrough (drop) |
| `inline_shell_completion_set_enabled=11` | spawned `handle_set_enabled` → per-process `INLINE_ENABLED` |

### 14.3 Remote socket to the app (`spawn_remote_ipc`)

From `ipc.rs:241-367`.

- Every **5 s** (interval, MissedTickBehavior::Skip) until stopped:
  1. Connect to `remote.sock` with a 5 s timeout. In WSL it instead spawns `fig.exe _ stream-from-socket` and talks over its stdio.
  2. Send `Hostbound::Handshake{id: session_id, secret: random hex, parent_id: $Q_PARENT}`.
  3. Wait for `Clientbound::HandshakeResponse{success}`. On failure, retry.
  4. On success: **drop any queued outgoing messages** (`outgoing_rx.drain()`), then run the writer and reader tasks until either ends.
- A writer error or flush error sends `Insert{empty, unlock: true}`, which resets interception.

Hostbound requests figterm sends (`proto/remote.proto:78-88`, `fig_proto/src/remote_hooks.rs`):

| Request | Contents |
|---|---|
| `edit_buffer=101` | EditBufferHook{context, text, cursor:int64, histno:int64 (=0), terminal_cursor_coordinates?{x,y,xpixel,ypixel}} |
| `prompt=102` | PromptHook{context} |
| `pre_exec=103` | PreExecHook{context, command: None} |
| `post_exec=108` | PostExecHook{context, command, exit_code} |
| `intercepted_key=104` | InterceptedKeyHook{context, key, action} |

Clientbound requests figterm handles (`message.rs:375-532`):
- `intercept=100`, `insert_text=101`, `diagnostics=103`, `insert_on_new_cmd=107`: same as local. Responses are Hostbound::Response{nonce, …}.
- `run_process=104`: run `executable` with `arguments` in `working_directory`. The cwd falls back to the shell cwd, then figterm's cwd, then `/` or `C:\`. The executable is resolved through the shell's PATH (`which_in`).
  - Env: the **shell's env** from the last pre-cmd if known (`env_clear` + set). Remove `LS_COLORS`, `CLICOLOR_FORCE`, `CLICOLOR`, `COLORTERM`. Set `PROCESS_LAUNCHED_BY_Q=1`, `HISTFILE=""`, `HISTCONTROL=ignoreboth`, `TERM=xterm-256color`, `NO_COLOR=1`. Then the request's own env.
  - `setsid()` in pre_exec (detach from the tty). `kill_on_drop`. Timeout = request timeout or 60 s.
  - Response: `RunProcessResponse{stdout, stderr (lossy utf8), exit_code (default 0)}`, or `Error("failed running executable …" / "timed out after Nms …")`.
  - **This is how spec generators/scripts execute in the user's environment.** tabsmith's JS spec runtime will need the same helper (`message.rs:61-151,459-515`).
- `read_file=105` / `set_buffer=102`: unhandled (warn).
- `Ping` → `Pong`.

---

## 15. Inline ghost text (`inline/`)

### 15.1 Who draws it

**Not figterm.** The ghost text is drawn by **zsh itself**, through a vendored fork of *zsh-autosuggestions*:
- `fig_integrations/src/shell/inline_shell_completion/*.zsh`, concatenated by build.rs into `inline_shell_completion.zsh`.
- Sourced by `q init zsh post` only when the rc file is not zprofile/profile/bash_profile, `dotfiles.enabled`, the shell is **zsh**, and `inline.enabled` is true (`q_cli/src/cli/init.rs:155-174`).
- Skipped entirely if real zsh-autosuggestions is loaded (`guard_start.zsh`).

How the plugin works:
- It sets `POSTDISPLAY` (text shown after the cursor) and adds a `region_highlight` entry `"$#BUFFER $(($#BUFFER+$#POSTDISPLAY)) $Q_AUTOSUGGEST_HIGHLIGHT_STYLE"`, default **`fg=8`** (`config.zsh:9-10`, `highlight.zsh`).
- figterm learns that style through `FigAutosuggestionColor=` and marks those cells IN_SUGGESTION, so they never enter the edit buffer.
- Strategy (`strategies/inline.zsh`): `suggestion="$(q _ inline-shell-completion --buffer "$BUFFER")"`.
- Async (zsh ≥ 5.0.8): fork the process substitution, `zle -F` on its fd, and on a new keystroke kill the previous child's process group (`async.zsh`). In `modify`, if `$PENDING`/`KEYS_QUEUED_COUNT > 0`, don't fetch. If the user typed a prefix of the current POSTDISPLAY, just shrink it (`widgets.zsh:39-83`).
- `_suggest`: `POSTDISPLAY="${suggestion#$BUFFER}"` (`widgets.zsh:97-107`).
- Accept widgets (`forward-char`, `end-of-line`, `vi-forward-char`, `vi-end-of-line`, `vi-add-eol`) accept only when the cursor is at the end. They append POSTDISPLAY to BUFFER and fire `q _ inline-shell-completion-accept --buffer … --suggestion …` in the background. Partial accept: `forward-word` etc. Clear widgets: history navigation, `accept-line`… (`config.zsh:23-74`).

For tabsmith: since we own the PTY and render the overlay ourselves, we can draw ghost text **ourselves** (inshellisense-style) for all shells. The zsh-plugin approach is a zsh-only option.

### 15.2 Request flow

`q _ inline-shell-completion --buffer B` (`q_cli/src/cli/internal/inline_shell_completion.rs:37-79`):
1. Connect to `$TMPDIR/cwrun/t/$QTERM_SESSION_ID.sock`.
2. Send `InlineShellCompletionRequest{buffer: B}` and wait up to **5 s** for the response.
3. On `insert_text: Some(t)`, print `B + t + "\n"` and exit 0.
4. On `None` or an error, exit 1 (no suggestion).

`handle_request` (`inline/mod.rs:164-365`):
1. If `!INLINE_ENABLED` (per process, toggled by `q inline enable|disable` over the **current** session's socket only, `inline.rs:152-165`), return **without responding**; the client times out after 5 s.
2. `buffer = request.buffer.trim_start()`.
3. **Cache** (unless `Q_INLINE_SHELL_COMPLETION_CACHE_DISABLE`): `cache.get_insert_text(buffer)`. On a hit, respond immediately with `insert_text.strip_prefix(buffer)` and return.
4. **Debounce:**
   - `now = SystemTime::now(); LAST_RECEIVED = now`.
   - Loop up to 3 times:
     - `sleep(DEBOUNCE = 300 ms or $Q_INLINE_SHELL_COMPLETION_DEBOUNCE_MS)`.
     - If `LAST_RECEIVED != now` (a newer request arrived), respond `insert_text: None` and return.
     - Else set `LAST_RECEIVED = SystemTime::now()`.

     **Bug:** this rewrite means any `continue` (the throttling retry) fails the check on the next iteration, so the request is effectively attempted once.
5. History: `HistoryCommand::Query{limit: HISTORY_COUNT=49}` → rows with `exit_code IS NOT NULL ORDER BY id DESC LIMIT 49` (§16).
6. `prompt(history, buffer)` (`:381-403`): for `i` from `len-1` down to 0, take the `i+1` most recent commands, reverse them to chronological order, append `buffer`, and number them like bash `history`: `format!("{:>5}  {cmd}", n)` joined by `\n`. Return the first one under **10240 bytes**, else `None`, in which case it returns **without responding**.

   Example: `"    1  echo hello\n    2  echo world\n    3  echo "`.
7. Call the model (CodeWhisperer `generate_recommendations`, filename `history.sh`, language Shell, max_results 1). **tabsmith replaces this with a local engine or an OpenAI/Anthropic call.** On a throttling error: sleep `1s - debounce` and `continue`.
8. `clean_completion` (`:405-421`): take the first line; remove regex `"{HISTORY_COUNT+1}\s+.*"` and `"{HISTORY_COUNT+2}\s+.*"` (i.e. `50\s+.*` and `51\s+.*`: the model running on into the next numbered history line); `trim_end`. The regexes are **unanchored**, so `sleep 50 && ls` would be cut to `sleep ` (quirk).
9. For each completion: `full = buffer + completion`.
   - Non-first valid non-empty completions are cached with priority 1.0.
   - The first one, if valid and non-empty, is cached with priority 0.0.
   - Respond with the first one if `validate(full)`, else `None`.
10. Responses go to the same connection.

`validate(command)` (`inline/validate.rs:3-59`), on `command.trim()`:
- Reject if it contains `"XXX"` (server-side PII redaction marker).
- If `shlex::split` gives exactly `["cd", arg]`: `shellexpand::full_with_context(arg, home, env)`.
  - If expansion fails (unknown var etc.), accept.
  - Else, if the path is absolute and not a dir, reject.
  - Else `canonicalize(cwd.join(path))` must succeed and be a dir. `cwd` is figterm's current dir, which follows the shell via `Dir=`.
- Everything else is valid.

`CompletionCache` (`completion_cache.rs`): a radix trie `full_text → priority (f64, lower is better)`.
- `get_insert_text(buffer)`: among cached keys **having `buffer` as a prefix**, return the key with the minimum priority (`get_raw_descendant` + `min_by(total_cmp)`).
- Cleared on every `CommandInfo` event (after each executed command).
- Test semantics: `{"echo hello, world":0.5, "echo hello there":1.0, "ls":0.5, "ls -l":1.0, "ls -al":1.0, "cat -n":0.5, "cat":1.0}` gives `"e"`→`"echo hello, world"`, `"echo hello "`→`"echo hello there"`, `"ls -a"`→`"ls -al"`, `"cat -"`→`"cat -n"`, `"other"`→None.

Accept and telemetry: `handle_accept` only marks queued telemetry items as accepted. **Drop it** (out of scope), or keep a local acceptance counter.

---

## 16. Command history (`history.rs` → `fig_settings::history`)

- A spawned task owns a flume bounded(64) channel `HistoryCommand::{Insert(CommandInfo), Query(params, reply)}` (`figterm/src/history.rs:30-85`).
- Insert maps the alacritty `CommandInfo` and builds `hostname = "{username}@{host}"`, then calls `insert_command_history(&info, legacy = true)`.
- Only commands that are **non-empty** after the trim at PreExec are stored (`fig_settings/src/history.rs:87-178`).

SQLite table `history` in `data.sqlite3`. The schema after migrations 001+002+003 is (`fig_settings/src/sqlite/migrations/001..003`):

```sql
CREATE TABLE history (id INTEGER PRIMARY KEY, command TEXT, shell TEXT, pid INTEGER, session_id TEXT, cwd TEXT,
                      start_time INTEGER /* unix secs, was `time` */, hostname TEXT, exit_code INTEGER,
                      end_time INTEGER /* unix secs */, duration INTEGER /* ms = end - start */);
-- (in_ssh, in_docker columns were dropped in 002)
```

Insert: `INSERT INTO history (command, shell, pid, session_id, cwd, start_time, end_time, duration, hostname, exit_code) VALUES (?…)`.

What each field means:
- `shell`: the `Shell=` name (e.g. `zsh`).
- `pid`: the shell pid from `PID=`.
- `session_id`: the **qterm session id**.
- `cwd`: figterm's `current_dir` at PreExec (it follows `Dir=`).
- `start_time`: PreExec time.
- `end_time`: when the `ExitCode=` OSC arrived.
- `exit_code`: from `ExitCode=`. A row stays NULL if `ExitCode` was never received. The inline query filters those out.

Legacy append to `fig_data_dir/history` (0600). Each entry is YAML-ish, with `escape_string` escaping `\ " \n \t \r \b \f`:

```
\n- command: <escaped>\n  exit_code: <i32|0>\n  shell: <…>\n  session_id: <…>\n  cwd: <…>\n  time: <start secs|0>
```

Query: `SELECT id, command, shell, pid, session_id, cwd, start_time, duration, hostname, exit_code FROM history WHERE exit_code NOT NULL ORDER BY id DESC LIMIT ? OFFSET 0` (`history.rs:60-79`, `fig_settings/src/history.rs:194-228`). `end_time` is reconstructed as `start + duration`.

---

## 17. `update.rs`, `cleanup.rs`, `logger.rs`

- **update.rs** (`:46-90`), called once at startup before raw mode:
  - Return early unless **Linux**, not CloudShell (`AWS_EXECUTION_ENV=cloudshell`), not installed via Toolbox, and `!get_bool_or("app.disableAutoupdates", true)` is false. The double negation is a bug: with the default value it does **not** return early, and setting `app.disableAutoupdates=false` *disables* the notice.
  - Spawn `fig_install::check_for_updates` and store the new version in state key `update.new-version-available`.
  - Synchronously read that key from the previous run. If it is > the current version, print `\nA new version of q is available: X\n` (plus `Run q update …` unless it's a Linux Full variant) to stdout before the shell starts.
  - Drop all of this for tabsmith, or replace it with an opt-in check.
- **cleanup.rs**: Linux only, a no-op. The old `Q_PARENT` socket cleanup is commented out.
- **logger.rs**: `stdio_debug_log(s)` does `println!` only when the max log level is ≥ DEBUG. It is used for early messages: pty name, parent/child pids.

---

## 18. Terminal detection (`fig_util/src/terminal.rs`, `fig_os_shim/src/env.rs`)

- `MACOS_TERMINALS`: Alacritty, iTerm, Kitty, Tabby, TerminalApp, VSCodeInsiders, VSCode, VSCodium, WezTerm, Zed, Cursor, CursorNightly, Rio, Windsurf, WindsurfNext, Ghostty, Positron, Trae (`:12-31`).
- `LINUX_TERMINALS`: Alacritty, Kitty, GnomeConsole, GnomeTerminal, Guake, Hyper, Konsole, XfceTerminal, WezTerm, Tilix, Terminator, VSCode, VSCodeInsiders, VSCodium, IntelliJ, Positron (`:34-51`).
- `SPECIAL_TERMINALS` (launch inside these too): Ssh (`sshd`), Tmux (`tmux`, `tmux: server`), Nvim, Vim, Zellij (`:54-60,599-605`).
- `parent_terminal(ctx)` (`:226-260`):
  1. macOS: `__CFBundleIdentifier` → `from_bundle_id`. The bundle ids include `com.googlecode.iterm2`, `com.apple.Terminal`, `net.kovidgoyal.kitty`, `com.microsoft.VSCode`, `com.mitchellh.ghostty`, `com.jetbrains.*`/`com.google.*` → IntelliJ… (`:419-450`).
  2. `TERM_PROGRAM`: `iTerm.app`, `Apple_Terminal`, `Hyper`, `vscode` (Insiders if `TERM_PROGRAM_VERSION` contains "insiders"), `Tabby`, `Nova`, `WezTerm`, `guake`, `ghostty`.
  3. Walk up to **5** ancestor processes, matching the exe file name against `executable_names()` (`code`, `code-insiders`, `alacritty`, `kitty`, `kgx`, `gnome-terminal-server`, `konsole`, `tilix`, `xfce4-terminal`, `wezterm`/`wezterm-gui`, `hyper`, `tabby`, `terminator`, `zed`, `Cursor`/`cursor`, `rio`, `windsurf`, `guake`, `ghostty`, `positron`, `trae`…) or `cmdline` (`try_from_cmdline`, which also checks the 2nd argv for python-launched Terminator/Guake) (`:264-333,495-530`).
- `internal_id()`: kebab ids like `iterm`, `terminal`, `vscode`, `gnome-terminal`… (`:345-386`). Used for the ShellContext `terminal` string (Display form, e.g. "iTerm 2").
- Env helpers:
  - `in_ssh`: `SSH_CLIENT | SSH_CONNECTION | SSH_TTY`
  - `in_ci`: `CI | Q_CI`
  - `in_codespaces`: `CODESPACES | Q_CODESPACES`
  - `in_cloudshell`: `AWS_EXECUTION_ENV=cloudshell`
  - `in_wsl`: `/proc/sys/kernel/osrelease` contains microsoft/wsl
  - `is_remote`: ssh | cloudshell | wsl | `Q_FAKE_IS_REMOTE`

  (`fig_os_shim/src/env.rs:145-172`, `fig_util/src/system_info/mod.rs:240-270`)

---

## 19. Quirks and bugs found (decide deliberately; don't copy blindly)

1. `qterm.enabled=false` makes qterm exit without spawning a shell, which kills the session (`main.rs:982-986`).
2. OSC payloads containing `;` are truncated at the first `;` (`ansi.rs:1176`), e.g. a cwd with `;`. Escape values in the shell scripts, as inshellisense does with `__is_escape_value`, or join `params[1..]` with `;`.
3. `cursor_idx` is a UTF-8 byte offset; reconciliation backspace counts are byte deltas (`term/mod.rs:744`, `message.rs:195-197`).
4. Multi-line commands are flattened with runs of spaces instead of `\n`; any text below the prompt line is included (§10.2).
5. A LEADING_WIDE_CHAR_SPACER cell adds a spurious space (§10.2).
6. Ctrl-C's interceptor reset only matches lowercase `Char('c')`, which the legacy parser never produces (§6.2).
7. `control+j` can't be a binding: LF is parsed as Enter (§12.2).
8. The inline debounce loop overwrites `LAST_RECEIVED`, so throttling retries abort (§15.2). The unanchored `50\s+.*` cleaning regex can truncate valid output.
9. A disabled or invalid inline request sometimes returns **no response** at all, and the client waits the full 5 s timeout.
10. `EXPECTED_BUFFER` uses the raw insertion including `\b`/escapes, so the lock mostly releases only on the 16 ms timer.
11. Blocking `send` on the bounded(16) main-loop channel from inside the VT parser can deadlock.
12. The update check has inverted `app.disableAutoupdates` logic.
13. Windows delayed events are flushed LIFO (`Vec::pop`), so NewCmd is processed before EndPrompt (`term/mod.rs:866-881`).
14. Remote IPC drops messages queued while disconnected (`outgoing_rx.drain()`).

---

## 20. What tabsmith should take from this (recommendations)

- **Keep:**
  - the launch model: rc `pre` block → `exec -a "<shell> (tabsmith)" tabsmith`, with `TABSMITH_TERM`, `TABSMITH_TERM_TMUX` and `TABSMITH_SESSION_ID` guards, and the same skip list (Emacs, Warp, SecureCRT, CI, ssh-without-parent, non-tty, non-interactive, `PROCESS_LAUNCHED_BY_TABSMITH`)
  - fallback to `exec`ing the plain shell on any startup error
  - the OSC protocol idea: StartPrompt/EndPrompt/NewCmd=<sid>/PreExec/Dir/Shell/ExitCode/PID/TTY/User/Lock/Unlock/suggestion-color. Consider a private OSC number of our own, or reuse **697** for drop-in compatibility with Fig/Kiro-style scripts. inshellisense uses `6973` with `PS`/`PE`/`CWD;…` (`ish/src/utils/ansi.ts:11-14`, `ish/shell/*`).
  - per-cell `IN_PROMPT`/`IN_SUGGESTION` marks, and edit-buffer extraction from `cmd_cursor`, fixing quirks 3–5 (use char indices, insert `\n` at hard line breaks, stop at the last non-blank row)
  - insertion via `\x08` × n + text + `ESC[D` × m, with buffer reconciliation and a short lock
  - the key-binding table and the reset rules
  - the RunProcess env sanitizing for spec generators
- **Simplify (single binary, in-process UI):**
  - no remote socket and no handshake. Hooks become in-process function calls into the completion engine.
  - keep only a small per-session local socket (for `tabsmith _ pre-cmd` env/alias updates and `tabsmith _ inline …` if the zsh plugin path is kept), using a cross-platform local-socket crate (named pipes on Windows) instead of `tokio::net::UnixListener` **(recommendation)**
- **Cross-platform:**
  - `portable-pty` (already used by figterm for `PtySize`/`Child`; it implements ConPTY on Windows)
  - `crossterm` for raw mode, size and resize events on all OSes (already a figterm dependency)
  - `vte` for parsing into a minimal own grid, or a vendored, trimmed alacritty_terminal fork
- **New for tabsmith (no figterm prior art):** the wrapper must draw the dropdown and ghost text itself, as stdout overlays: save cursor, draw below or above the prompt, restore. It must also erase/redraw on every shell output chunk, and must never let those overlay bytes into the shadow grid that feeds the edit buffer. See inshellisense `ish/src/isterm/pty.ts` and `ish/src/ui/` for the rendering approach.
