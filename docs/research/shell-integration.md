# Shell integration scripts and installation (research for tabsmith)

Paths in this report are relative to the reference roots:

- `qac/` = `.../scratchpad/ref/qac` (aws/amazon-q-developer-cli-autocomplete, the Fig/Q/Kiro lineage)
- `ish/` = `.../scratchpad/ref/ish` (microsoft/inshellisense)

Citations use `file:line`. "(inferred)" marks conclusions that are not directly stated in the code.

---

## 0. TL;DR architecture

1. The user's dotfile gets two tiny blocks: a **pre block** (first line of the file) and a **post block** (last line). Each block only `source`s a small script file kept in the app data dir. That script file contains a single `eval "$(q init <shell> <pre|post> --rcfile <name>)"`. So the real integration code is always produced at runtime by `q init` (versioned with the binary, never stale on disk).
2. **pre** (`pre.sh` / `pre.fish` / `pre.nu`): decides whether to start the PTY wrapper (`qterm`, a.k.a. figterm). If so it `exec`s it (replacing the shell process). qterm creates a PTY, sets `Q_TERM`, `QTERM_SESSION_ID`, etc. and starts the *same* shell again inside it. That inner shell re-runs the dotfile, and the pre block now sees `Q_TERM` and does nothing.
3. **post** (`post.zsh` / `post.bash` / `post.fish` / `post.nu`): installs precmd/preexec hooks and wraps the prompt variables so the shell emits **OSC 697** markers: `StartPrompt`, `EndPrompt`, `NewCmd=<session>`, `PreExec`, `OSCLock/OSCUnlock`, and context like `Dir=`, `ExitCode=`, `Shell=`, `PID=`, `TTY=`.
4. **The shell never reports the edit buffer or cursor.** There is no `zle-line-pre-redraw`, no `bind -x`, and no fish event for the buffer. qterm runs a full terminal emulator (a fork of alacritty_terminal) over the PTY output. It rebuilds the edit buffer from the screen grid, starting at the cursor position saved at `NewCmd`. It leaves out cells written between `StartPrompt` and `EndPrompt`, and cells drawn in the shell's ghost-text color.
5. zsh only: the post block also injects a renamed copy of **zsh-autosuggestions v0.7.0**. Its only strategy asks qterm for the inline suggestion (`q _ inline-shell-completion --buffer "$BUFFER"`) and draws it as `POSTDISPLAY` ghost text. Right arrow, End and Ctrl-E accept the suggestion, because they are the `forward-char`, `end-of-line` and `vi-*` widgets.
6. inshellisense uses the same idea with a simpler protocol. It uses **OSC 6973** with `PS`, `PE` and `CWD;<escaped>`. It injects the integration through shell launch flags (`--init-file`, ZDOTDIR swap, `--init-command`, `-noexit -command`, `-e`, `--rc`), not through `exec` from the dotfile. It supports **PowerShell, pwsh and cmd.exe on Windows**.

Name mapping for the rebrand: `CLI_BINARY_NAME="q"`, `PTY_BINARY_NAME="qterm"`, `PRODUCT_NAME="Amazon Q"`, `DATA_DIR_NAME="amazon-q"` (unix) or `"AmazonQ"` (win), `BACKUP_DIR_NAME=".amazon-q.dotfiles.bak"` (`qac/crates/fig_util/src/consts.rs:15-35`). Scripts use placeholders `{{CLI_BINARY_NAME}}`, `{{PTY_BINARY_NAME}}`, `{{CLI_BINARY_NAME_UNDERSCORE}}` and `{{CLI_BINARY_NAME_UPPER}}`. These are replaced with `.replace()` at build or run time (`qac/crates/fig_integrations/src/shell/mod.rs:225-227`, `qac/crates/fig_integrations/build.rs:37-42`). For tabsmith: `tabsmith`, `tabsmith-term` (or `tabsmith _ pty`), `TABSMITH`, `tabsmith` (my suggestion, inferred).

---

## 1. Environment variables (the contract between scripts, CLI and PTY)

Declared in `qac/crates/fig_util/src/consts.rs:107-143`; others come from the scripts and figterm.

| Var | Set by | Meaning / use |
|---|---|---|
| `QTERM_SESSION_ID` | qterm (`figterm/src/main.rs:484-491`): `uuid::Uuid::new_v4().simple()` (32 hex, no dashes), overridable via `MOCK_QTERM_SESSION_ID` | Identifies the PTY session. Sent in `OSCLock=`, `OSCUnlock=` and `NewCmd=`. Used to find the qterm socket `<sockets_dir>/t/<id>.sock` (`fig_util/src/directories.rs:382-384`). |
| `Q_TERM` | qterm, set to the qterm version (`figterm/src/main.rs:434`) | "Already inside qterm". Used by the fallback launch logic (exit code 2). |
| `Q_TERM_TMUX` | qterm, only if `TMUX` was set when qterm started (`main.rs:435-437`) | Lets qterm start again inside tmux panes: launch if `TMUX` is set and `Q_TERM_TMUX` is not. |
| `Q_SHELL` | `q init` assigns it to the parent-process exe path (`init.rs:133-144`); pre.sh falls back to `q _ get-shell` | qterm runs this shell (falls back to `$SHELL`) (`main.rs:390-400`). qterm removes it from the child env (`main.rs:440`). |
| `Q_IS_LOGIN_SHELL` | pre script: `1` if bash `shopt -q login_shell` or zsh `-o login` or fish `status --is-login` | qterm adds `--login` (`main.rs:418-420`). Removed from the child env. |
| `Q_EXECUTION_STRING` | pre.sh: `${BASH_EXECUTION_STRING:=$ZSH_EXECUTION_STRING}` | qterm adds `-c <string>` (`main.rs:422-424`). |
| `Q_START_TEXT` | pre.sh: typeahead read from stdin before exec | qterm writes it plus `\n` to the PTY after the first prompt is seen (`main.rs:620-630`). |
| `Q_SHELL_EXTRA_ARGS` | external | qterm appends it split on whitespace, dropping `--login` (`main.rs:426-428`). |
| `Q_TERM_PATH` | pre script, or `q init` from setting `qterm.path` (`init.rs:202-204`) | Path of the PTY binary to exec. |
| `SHOULD_QTERM_LAUNCH` | `q init <shell> pre` emits `SHOULD_QTERM_LAUNCH="<0\|1\|2>"` (`init.rs:146-153`) | Cached result of `should-figterm-launch` (§3). A plain shell var, not exported. |
| `PROCESS_LAUNCHED_BY_Q` | set to `1` when the app spawns processes (`figterm/src/message.rs:141`) | Pre block: never launch qterm. Post block: emit `DoneSourcing`. |
| `Q_PARENT`, `Q_SET_PARENT`, `Q_SET_PARENT_CHECK` | SSH integration (§9) | Remote socket forwarding. pre.sh moves `Q_SET_PARENT` into `Q_PARENT` once. |
| `Q_NEW_SESSION` | external (not set anywhere in the Rust sources) | pre: `unset QTERM_SESSION_ID Q_TERM Q_NEW_SESSION`, which forces a fresh qterm. |
| `Q_LOG_LEVEL` | user | Reported via `Log=`. |
| `Q_ZDOTDIR` | user | Fallback for `ZDOTDIR` when locating zsh dotfiles (`fig_util/src/shell.rs:109-116`). |
| `Q_USING_ZSH_AUTOSUGGESTIONS` | zsh inline plugin guard, exported `=1` when the real zsh-autosuggestions is loaded | Tells the system that inline is disabled. |
| `Q_DOTFILES_SOURCED` | guard var around the zsh inline plugin (§6) | Prevents loading the plugin twice. |
| `Q_FORCE_FIGTERM_LAUNCH`, `Q_TERM_DISABLED` | user | Force or disable launch (§3). |
| `Q_DISABLE_AUTOCOMPLETE` | user | Non-empty disables autocomplete inside qterm (`figterm/src/main.rs:348-351`). |
| `Q_PROMPT_OFFSET_WORKAROUND` | user | Integer. Shifts the saved command-start column left (emoji prompt fix) (`term/mod.rs` in `new_cmd_internal`). |
| `TTY`, `SHELL_PID` | post scripts: `export TTY` (from `tty`), `export SHELL_PID=$$` | Context. |
| `Q_INIT_SNAPSHOT_TEST` | tests | Makes `init` output deterministic (`init.rs:36`). |

---

## 2. `q init <shell> <pre|post> [--rcfile <name>]`

Clap definition (`qac/crates/q_cli/src/cli/init.rs:38-48`): positional `shell` ∈ {bash, zsh, fish, nu}, positional `when` ∈ {pre, post}, and optional `--rcfile <String>`. The rcfile value is the dotfile name without its leading dot and without any extension, e.g. `zshrc`, `zprofile`, `bashrc`, `bash_profile`, `bash_login`, `profile`, `00_fig_pre`, `99_fig_post`.

`execute` (`init.rs:51-59`) always exits 0. It prints the generated source followed by `\n`. On error it prints `# Could not load source: {err}`, which is a shell comment, so the `eval` never breaks.

### 2.1 Output composition, in order (`init.rs:121-295`)

Each item is a string. Items are joined with `"\n"`.

1. **Linux + `--rcfile profile`**: return `""` (empty) (`:122-125`). The comment says it "can cause issues on launch".
2. If state key `shell-integrations.enabled` is false: return only the disabled notice (`:127-129`, `:297-308`):
   ```sh
   if [ -z "${Q_SHELL_INTEGRATION_DISABLED}" ]; then
     printf 'Amazon Q shell integration is disabled.\nRe-enable by running: <magenta>q _ local-state -d shell-integrations.enabled</magenta>\n'
     Q_SHELL_INTEGRATION_DISABLED="1"
   fi
   ```
3. `Q_SHELL="<parent exe path>"` when the parent process exe is known (`:133-144`). In snapshot tests the value is `/bin/zsh`.
4. **pre only**: `SHOULD_QTERM_LAUNCH="<n>"` from `should_figterm_launch_exit_status(ctx, quiet=true)` (`:146-153`).
5. **post only, zsh only**: the inline autosuggest plugin, wrapped in `guard_source(..., "Q_DOTFILES_SOURCED", AfterSourcing, ZSH_SCRIPT)` (`:155-174`). All of these must hold:
   - rcfile is not `zprofile` (the code also excludes bash `profile` and `bash_profile`, which is redundant because the shell must be zsh);
   - state `dotfiles.enabled` is true (default true);
   - setting `inline.enabled` is true (default true);
   - this is not a snapshot test.
6. post: if state `shell-integrations.immediateLogin` is true, set it false and push `q login` (`:186-190`). Out of scope for tabsmith.
7. **pre + bash + JetBrains terminal** (`TERMINAL_EMULATOR == "JetBrains-JediTerm"`, `fig_util/src/terminal.rs:570-576`): push `Q_IS_LOGIN_SHELL=1` (`:193-200`). The reason is that JediTerm does not start a real login shell, so `shopt -q login_shell` is wrong.
8. If setting `qterm.path` is set: `Q_TERM_PATH="<path>"` (`:202-204`).
9. **The shell script itself**: `shell.get_fig_integration_source(when)` (`:206-207`), see §4 and §5.
10. **pre + JetBrains (macOS bundle)**: re-source the JetBrains integration after the exec, guarded by `Q_JETBRAINS_SHELL_INTEGRATION` with `BeforeSourcing` (`:209-252`). For bash, `[ -f '{bundle}/Contents/plugins/terminal/jediterm-bash.in' ] && source ...` (plus the `bash/` variant). For zsh, `.zshenv` (plus the `zsh/` variant). For fish, `fish/config.fish` and `fish/init.fish`.
11. macOS post: input-method promotion prompt. Out of scope.
12. **post + zsh + inline enabled**: a one-time `printf` "now supports AI-powered inline completions! To disable run: q inline disable", guarded by `Q_INLINE_PROMPT`. It is counted in state `prompt.inline.count < 1` (`:270-279`, `:325-336`).

### 2.2 Helpers (exact formats)

`assign_shell_variable(shell, name, value, exported)` (`init.rs:68-76`):

| shell | not exported | exported |
|---|---|---|
| bash/zsh | `NAME="value"` | `export NAME="value"` |
| fish | `set -g NAME "value"` | `set -gx NAME "value"` |
| nu | `let-env NAME = "value";` | same |

`guard_source(shell, export, guard_var, assignment, source)` (`init.rs:78-119`) builds the following. Every source line is indented two spaces.

```
bash/zsh:  if [ -z "${GUARD}" ]; then      fish: if test -z "$GUARD"      nu: if env | any name == 'GUARD' {
             GUARD="1"        (BeforeSourcing: assignment first, avoids recursion)
             <source lines>
             GUARD="1"        (AfterSourcing: assignment last)
           fi\n                             end\n                           }
```

Note that the nu guard condition looks inverted (`any name == ...`), and nu uses the obsolete `let-env` (see §5.4).

### 2.3 Snapshot of a real output (`qac/crates/q_cli/tests/snapshots/init__init_snapshot_zsh_pre_zshrc.snap`)

```sh
Q_SHELL="/bin/zsh"
SHOULD_QTERM_LAUNCH="0"
#!/usr/bin/env bash
mkdir -p "${HOME}/.local/bin" > /dev/null 2>&1
...(rest of pre.sh)
```

---

## 3. Launch decision: `q _ should-figterm-launch` (exit 0 = launch, 1 = don't, 2 = "fall back to the `Q_TERM` check")

Source: `qac/crates/q_cli/src/cli/internal/should_figterm_launch.rs`. Checks run in order (`:206-295`). The first match wins.

1. `Q_FORCE_FIGTERM_LAUNCH` set → **0**
2. `Q_TERM_DISABLED` set → 1
3. `PROCESS_LAUNCHED_BY_Q` set → 1
4. `INSIDE_EMACS` set → 1
5. `TERM_PROGRAM == "WarpTerminal"` → 1
6. `__CFBundleIdentifier == "com.vandyke.SecureCRT"` → 1
7. `__PWSH_LOGIN_CHECKED` set (`pwsh -Login` re-running the profile) → 1
8. CI: `CI` or `Q_CI` set (`fig_os_shim/src/env.rs:158-160`) → 1
9. SSH (`SSH_CLIENT` / `SSH_CONNECTION` / `SSH_TTY`, `env.rs:150-152`) **and** `Q_PARENT` unset → 1
10. WSL → **2**
11. Otherwise `should_launch()` (`:165-203`):
    - **Parent** (the shell running `q`; `:56-98`): no parent pid, path or name → 1. `valid_parent` = exe file name ∈ {`zsh`, `bash`, `fish`, `nu`}. Then: SSH with `Q_TERM` unset → **0**. Codespaces (`CODESPACES` or `Q_CODESPACES`) → 1 if `Q_TERM` is set, else 0.
    - **Grandparent** (the terminal emulator; `:100-162`): valid if its exe name is in `MACOS_TERMINALS`/`LINUX_TERMINALS` + `SPECIAL_TERMINALS`. On Linux it is also valid if `/proc/<pid>/cmdline` matches (first argv basename, or the second argv for python-launched Terminator/Guake; `terminal.rs:296-333`).
    - macOS and the grandparent is not "special" → **2** (`:195-200`).
    - Otherwise `0` if parent and grandparent are both valid, else `1`.
12. Windows build (`:302-311`): `GetConsoleMode(stdin)` succeeds → 2, else 1. (Note: `ExitCode::from(...)` is missing `return`/value use; the Windows CLI was effectively unsupported.)

Terminal tables (`qac/crates/fig_util/src/terminal.rs`):
- `MACOS_TERMINALS` (`:12-31`): Alacritty, iTerm, Kitty, Tabby, Terminal.app, VSCode Insiders, VSCode, VSCodium, WezTerm, Zed, Cursor, Cursor Nightly, Rio, Windsurf, WindsurfNext, Ghostty, Positron, Trae.
- `LINUX_TERMINALS` (`:34-51`): Alacritty, Kitty, GnomeConsole, GnomeTerminal, Guake, Hyper, Konsole, XfceTerminal, WezTerm, Tilix, Terminator, VSCode, VSCodeInsiders, VSCodium, IntelliJ, Positron.
- `SPECIAL_TERMINALS` (`:54-60`, `is_special` `:600-605`): ssh (`sshd`), tmux (`tmux`, `tmux: server`), nvim, vim, zellij. These are "pseudo-terminals we also want to launch within".
- Exe names (`:495-528`): code, code-insiders, alacritty, kitty, kgx, gnome-terminal-server, konsole, tilix, xfce4-terminal, terminology, wezterm/wezterm-gui, hyper, tabby, terminator, zed, Cursor/cursor, "Cursor Nightly"/cursor-nightly, rio, windsurf, windsurf-next, guake, ghostty, positron, trae.

**Kiro / VSCode / JetBrains guards.** Nothing in qac blocks VSCode or JetBrains: VSCode is a supported terminal, and JetBrains only gets the login-shell fix and the re-sourcing from §2.1. There is no Kiro entry, because Kiro is newer than this snapshot. Kiro is a VSCode fork, so (inferred) it probably sets `TERM_PROGRAM=kiro`, and its exe would be `kiro`/`Kiro`. Treat it like VSCode, or block it by default. inshellisense additionally refuses to auto-start when `VSCODE_RESOLVING_ENVIRONMENT` is set (`ish/src/utils/shell.ts:330-372`). VSCode, and therefore Kiro and Cursor, spawn a hidden interactive login shell to read the env, and wrapping it would hang. tabsmith should copy that guard.

**Why exit code 2 exists.** On macOS the process tree is unreliable: the app bundle exe name differs from the terminal name. So the script falls back to "launch unless already inside" (`Q_TERM` unset), plus the tmux exception (§4.1 line 37).

---

## 4. The pre block

### 4.1 bash & zsh: `qac/crates/fig_integrations/src/shell/scripts/pre.sh` (77 lines; essential lines verbatim)

For bash, `get_fig_integration_source` puts the whole of `bash-preexec.sh` in front of it, wrapped in a function (§5.2.1). zsh gets `pre.sh` unchanged (`shell/mod.rs:193-228`).

```sh
mkdir -p "${HOME}/.local/bin" > /dev/null 2>&1
if [[ -d "${HOME}/.local/bin" ]] && [[ ":$PATH:" != *":${HOME}/.local/bin:"* ]]; then
  PATH="${PATH:+"$PATH:"}${HOME}/.local/bin"
fi
if [[ -n "${Q_NEW_SESSION:-}" ]]; then
  unset QTERM_SESSION_ID; unset Q_TERM; unset Q_NEW_SESSION      # (3 separate lines in source)
fi
if [[ -z "${Q_SET_PARENT_CHECK:-}" ]]; then
  if [[ -z "${Q_PARENT:-}" && -n "${Q_SET_PARENT:-}" ]]; then
    export Q_PARENT="$Q_SET_PARENT"; unset -v Q_SET_PARENT
  fi
  export Q_SET_PARENT_CHECK=1
fi
# 0 = Yes, 1 = No, 2 = Fallback to Q_TERM
if [ -z "${SHOULD_QTERM_LAUNCH:-}" ]; then
  q _ should-figterm-launch 1>/dev/null 2>&1
  SHOULD_QTERM_LAUNCH=$?
fi
if   [[ -t 1 ]] \
  && [[ -z "${PROCESS_LAUNCHED_BY_Q:-}" ]] \
  && command -v qterm 1>/dev/null 2>&1 \
  && [[ ("${SHOULD_QTERM_LAUNCH}" -eq 0) || (("${SHOULD_QTERM_LAUNCH}" -eq 2) && (-z "${Q_TERM:-}" || (-z "${Q_TERM_TMUX:-}" && -n "${TMUX:-}"))) ]]
then
  if [ -z "${Q_SHELL:-}" ]; then Q_SHELL=$(q _ get-shell); fi
  Q_IS_LOGIN_SHELL="${Q_IS_LOGIN_SHELL:='0'}"
  if ([[ -n "${BASH:-}" ]] && shopt -q login_shell) || [[ -n "${ZSH_NAME:-}" && -o login ]]; then
    Q_IS_LOGIN_SHELL=1
  fi
  # Do not launch figterm in non-interactive shells (like VSCode Tasks)
  if [[ $- == *i* ]]; then
    Q_TERM_NAME="$(basename "${Q_SHELL}") (qterm)"
    if [[ -z "${Q_TERM_PATH:-}" ]]; then
      if [[ -x "${HOME}/.local/bin/${Q_TERM_NAME}" ]]; then
        Q_TERM_PATH="${HOME}/.local/bin/${Q_TERM_NAME}"
      else
        Q_TERM_PATH="$(command -v qterm || echo "${HOME}/.local/bin/qterm")"
      fi
    fi
    Q_EXECUTION_STRING="${BASH_EXECUTION_STRING:=$ZSH_EXECUTION_STRING}"
    INITIAL_TEXT=""
    if [[ -z "${BASH:-}" || "${BASH_VERSINFO[0]}" -gt "3" ]]; then
      while read -rt 0; do
        if [[ -n "${BASH:-}" ]]; then read -r; fi
        INITIAL_TEXT="${INITIAL_TEXT}${REPLY}\n"
      done
    fi
    Q_EXECUTION_STRING="${Q_EXECUTION_STRING}" Q_START_TEXT="$(printf "%b" "${INITIAL_TEXT}")" Q_SHELL="${Q_SHELL}" Q_IS_LOGIN_SHELL="${Q_IS_LOGIN_SHELL}" exec -a "${Q_TERM_NAME}" "${Q_TERM_PATH}"
  fi
fi
```

Guards, in summary: stdout is a TTY (`-t 1`), not app-spawned, the qterm binary is on PATH, the launch decision (0, or 2 + not inside / tmux), and the shell is interactive (`$-` contains `i`). The SSH/CI/Emacs/Warp/... checks live inside `should-figterm-launch`.

**argv[0] trick.** `exec -a "zsh (qterm)"` makes the process show up as "zsh (qterm)", which keeps terminal tab titles sensible. The desktop installer also copies the qterm binary to `~/.local/bin/"<shell> (qterm)"` for every shell, re-copying when `--version` differs (`qac/crates/fig_desktop/src/install.rs:266-300`). macOS reads the process name from the executable file, so `exec -a` alone does not change it there (inferred). Uninstall removes `<shell> (<pty>)` copies (`fig_install/src/common.rs:88-93`).

**Typeahead.** Keys typed before the wrapper started are drained with `read -t 0`. They are forwarded as `Q_START_TEXT`, which qterm types into the PTY with a trailing `\n` once `has_seen_prompt` is true (`figterm/src/main.rs:620-630`). This is skipped on bash 3 (macOS `/bin/bash`).

### 4.2 fish: `scripts/pre.fish` (`:1-51`)

Same logic in fish syntax. Differences:
- `builtin contains $HOME/.local/bin $PATH; or set --append PATH $HOME/.local/bin`.
- Login and interactive checks use `status --is-login` and `status --is-interactive`.
- Line 13 has an inverted-looking parent copy: `if test -n "$Q_PARENT"; and test -z "$Q_SET_PARENT"; set --export Q_PARENT $Q_SET_PARENT`.
- fish's `exec` has no `-a`, so it goes through bash:
  ```fish
  exec bash -c "Q_PARENT=$Q_PARENT Q_SHELL=$Q_SHELL Q_IS_LOGIN_SHELL=$Q_IS_LOGIN_SHELL exec -a \"$Q_TERM_NAME\" \"$Q_TERM_PATH\""
  ```
  No typeahead and no execution string are passed.

### 4.3 nushell: `scripts/pre.nu`

Legacy and broken on modern nu. It uses `let-env` (removed in nu 0.83+) and hardcoded `~/.fig/bin/nu (figterm)` / `figterm` paths (`:41-48`). It runs `exec $figterm_path` with `Q_SHELL` in `with-env`. No dotfile is ever installed for nu (`get_shell_integrations` returns `vec![]`, `shell/mod.rs:187`), and nu is excluded from `integrations install dotfiles` (`integrations.rs:131-134`). **For tabsmith, model nu on inshellisense instead (§10).**

### 4.4 What qterm does after the exec (needed to re-implement the loop)

`figterm/src/main.rs`:
- `qterm.enabled` state false → prints `[NOTE] qterm is disabled. Autocomplete will not work.` and returns (`:982-986`).
- The session id is generated and `QTERM_SESSION_ID` is set in its own env (`:484-491`), so the child inherits it.
- Child command (`build_shell_command`, `:403-449`): `$Q_SHELL` (else `$SHELL`) + `--login` if `Q_IS_LOGIN_SHELL=1` + `-c $Q_EXECUTION_STRING` + `Q_SHELL_EXTRA_ARGS`. Env `Q_TERM=<ver>`, `Q_TERM_TMUX=<ver>` if `TMUX`. It removes `Q_SHELL`, `Q_IS_LOGIN_SHELL`, `Q_START_TEXT`, `Q_SHELL_EXTRA_ARGS` and `Q_EXECUTION_STRING`, and sets cwd = current dir.
- It writes the title `ESC ] 0 ; <shell basename> BEL` to the real terminal (`:608-615`).
- On any startup error it falls back to `execvp` of the plain shell (`launch_shell`, `:454-476`, used at `main` on `Err`). **tabsmith must keep this fallback.** A broken wrapper would otherwise make the terminal unusable.
- The PTY output, OSC 697 included, is forwarded to the real stdout unchanged. Unknown OSCs are ignored by real terminals (inferred).

---

## 5. The post block per shell (hooks + OSC 697 emission)

Every post script defines `fig_osc` = `printf "\033]697;$1\007" "${@:2}"`. It is a printf format, so callers pass `%s`/`%d` args. The terminator is always **BEL**.

### 5.1 zsh: `scripts/post.zsh` (193 lines)

The whole body is inside `if [[ -n "${ZSH_NAME:-}" ]]; then ... fi`. It does the following:
- PATH gets `~/.local/bin`. `TTY=$(tty)` if unset, `export TTY`, `export SHELL_PID="$$"`, and `Q_SHELL=$(q _ get-shell)` if unset.
- `fig_preexec` (`:24-75`): restores the user's prompt variables from saved copies, for each of `PS1 PROMPT prompt PS2 PROMPT2 PS3 PROMPT3 PS4 PROMPT4 RPS1 RPROMPT RPS2 RPROMPT2`, but only if the variable is set (`[ -n "${VAR+x}" ]`). It sets `Q_HAS_SET_PROMPT=0` and emits:
  ```
  ESC]697;OSCLock=<QTERM_SESSION_ID>BEL
  ESC]697;PreExecBEL
  ```
- `fig_precmd` (`:77-171`):
  1. `local LAST_STATUS=$?` (must be first), then `fig_reset_hooks`.
  2. It emits, in this order: `OSCUnlock=<sid>`, `Dir=$PWD`, `Shell=zsh`, `ShellPath=${Q_SHELL:-$SHELL}`, `WSLDistro=$WSL_DISTRO_NAME` (only if set), `PID=$$` (`%d`), `ExitCode=$LAST_STATUS`, `TTY=$TTY`, `Log=$Q_LOG_LEVEL`, `ZshAutosuggestionColor=$ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE`, `FigAutosuggestionColor=$Q_AUTOSUGGEST_HIGHLIGHT_STYLE`, `User=${USER:-root}`.
  3. If `Q_HAS_SET_PROMPT == 1` (Ctrl-C at the prompt, so preexec never ran), it calls `fig_preexec` to restore the prompts first.
  4. It saves every user prompt var into `Q_USER_<VAR>` and wraps them:
     ```zsh
     START_PROMPT=$'\033]697;StartPrompt\007'
     END_PROMPT=$'\033]697;EndPrompt\007'
     NEW_CMD=$'\033]697;NewCmd='"${QTERM_SESSION_ID}"$'\007'
     # primary prompt: first of PROMPT / prompt / PS1 that is set
     PROMPT="%{$START_PROMPT%}$PROMPT%{$END_PROMPT$NEW_CMD%}"
     # PS2/PROMPT2, PS4/PROMPT4, RPS1/RPROMPT, RPS2/RPROMPT2: %{START%}...%{END%}  (no NewCmd)
     # PS3/PROMPT3 (select prompt): %{START%}...%{END NEW_CMD%}
     ```
     `%{...%}` tells zsh the bytes have zero width.
  5. `Q_HAS_SET_PROMPT=1`, then a background `(command q _ pre-cmd --alias "$(\alias)" > /dev/null 2>&1 &) >/dev/null 2>&1`.
- `fig_reset_hooks` (`:173-184`) keeps `fig_precmd` **last** in `precmd_functions` and `fig_preexec` **first** in `preexec_functions`. It runs on every precmd, so it wins against plugins that reorder hooks:
  ```zsh
  if [[ "$precmd_functions[-1]" != fig_precmd ]]; then
    precmd_functions=(${(@)precmd_functions:#fig_precmd} fig_precmd); fi
  if [[ "$preexec_functions[1]" != fig_preexec ]]; then
    preexec_functions=(fig_preexec ${(@)preexec_functions:#fig_preexec}); fi
  ```
- It finishes with `fig_reset_hooks`, emits `DoneSourcing` if `PROCESS_LAUNCHED_BY_Q`, and (outside the `if ZSH_NAME`) one more background `q _ pre-cmd`.

`q _ pre-cmd [--alias <text>]` (`internal/mod.rs:164-167`, `:873-909`) connects to the qterm socket for `$QTERM_SESSION_ID`. It sends `UpdateShellContext { update_environment_variables: true, environment_variables: <all env>, update_alias: true, alias }`, which gives qterm the shell's env and aliases for generators. It exits 1 silently if not inside qterm. (inferred) tabsmith can replace this with an OSC carrying a compact payload, or keep a socket.

### 5.2 bash: `scripts/post.bash` (183 lines), with vendored bash-preexec

#### 5.2.1 bash-preexec vendoring
- File: `scripts/bash-preexec.sh`, upstream rcaloras/bash-preexec **V0.5.0** (`:12`), MIT. `scripts/update-bash-preexec.sh` just runs `curl -sSfL -o bash-preexec.sh https://raw.githubusercontent.com/rcaloras/bash-preexec/master/bash-preexec.sh`. inshellisense ships its own copy (`ish/shell/bash-preexec.sh`, also V0.5.0, a slightly different revision).
- Both bash pre and bash post are emitted as (`shell/mod.rs:199-220`):
  ```sh
  function __fig_source_bash_preexec() {
  <entire bash-preexec.sh>
  }
  __fig_source_bash_preexec
  # shellcheck disable=SC2329
  function __bp_adjust_histcontrol() { :; }
  <pre.sh | post.bash>
  ```
  Reasons (inferred): bash-preexec uses top-level `return` (non-bash, bash < 3.1, already imported; `bash-preexec.sh:40-55`). Inside a function those `return`s only leave the function, not the whole `eval`. Redefining `__bp_adjust_histcontrol` as a no-op stops bash-preexec from rewriting the user's `HISTCONTROL` (it strips `ignorespace` and rewrites `ignoreboth`; `:92-101`).
- Key bash-preexec mechanics used. The DEBUG trap `trap '__bp_preexec_invoke_exec "$_"' DEBUG` (`:298`) runs `preexec_functions`. `PROMPT_COMMAND` gets `__bp_precmd_invoke_cmd` (runs `precmd_functions`) and `__bp_interactive_mode`. `__bp_install_after_session_init` (`:363-376`) appends an install string to `PROMPT_COMMAND` so the install happens at the first prompt. `__bp_set_ret_value` restores `$?` (`:181-183`). On bash ≥ 5.1, `PROMPT_COMMAND` is used as an array (`:343-348`).

#### 5.2.2 post.bash logic
- PATH, `TTY`, `SHELL_PID`, `Q_SHELL` as in zsh. Initial `Q_LAST_PS1/2/3="$PSx"`.
- `__fig_preexec` (`:27-47`): emits `OSCLock=<sid>` and `PreExec`. It restores `PSx=$Q_USER_PSx` only if `PSx` still equals what we last set (`Q_LAST_PSx`), so a prompt the user changed is kept. Sets `_fig_done_preexec="yes"`.
- `__fig_preexec_preserve_status` (`:49-53`) wraps it and restores `$?` via `__bp_set_ret_value "${__fig_ret_value}" "${__bp_last_argument_prev_command:?}"`.
- `__fig_pre_prompt` (`:55-82`, first in `precmd_functions`): `__fig_ret_value="$?"`. Emits `OSCUnlock`, `Dir`, `Shell=bash`, `ShellPath`, `WSLDistro`?, `PID`, `ExitCode=$__fig_ret_value`, `TTY`, `Log`, `User` (no color keys), then a background `q _ pre-cmd --alias "$(\alias)"`. CentOS 7.2 workaround: `[[ -z "${_fig_done_preexec:-}" ]] && __fig_preexec ""`, then `_fig_done_preexec=""`, then it restores `$?`.
- `__fig_post_prompt` (`:84-115`, near the end of `PROMPT_COMMAND`, **after** the user's own `PROMPT_COMMAND`, so prompt frameworks that rewrite `PS1` run first):
  ```bash
  if [[ -z "${Q_USER_PS1+x}" || "${PS1}" != "${Q_LAST_PS1}" ]]; then Q_USER_PS1="${PS1}"; fi   # same for PS2, PS3
  START_PROMPT="\[$(fig_osc StartPrompt)\]"
  END_PROMPT="\[$(fig_osc EndPrompt)\]"
  NEW_CMD="\[$(fig_osc NewCmd=${QTERM_SESSION_ID})\]"
  __bp_set_ret_value "${__fig_ret_value}" "${__bp_last_argument_prev_command}"
  PS1="${START_PROMPT}${Q_USER_PS1}${END_PROMPT}${NEW_CMD}"
  PS2="${START_PROMPT}${Q_USER_PS2}${END_PROMPT}"
  PS3="${START_PROMPT}${Q_USER_PS3}${END_PROMPT}${NEW_CMD}"
  Q_LAST_PS1="${PS1}"; Q_LAST_PS2="${PS2}"; Q_LAST_PS3="${PS3}"
  ```
  `\[...\]` marks the bytes as zero-width for readline.
- `__fig_reset_hooks` (`:117-164`) rebuilds `PROMPT_COMMAND` = `<existing minus our entries>\n__fig_post_prompt\n__bp_interactive_mode`. It strips `__fig_post_prompt[;\n]` and `__bp_interactive_mode[;\n]` occurrences and runs `__bp_sanitize_string` on the rest. It forces `precmd_functions[0]=__fig_pre_prompt` and `preexec_functions[0]=__fig_preexec_preserve_status` (dedupes, then prepends). Effective order: `__fig_pre_prompt`, other precmds, original PROMPT_COMMAND, `__fig_post_prompt`, `__bp_interactive_mode`.
- `if [[ "${TERM_PROGRAM}" != "WarpTerminal" ]]; then __bp_install_after_session_init; fi` (`:173-175`). This re-ensures bash-preexec is installed even if the user overwrote `PROMPT_COMMAND` (withfig/fig#888). Warp ships its own preexec.
- Then `__fig_reset_hooks`, `DoneSourcing` if launched by the app, and a final background `q _ pre-cmd`.

### 5.3 fish: `scripts/post.fish` (112 lines)

- `set --query TTY; or set TTY (command tty)`, `set --export SHELL_PID $fish_pid`, `fig_osc` = `builtin printf "\033]697;$argv[1]\007" $argv[2..-1]`.
- Function copying for prompt wrapping: `fig_copy_fn src dst` = `functions --erase dst; functions --copy src dst`. `fig_fn_defined` = `functions --query`.
- `fig_wrap_prompt` (`:26-34`): saves `$status`, then prints `StartPrompt`, then `printf "%b" (string join "\n" $argv)`, then `EndPrompt`, then `return $last_status`.
- `fig_preexec --on-event fish_preexec` (`:36-51`): `OSCLock=<sid>`, `PreExec`. Restores `fish_mode_prompt` / `fish_right_prompt` from the `fig_user_*` copies if present. Always restores `fish_prompt` from `fig_user_prompt`. Sets `fig_has_set_prompt 0`.
- `fig_precmd --on-event fish_prompt` (`:53-104`): `OSCUnlock`, `Dir`, `Shell=fish`, `ShellPath=$Q_SHELL`, `WSLDistro`?, `PID=$fish_pid`, `ExitCode`, `TTY`, `Log`, **`FishSuggestionColor=$fish_color_autosuggestion`**, `User=$USER` (or `User=root`). If `fig_has_set_prompt = 1` it calls `fig_preexec`. Then it wraps each prompt function:
  ```fish
  fig_copy_fn fish_prompt fig_user_prompt
  function fish_prompt
      fig_wrap_prompt (fig_user_prompt)
      fig_osc NewCmd=$QTERM_SESSION_ID
  end
  ```
  `fish_mode_prompt` and `fish_right_prompt` are wrapped without `NewCmd`. After that: `set fig_has_set_prompt 1` and `begin; command q _ pre-cmd --alias (alias) &> /dev/null &; end`.
- Installed as fish `conf.d` files (§7.2), so there is no dotfile editing for fish.

### 5.4 nushell: `scripts/post.nu` (211 lines, legacy `let-env`/`def-env` syntax)

- `fig_osc` = `$"(ansi -o '697');($s)(char bel)"`.
- `fig_reset_hooks` appends closures to `$env.config.hooks.pre_prompt` and `.pre_execution`.
- The pre-exec hook emits `OSCLock` and `PreExec`. The pre-prompt hook emits `OSCUnlock`, `Dir`, `Shell=nu`, `ShellPath`, `WSLDistro`, `PID=$nu.pid`, `ExitCode=$env.LAST_EXIT_CODE`, `TTY=(^tty)`, `Log`, **`NuHintColor=$env.config.color_config.hints`**, `User`. If the binary `fig` exists it runs `q _ pre-cmd`, and if that prints `EXEC_NEW_SHELL` it runs `exec nu`.
- Prompt wrapping (`fig_set_prompt`, `:160-202`):
  - `PROMPT_COMMAND` → `StartPrompt + original`
  - `PROMPT_COMMAND_RIGHT` → `Start + orig + End`
  - `PROMPT_INDICATOR`, `PROMPT_INDICATOR_VI_INSERT` and `PROMPT_INDICATOR_VI_NORMAL` → `orig + EndPrompt + NewCmd=<sid>`
  - `PROMPT_MULTILINE_INDICATOR` → `Start + orig + End`
- **Do not port verbatim.** Rewrite with `$env.X = ...` and closures (see ish §10.3).

---

## 6. OSC 697 protocol as parsed by qterm

Parser: `qac/crates/alacritty_terminal/src/ansi.rs:1175-1272` (`osc_dispatch`, `params` = the OSC payload split on `;` by the vte parser).

- Format: `ESC ] 697 ; <Token> BEL`. `ST` (`ESC \`) is also accepted by vte (inferred).
- Bare tokens: `NewCmd` (same as `NewCmd=`), `StartPrompt`, `EndPrompt`, `PreExec`.
- `Key=Value` tokens: split at the first `=`. If the value part is ≤ 1 byte (empty value), the token is **ignored**. Keys: `Dir`, `ShellPath`, `WSLDistro`, `ExitCode` (i32), `Shell`, `FishSuggestionColor`, `ZshAutosuggestionColor`, `FigAutosuggestionColor`, `NuHintColor`, `TTY`, `PID` (i32), `User`, `Log`, `NewCmd`, `OSCLock`, `OSCUnlock`. `DoneSourcing` is not handled here; the app strips everything before it in login-shell output (`qac/packages/api-bindings-wrappers/src/executeCommandWrappers.ts:10,82-84`).
- **Edge case:** only `params[1]` is read. A value containing `;` (e.g. a cwd like `/tmp/a;b`) is truncated, and values are not escaped. tabsmith should adopt inshellisense's escaping (§10.1).

Semantics in the emulator (`qac/crates/alacritty_terminal/src/term/mod.rs`):

| Event | Effect |
|---|---|
| `OSCLock=<sid>` (`:1922-1935`) | If `sid` equals the local session id (or no local id is known), set `osc_lock = true`. While locked, StartPrompt/EndPrompt/Dir/ShellPath/WSLDistro/ExitCode/Shell/colors/TTY/PID/Log are ignored. This stops nested shells or programs printing 697 sequences from corrupting state. `NewCmd` and `User` are **not** gated. |
| `OSCUnlock=<sid>` (`:1937-1949`) | Same id check, sets `osc_lock = false`. Sent first in every precmd. |
| `StartPrompt` (`:1730-1739`) | `has_seen_prompt = true`. The cursor template gets the `IN_PROMPT` flag, so every cell printed until `EndPrompt` is marked as prompt. |
| `EndPrompt` (`:1741-1747`, `:796-804`) | Clears `IN_PROMPT`. On Windows it is delayed (see below). |
| `NewCmd[=<sid>]` (`:806-859`) | If `sid` is non-empty and differs from the local id, it is ignored (a remote or nested session). Otherwise `cmd_cursor = cursor.point`, the **start of the edit buffer**, optionally shifted left by `Q_PROMPT_OFFSET_WORKAROUND`. Sets `preexec = false` and fires `Event::Prompt` and `Event::CommandInfo` (finished command, for history). |
| `PreExec` (`:1749-1773`) | Once per command (`preexec` flag). Fires `Event::PreExec`, which hides the popup. Snapshots the current buffer as `command_info.command` with cwd, start time, shell, pid, session and user. |
| `ExitCode` (`:1804-1814`) | Fills `command_info.exit_code` and `end_time`. |
| `Dir` (`:1775-1786`) | Sets the context cwd **and calls `env::set_current_dir`** in qterm (so generators run in the shell's cwd). |
| `Shell` (`:1816-1829`) | Fires `ShellChanged` on change. |
| colors (`:1831-1882`) | Parsed into the suggestion color used for `IN_SUGGESTION` (below). |

### 6.1 Edit buffer and cursor: no shell cooperation, read from the grid

- `get_current_buffer` (`term/mod.rs:755-789`): rect from `(cmd_cursor.line, col 0)` to `(bottommost line, last column)`, starting at `cmd_cursor.column`. Returns `None` if `cmd_cursor` scrolled above `topmost_line`. If the cursor is inside, the result is `trim_end()`-ed and then padded with spaces back to the cursor index.
- `get_text_region` (`:681-752`): iterates cells. It **skips** cells that are `'\0'`, flagged `IN_PROMPT` (e.g. RPROMPT drawn after the command start), or flagged `IN_SUGGESTION`. Skipped cells become padding spaces only if real text follows. Wide-char spacers are skipped, and zero-width combining chars are appended. Padding resets at the end of each row. `cursor_idx` is the buffer length when the cell under the cursor is reached.
- `IN_SUGGESTION` (`:1455-1492`): on every SGR fg/bg change, the cell is marked as suggestion if it matches the shell's suggestion color. The color comes from `fish_suggestion_color` (fish), `fig_autosuggestion_color` or else `zsh_autosuggestion_color` (zsh), or `nu_hint_color` (nu). A missing fg or bg component in the suggestion style matches anything. **This is why the post scripts report the ghost-text colors.** Without them the ghost text would be treated as typed text.
- **Windows (ConPTY)** (`figterm/src/main.rs:583-584`, `:859-870`; `term/mod.rs:796-830`, `:862-881`): `EndPrompt` and `NewCmd` are queued as delayed events. After each PTY read, if the queue is non-empty and did not grow during that read, the queue is flushed (`force=true`, with `NewCmd` using no sid check). ConPTY re-renders and can emit the OSC before the prompt text, so qterm takes the cursor position only after the prompt text arrives (inferred from the code).

---

## 7. Dotfile installation (`fig_integrations`)

### 7.1 Which files, per shell (`qac/crates/fig_integrations/src/shell/mod.rs:132-191`)

Config dir (`fig_util/src/shell.rs:106-123`): bash → `$HOME`; zsh → `$ZDOTDIR` or else `$Q_ZDOTDIR` or else `$HOME`; fish → `$__fish_config_dir` or else `~/.config/fish`; nu → `<config_dir>/nushell` (unused).

| Shell | Integration objects |
|---|---|
| bash | `DotfileShellIntegration{pre:true, post:true}` for `.bashrc` always, plus each of `.profile`, `.bash_login`, `.bash_profile` **that exists**. If none of those three exists, `.profile` is added (it will be created). |
| zsh | `.zshrc` and `.zprofile` (both always, created if missing). |
| fish | `ShellScriptShellIntegration` files (no dotfile editing): `<fish_config>/conf.d/00_fig_pre.fish` (When::Pre) and `<fish_config>/conf.d/99_fig_post.fish` (When::Post). fish loads conf.d files in lexical order before `config.fish`. The post file therefore runs *before* config.fish (inferred limitation). |
| nu | none |

Script files referenced from the dotfiles (`script_integration`, `:403-415`; `integration_file_name`, `:93-98`): `<fig_data_dir>/shell/<dotfile without leading dot>.<pre|post>.<shell>`. Examples: `zshrc.pre.zsh`, `zprofile.post.zsh`, `bashrc.pre.bash`, `bash_profile.post.bash`, `profile.pre.bash`. `fig_data_dir` = `dirs::data_local_dir()/amazon-q`, which is `~/Library/Application Support/amazon-q` on macOS, `$XDG_DATA_HOME|~/.local/share/amazon-q` on Linux, and `%LOCALAPPDATA%\AmazonQ` on Windows (`fig_util/src/directories.rs:141-150`).

### 7.2 Script file contents (`ShellScriptShellIntegration::get_contents`, `shell/mod.rs:293-331`)

`rcfile` = `--rcfile <file name up to the first '.'>`. For example, `zshrc.pre.zsh` gives `--rcfile zshrc` and `00_fig_pre.fish` gives `--rcfile 00_fig_pre`.

**macOS** (one line, no trailing newline):
```sh
# bash/zsh
[ -x ~/.local/bin/q ] && eval "$(~/.local/bin/q init zsh pre --rcfile zshrc)"
# fish
test -x ~/.local/bin/q; and eval (~/.local/bin/q init fish pre --rcfile 00_fig_pre | string split0)
```

**Linux/other** (verbatim from snapshots `shell/snapshots/*.snap`; ends with `\n`):
```sh
_Q_LOCAL_BIN="$HOME/.local/bin"
[[ ":$PATH:" != *":$_Q_LOCAL_BIN:"* ]] && PATH="${PATH:+"$PATH:"}$_Q_LOCAL_BIN"
unset _Q_LOCAL_BIN

[ -n "$BASH_VERSION" ] && command -v q >/dev/null 2>&1 && eval "$(q init bash pre --rcfile bashrc)"
```
- zsh omits the `[ -n "$BASH_VERSION" ] && ` guard. The guard exists because `.profile` can be read by `sh`/`dash`.
- fish: `contains $HOME/.local/bin $PATH; or set -a PATH $HOME/.local/bin` + `\n` + `command -qv q; and eval (q init fish pre --rcfile 00_fig_pre | string split0)`.

`FileIntegration` (`file.rs`) behaviour:
- `install` (`:77-106`) is a no-op if already installed. Otherwise it does `create_dir_all(parent)` and writes with truncate, applying the optional unix mode.
- `is_installed` (`:33-75`): the content must be **byte-equal** to the expected text, else `ImproperInstallation("<path> should contain:\n<contents>")`. A missing file gives `FileDoesNotExist`. On unix it first checks R|W|X access on the parent dir.
- `uninstall` (`:108-114`) removes the file and treats NotFound as Ok.

### 7.3 The block written into the dotfile

`description` (`:418-423`) and `source_text` (`:512-521`). The path is `$HOME`-relative with `${HOME}` written literally, and is quoted:

```sh
# Amazon Q pre block. Keep at the top of this file.
[[ -f "${HOME}/.local/share/amazon-q/shell/zshrc.pre.zsh" ]] && builtin source "${HOME}/.local/share/amazon-q/shell/zshrc.pre.zsh"
...user content...
# Amazon Q post block. Keep at the bottom of this file.
[[ -f "${HOME}/.local/share/amazon-q/shell/zshrc.post.zsh" ]] && builtin source "${HOME}/.local/share/amazon-q/shell/zshrc.post.zsh"
```
fish form: `test -f "${HOME}/..."; and builtin source "${HOME}/..."` (only used for legacy matching). On macOS the path is `${HOME}/Library/Application Support/amazon-q/shell/...`, which works because of the quotes. Edge case: `strip_prefix(home)` fails with an error if the data dir is outside `$HOME` (e.g. `XDG_DATA_HOME=/data`).

Note: `[[ ... ]]` is used even in `.profile`/`.bash_profile`. That is fine for bash, but `.profile` sourced by `dash` would error. The *script file* guards `$BASH_VERSION`, but the `[[` in the dotfile line itself is evaluated first (inferred risk). tabsmith should use `[ -f ... ] && . ...` in `.profile`.

### 7.4 Install algorithm (`install_inner`, `shell/mod.rs:590-629`; `install`, `:644-650`)

```
install():
  if is_installed().is_ok(): return Ok            # idempotent
  install_inner()

install_inner():
  if dotfile exists:
      backup_file(dotfile, utc_backup_dir())        # copy to ~/.amazon-q.dotfiles.bak/<YYYY-MM-DD_HH-MM-SS UTC>/<file name>
      self.uninstall()                              # strips any old/legacy blocks (and deletes script files)
      contents = read(dotfile)
  else contents = ""
  original = contents
  if pre:
      script_integration(Pre).install()             # (re)write <data>/shell/<name>.pre.<shell>
      (shebang, rest) = split_shebang(contents)     # first line kept on top iff it starts with "#!"
      contents = shebang + DESC_PRE + "\n" + SOURCE_PRE + "\n" + rest
  if post:
      script_integration(Post).install()
      contents = contents + "\n" + DESC_POST + "\n" + SOURCE_POST + "\n"
  if contents != original: File::create(dotfile).write_all(contents)
```
- Backup dir: `backups_dir() = ~/.amazon-q.dotfiles.bak`. `utc_backup_dir()` adds a subdirectory named with the `[year]-[month]-[day]_[hour]-[minute]-[second]` UTC timestamp (`fig_util/src/directories.rs:310-323`). `backup_file` copies only if the file exists (`fig_integrations/src/backup.rs:8-21`).
- `split_shebang` (`shell/mod.rs:759-768`): `("#!...\n", rest)` if the text starts with `#!` and has a newline. Otherwise `("", text)`.
- Dotfiles that are symlinks: `File::create` writes through the symlink (inferred). Doctor only warns "dotfiles are symlinked" (`doctor/mod.rs:1320-1340`).

### 7.5 Status detection (`is_installed`, `shell/mod.rs:691-729`; `matches_text`, `:556-580`)

```
text = read(dotfile)                 # NotFound -> FileDoesNotExist
if text =~ (?mi)^\s*#\s*fig ignore\s?.*$ : return Ok   # user opt-out marker: treated as installed
filtered = text.replace_all((?m)^\s*(#.*)?\n, "").trim()   # drop comment & blank lines (incl. our DESC lines and shebang)
for when in [pre, post]:
   if legacy_regexes(when).is_match(filtered): Err(LegacyInstallation("<path> has legacy <when> integration."))
   if !source_regex(when, false).is_match(filtered): Err(NotInstalled("<path> does not source <when> integration"))
   if !source_regex(when, true).is_match(filtered):  Err(ImproperInstallation("<path> does not source <when> integration <first|last>"))
   script_integration(when).is_installed()?     # script file must exist with exact contents
```
`source_regex(when, constrain)` (`:523-540`) is `{^ if constrain&&pre}(?:<escaped DESC>\n)?<escaped SOURCE>\n{0,2}{$ if constrain&&post}`. It has **no `(?m)`**, so `^`/`$` anchor the whole filtered text: pre must be the very first non-comment line and post the very last.

`migrate()` (`:731-740`) reinstalls only when the result is `LegacyInstallation`.

### 7.6 Uninstall (`shell/mod.rs:652-689`, `remove_from_text` `:542-554`)

```
if dotfile exists:
   c = read
   c = replace_all((?mi)^#.*fig.*var.*$\n?, "")                                   # old Fig "fig var" comment lines
   c = replace_all((?mi)^#.*Please make sure this block is at the .* of this file.*$\n?, "")
   if pre:  c = remove_from_text(c, Pre)    # fold replace_all over [source_regex(Pre,false)] + legacy_regexes(Pre)
   if post: c = remove_from_text(c, Post)
   c = c.trim() + "\n"
   write(dotfile, c)
script_integration(Pre/Post).uninstall()   # delete <data>/shell/... files
```
Legacy regexes (`legacy_regexes`, `:433-478`; `old_brand_regex`, `:582-588`):
1. `\[ -s ~/\.fig/shell/pre\.sh \] && source ~/\.fig/shell/pre\.sh\n?` (pre) or `\[ -s ~/\.fig/fig\.sh \] && source ~/\.fig/fig\.sh\n?` (post).
2. `(?m)(?:# CodeWhisperer pre block. Keep at the top of this file.\n)?^<old eval source>\n{0,2}`. The old eval source for pre is `export PATH="${PATH}:${HOME}/.local/bin"\neval "$(q init zsh pre)"`; fish uses `set -Ua fish_user_paths $HOME/.local/bin\neval (q init fish pre | string split0)`; post is just the eval line.
3. and 4. `. "$HOME/<rel path>"` and `[[ -f "$HOME/<rel>" ]] && . "$HOME/<rel>"` (fish: `if test -f ...; . ...; end`), each with an optional CodeWhisperer description.
5. Old-brand regex, whitespace tolerant, against the old data dir `<data_local_dir>/codewhisperer/shell/<file>`: `(?m)(?:\s*<desc>\s*\n)?^\s*\[\[\s*\-f\s*"\$\{HOME\}/<rel>"\s*\]\]\s*&&\s*builtin\s+source\s*"\$\{HOME\}/<rel>"\s*\n{0,2}`. Its unit test (`:838-901`) shows it removes repeated lines and keeps unrelated comments.

Caution for tabsmith: regex 1 of the uninstall (`^#.*fig.*var.*$`) is aggressive and can delete unrelated user comments containing "fig" and "var". Use exact marker lines instead (recommendation).

### 7.7 CLI surface: `q integrations ...` (`qac/crates/q_cli/src/cli/integrations.rs`)

```
q integrations install   <dotfiles [bash|zsh|fish|nu] | ssh | input-method | vscode | intellij-plugin(alias jetbrains-plugin) | autostart-entry | gnome-shell-extension | all(hidden)> [-s|--silent]
q integrations uninstall <...> [-s]
q integrations reinstall <...> [-s]      # = uninstall then install
q integrations status    <...> [-f|--format plain|json|json-pretty]
```
- `install dotfiles` without a shell installs `[Bash, Zsh, Fish]` (`:131-134`). For each integration it skips if `is_installed` is Ok, otherwise it calls `install()` and collects errors as `"\n\n<describe bold>: <verbose_message>"`. It prints `Installed!` if anything was installed, or `Already installed` if nothing was and nothing errored (`:237-247`).
- `uninstall dotfiles` calls `uninstall()` only on integrations whose `is_installed()` is Ok. It prints `Uninstalled!` or `Not installed` (`:381-387`).
- `status dotfiles` (`:410-481`) always covers bash, zsh and fish. Plain output is one line per integration: `✔ ` (green) or `✘ ` (red) followed by `describe()`. `describe()` is `"{shell} pre post into .zshrc"` for dotfiles (`:634-642`) and `"{shell} {when}"` for fish script files (`:348-350`). JSON output:
  `{"integrations":[{"installed":bool,"description":str,"shell":"bash|zsh|fish","file_name":".zshrc|00_fig_pre.fish"}],"errors":[{"shell":str,"error":str}]}`.
- `all`: install = dotfiles + ssh (+ input-method on mac). Uninstall also covers autostart and the gnome extension on Linux.
- Global install flow (`installation.rs:56-106`, `fig_install/src/common.rs:142-163`): asks "Do you want q to modify your shell config (you will have to manually do this otherwise)?". If declined or failed, it prints manual instructions such as `bash:    . "$HOME/<data>/shell/bashrc.pre.bash"` (top) and `... bashrc.post.bash` (bottom), and still writes the script files.
- `doctor` (`doctor/mod.rs:943-1010`, `:2134-2152`): one `DotfileCheck` per integration, named `"<shell> ~/<path> integration check"`. It is a NormalCheck if it is the current shell, a SoftCheck if that shell binary is on PATH, else skipped. The fix runs `integration.install()`. The hint is `Run q integrations install dotfiles to reinstall shell integrations for <shell>`.

---

## 8. The zsh inline ghost-text plugin (vendored zsh-autosuggestions)

### 8.1 Build and packaging

`qac/crates/fig_integrations/build.rs:5-44` concatenates these files **in this order**: `guard_start.zsh`, `LICENSE` (MIT notice of zsh-autosuggestions **v0.7.0**, © Thiago de Arruda, Eric Freese, all `#` comment lines), `config.zsh`, `util.zsh`, `bind.zsh`, `highlight.zsh`, `widgets.zsh`, `strategies/inline.zsh`, `strategies/completion.zsh`, `strategies/history.zsh`, `strategies/match_prev_cmd.zsh`, `fetch.zsh`, `async.zsh`, `start.zsh`, `guard_end.zsh`. It replaces `{{CLI_BINARY_NAME}}`→`q`, `{{CLI_BINARY_NAME_UNDERSCORE}}`→`q`, `{{CLI_BINARY_NAME_UPPER}}`→`Q`. Exposed as `inline_shell_completion_plugin::ZSH_SCRIPT = "\n" + script + "\n"` (`shell/mod.rs:39-44`). Its unit test asserts the text contains "Copyright", `_q_autosuggest_strategy_inline_shell_completion()` and `add-zsh-hook precmd _q_autosuggest_start` (`:52-67`).

Upstream names `_zsh_autosuggest_*` / `ZSH_AUTOSUGGEST_*` become `_q_autosuggest_*` / `Q_AUTOSUGGEST_*`, and widgets are `autosuggest-*`. **Conflict guard** (`guard_start.zsh`, `guard_end.zsh`):
```zsh
if command -v _zsh_autosuggest_accept >/dev/null 2>&1; then
  export Q_USING_ZSH_AUTOSUGGESTIONS=1
else
  ...whole plugin...
fi
```
So if the user already loads zsh-autosuggestions (in .zshrc, before the post block), Q inline is disabled. qterm then removes the user's ghost text using `ZshAutosuggestionColor`.

### 8.2 Config defaults (`config.zsh`; each set only if not already defined)

| Var | Default |
|---|---|
| `Q_AUTOSUGGEST_HIGHLIGHT_STYLE` | `'fg=8'` (also reported as `FigAutosuggestionColor`) |
| `Q_AUTOSUGGEST_ORIGINAL_WIDGET_PREFIX` | `autosuggest-orig-` |
| `Q_AUTOSUGGEST_STRATEGY` | `(inline_shell_completion)`, the only strategy enabled |
| `Q_AUTOSUGGEST_CLEAR_WIDGETS` | history-search-forward, history-search-backward, history-beginning-search-forward, history-beginning-search-backward, history-substring-search-up, history-substring-search-down, up-line-or-beginning-search, down-line-or-beginning-search, up-line-or-history, down-line-or-history, accept-line, copy-earlier-word |
| `Q_AUTOSUGGEST_ACCEPT_WIDGETS` | forward-char, end-of-line, vi-forward-char, vi-end-of-line, vi-add-eol |
| `Q_AUTOSUGGEST_EXECUTE_WIDGETS` | (empty) |
| `Q_AUTOSUGGEST_PARTIAL_ACCEPT_WIDGETS` | forward-word, emacs-forward-word, vi-forward-word, vi-forward-word-end, vi-forward-blank-word, vi-forward-blank-word-end, vi-find-next-char, vi-find-next-char-skip |
| `Q_AUTOSUGGEST_IGNORE_WIDGETS` | `orig-\*`, beep, run-help, set-local-history, which-command, yank, yank-pop, `zle-\*` |
| `Q_AUTOSUGGEST_COMPLETIONS_PTY_NAME` | `q_autosuggest_completion_pty` |
| optional user vars | `Q_AUTOSUGGEST_BUFFER_MAX_SIZE`, `Q_AUTOSUGGEST_MANUAL_REBIND`, `Q_AUTOSUGGEST_HISTORY_IGNORE`, `Q_AUTOSUGGEST_USE_ASYNC` |

**Keybindings.** No keys are bound directly. Existing widgets are wrapped instead. In the default emacs keymap, Right arrow / Ctrl-F is `forward-char` (accepts the whole suggestion when the cursor is at the end), Ctrl-E / End is `end-of-line` (accept), Alt-F / Ctrl-Right is `forward-word` (partial accept, word by word), and Up/Down/Enter are clear widgets (inferred from the standard zsh bindings).

### 8.3 Widget machinery (`bind.zsh`, `widgets.zsh`, `highlight.zsh`)

- `_q_autosuggest_bind_widgets` (`bind.zsh:63-92`): for every widget in `zle -la` except `.*`, `_*`, `autosuggest-<builtin action>`, `$PREFIX*` and the ignore list, it binds the widget to the action given by its list: clear, accept, execute or partial_accept. Every other widget gets `modify`.
- `_q_autosuggest_bind_widget w action` (`:12-60`) saves the original widget:
  - user widget → `zle -N autosuggest-orig-<n>-w <fn>`
  - builtin → `_q_autosuggest_orig_w() { zle .w }` + `zle -N`
  - completion widget → `zle -C`
  - already ours → reuse the count

  It then defines `_q_autosuggest_bound_<n>_w() { _q_autosuggest_widget_<action> autosuggest-orig-<n>-w $@ }` and `zle -N -- w` pointing at it. `<n>` is a per-widget bind counter (`_Q_AUTOSUGGEST_BIND_COUNTS`).
- Each `_q_autosuggest_widget_<action>` (`widgets.zsh:201-236`) does `highlight_reset; _q_autosuggest_<action> $@; highlight_apply; zle -R`. Builtin actions are exposed as widgets `autosuggest-{clear,fetch,suggest,accept,execute,enable,disable,toggle}`.
- Highlighting: `region_highlight+=("$#BUFFER $(($#BUFFER + $#POSTDISPLAY)) $Q_AUTOSUGGEST_HIGHLIGHT_STYLE")`. The last entry is remembered and removed on reset (`highlight.zsh:7-26`).
- `modify` (`widgets.zsh:39-83`):
  1. Save BUFFER/POSTDISPLAY, `unset POSTDISPLAY`, run the original widget.
  2. If `PENDING > 0 || KEYS_QUEUED_COUNT > 0`, restore POSTDISPLAY and return (typing fast, no fetch).
  3. If the new BUFFER extends the old one and the old suggestion starts with the added text (the user is typing the suggestion), shift the suggestion: `POSTDISPLAY="${orig_postdisplay:$(($#BUFFER - $#orig_buffer))}"`, with no fetch.
  4. If disabled, return.
  5. If `$#BUFFER > 0` and under the max size, `fetch`.
- `fetch`: async if `Q_AUTOSUGGEST_USE_ASYNC` is defined, which `start.zsh` defines for zsh ≥ 5.0.8. Otherwise synchronous. `suggest s` sets `POSTDISPLAY="${s#$BUFFER}"` if s and BUFFER are non-empty, else unsets it.
- `accept` (`:110-148`):
  1. If `CURSOR != $#BUFFER` (minus 1 in vicmd) or there is no POSTDISPLAY, run the original widget and return.
  2. Otherwise run `(q _ inline-shell-completion-accept --buffer "$BUFFER" --suggestion "$POSTDISPLAY" > /dev/null 2>&1 &)` (telemetry and cache).
  3. `BUFFER="$BUFFER$POSTDISPLAY"; unset POSTDISPLAY`, run the original widget, then `CURSOR=$#BUFFER` (−1 in vicmd).
- `execute` does the same, then calls the original `accept-line`. `partial_accept` temporarily appends the suggestion, runs the movement widget, and splits BUFFER/POSTDISPLAY at the new cursor.
- `fetch.zsh:9-27` tries each strategy in turn. A suggestion is discarded unless it starts with the buffer (`[[ "$suggestion" != "$1"* ]]`).
- **Strategy** (`strategies/inline.zsh:8-10`):
  ```zsh
  typeset -g suggestion="$(command -v q >/dev/null 2>&1 && q _ inline-shell-completion --buffer "${BUFFER}")"
  ```
  `q _ inline-shell-completion --buffer B` (`internal/inline_shell_completion.rs:37-79`) sends `InlineShellCompletionRequest{buffer}` to the qterm socket with a **5 s timeout** and prints `B + insert_text` followed by a newline. The newline is stripped by `$(...)`. It exits 1 with no output on failure. qterm debounces 300 ms by default and uses a completion cache (`figterm/src/inline/mod.rs:56-80`, `:164-210`). The engine itself is out of scope here.
- Async (`async.zsh:6-76`):
  1. Cancel the previous request: close the fd, `zle -F` off, and `kill -TERM -<pgid>` if `MONITOR` is set, else `kill -TERM <pid>`.
  2. `exec {FD}< <( echo $sysparams[pid]; _q_autosuggest_fetch_suggestion "$1"; echo -nE "$suggestion" )`. `zmodload zsh/system` provides `$sysparams`.
  3. Read the child pid line, then `zle -F $FD _q_autosuggest_async_response`.
  4. The handler reads everything when there is no error or on `hup`, runs `zle autosuggest-suggest -- "$suggestion"` and closes the fd.
  5. `is-at-least 5.8 || command true` is a fork workaround for zsh-autosuggestions#364.
- `start.zsh`: `autoload -Uz add-zsh-hook is-at-least`, async enabled if `is-at-least 5.0.8`, `add-zsh-hook precmd _q_autosuggest_start`. That hook rebinds all widgets on **every** precmd, so it stays outermost over zsh-syntax-highlighting. If `Q_AUTOSUGGEST_MANUAL_REBIND` is set, it removes itself after the first run.
- The other strategies are present but unused by default: `history` (`${history[(r)$prefix*]}` with escaping), `match_prev_cmd`, and `completion` (zpty-based). For a tabsmith "local history" mode, setting `TABSMITH_AUTOSUGGEST_STRATEGY=(tabsmith history)` gives an offline fallback (recommendation).

Other shells: bash, fish and nu get **no** shell-side ghost-text plugin in qac. fish and nu have native autosuggestions/hints, which qterm only excludes from the buffer via the color keys. For bash, (inferred) ghost text would have to be drawn by the PTY wrapper itself as an overlay, the way inshellisense draws its UI.

---

## 9. SSH integration (probably out of scope for tabsmith)

`qac/crates/fig_integrations/src/ssh/mod.rs`:
- Appends to `~/.ssh/config` (mkdir `~/.ssh` with 0700, backup first, uninstall old block first):
  ```
  # Amazon Q SSH Integration. Keep at the bottom of this file.
  Match all
    Include "~/<rel data dir>/ssh"
  ```
- `<data>/ssh` (mode 0600):
  ```
  Match exec "command -v q && q internal generate-ssh --remote-host %h --remote-port %p --remote-username %r"
      Include "<data>/ssh_inner"
  ```
- `q _ generate-ssh` (`q_cli/src/cli/internal/generate_ssh.rs`) writes `ssh_inner` with `RemoteForward '/tmp/q-parent-<uuid>.socket' '<remote socket>'`, `SetEnv Q_SET_PARENT=...`, `StreamLocalBindMask 600`, `StreamLocalBindUnlink yes`, `PermitLocalCommand yes` and `LocalCommand <exe> _ ssh-local-command '%r@%n' '<uuid>' 1>&2`. This happens only if the desktop's remote socket is reachable and the user is not `git` or `aur`. Otherwise it writes just a header.
- Uninstall regex: `(?:<desc>\n)?Match all\n\s+Include\s+<escaped "~/...">\n{0,2}`.

This exists only to tunnel the *desktop app* socket into remote shells, which explains the `Q_PARENT`/`Q_SET_PARENT` handling in pre.sh and the "SSH without Q_PARENT → don't launch" guard. A self-contained in-terminal wrapper does not need it: tabsmith running locally already wraps the ssh session's PTY, but completions inside the remote shell would need tabsmith installed remotely (inferred).

---

## 10. inshellisense shell plugins (`ish/shell/*`, `ish/src/utils/shell.ts`, `ish/src/isterm/*`)

### 10.1 Protocol: OSC 6973 (`ish/src/utils/ansi.ts:11-21`)

- `ESC ] 6973 ; PS BEL` marks the prompt start. `ESC ] 6973 ; PE BEL` marks the prompt end, i.e. where the command starts. `ESC ] 6973 ; CWD ; <escaped path> BEL` reports the cwd.
- **Value escaping** (identical in bash, zsh, fish, nu, xonsh and pwsh): process byte-by-byte with `LC_ALL=C`. `\` → `\\`, `;` → `\x3b`, newline → `\x0a`, ESC → `\x1b`, BEL → `\x07`. pwsh hex-escapes each UTF-8 byte of any of `[ESC BEL \ \n ;]` as `\xHH` (`shellIntegration.ps1:10-16`).
- Decoder (`isterm/pty.ts:158-160`): `message.replaceAll(/\\(\\|x([0-9a-f]{2}))/gi, (_, op, hex) => hex ? String.fromCharCode(parseInt(hex,16)) : op)`.
- Handler (`pty.ts:177-196`): sequence = text before the first `;`. `PS` → `handlePromptStart`, `PE` → `handlePromptEnd`. `CWD` → `path.resolve(sanitize(decode(arg)))`, where sanitize strips surrounding quotes, converts Git-Bash `/c/...` to `C:\...` on Windows, and uppercases the drive letter.
- There is **no** preexec, exit-code, session-id or lock event. "Command accepted" is inferred by the UI (`clearActiveCommand` on Enter).

### 10.2 How each shell is launched with the integration (`ish/src/isterm/pty.ts:432-508`)

Common env: `ISTERM=1` (nesting guard), `ISTERM_TESTING=1` under test, and `ISTERM_LOGIN=1` when `--login`. PTY: node-pty `name: "xterm-256color"`, `useConpty: true`, `useConptyDll: true` (bundled conpty.dll; on Windows this is what lets OSC sequences pass through, inferred).

| Shell | Target | Args | Plugin behavior |
|---|---|---|---|
| bash | `bash`. On Windows, Git Bash found via `git.exe` → `../..`, `ProgramW6432`, `ProgramFiles`, `ProgramFiles(X86)`, `%LocalAppData%\Program` (each joined with `\Git\bin\bash.exe`, `\Git\usr\bin\bash.exe`, `\usr\bin\bash.exe`) and scoop paths (`shell.ts:214-261`) | `--init-file <res>/shell/shellIntegration.bash` | `shellIntegration.bash`: if `ISTERM_LOGIN` is unset, sources `~/.bashrc`. Otherwise sources `/etc/profile` and the first existing of `~/.bash_profile`, `~/.bash_login`, `~/.profile`. Then it sources the sibling `bash-preexec.sh` (path via `cygpath -u` if available). `__is_update_prompt` wraps `PS1="\[PS\]$orig\[PE\]"`, recomputed when `PS1` changes. If bash-preexec loaded: `precmd_functions+=(__is_precmd)` and, if the user had a `PROMPT_COMMAND`, `PROMPT_COMMAND=__is_prompt_cmd_safe` (re-run the originals, then re-wrap if PS1 changed). Without bash-preexec: `PROMPT_COMMAND=__is_prompt_cmd_original` or `__is_precmd`. Note `--login` is **not** passed for bash; login is emulated via `ISTERM_LOGIN`. |
| zsh | `zsh` | `--login` if login | **ZDOTDIR swap** (`shell.ts:54-55`, `165-175`). It creates `$TMPDIR/is-zsh-<pid>/` with `.zshrc`, `.zprofile`, `.zshenv` and `.zlogin` copied from `shellIntegration-{rc,profile,env,login}.zsh`, and sets `ZDOTDIR=<that dir>`, `USER_ZDOTDIR=${ZDOTDIR:-$HOME}`. Each file sources the user's real file with `ZDOTDIR` temporarily switched back. `.zshenv` has a recursion guard. The dir is removed on process exit. rc: `add-zsh-hook precmd __is_precmd` (emits CWD) and wraps `zle-line-init` so **PS/PE are emitted when line editing starts** (`zle -A zle-line-init __is_orig_zle_line_init`), writing `printf '\e]6973;PS\a' > /dev/tty`. So PE lands exactly at the edit-buffer start, even with complex or async prompts. |
| fish | `fish` | `--init-command ". <res>/shell/shellIntegration.fish"` (spaces escaped; on Windows `. "$(cygpath -u '<path>')"`), plus `--login` | Copies `fish_prompt` to `is_user_prompt` via `functions ... | sed | source`. The new `fish_prompt` prints `PS + prompt lines joined with '\n' + PE` using `printf '%s'`. CWD is emitted on the `fish_prompt` event. |
| pwsh / powershell (Windows only for `powershell`) | `pwsh` / `powershell.exe` | `-noexit -command "try { . \"<res>/shell/shellIntegration.ps1\" } catch {}"`, plus `-login` first if login | Saves `$function:Prompt` and redefines `Global:Prompt` to return `ESC]6973;PS BEL` + original prompt + `ESC]6973;PE BEL` + (if the provider is FileSystem) `ESC]6973;CWD;<escaped ProviderPath> BEL`. CWD is placed *after* PE. It is invisible, so the cursor column is unaffected (inferred). |
| cmd (Windows only) | `cmd.exe` | none | No script. Env `PROMPT = <PS OSC> + ($env:PROMPT or "$P$G") + <PE OSC>` (`pty.ts:494-501`). The raw ESC/BEL bytes are placed in the env var. No CWD reporting; ish recovers cwd elsewhere (not covered here). |
| nu | `nu` | `-e "source \`<res>/shell/shellIntegration.nu\`"` (+ `-n` in tests), plus `--login` | `$env.PROMPT_COMMAND = {|| "PS" + original(closure or string) + CWD }` and `$env.PROMPT_INDICATOR = {|| indicator + "PE" }`. Nu redraws the prompt on every keystroke and after accept, so the UI uses `getShellPromptRewrites` and ignores re-written prompts on accepted lines (`shell.ts:287-289`, `commandManager.ts:95-99`). |
| xonsh | `xonsh` | `--rc <existing of /etc/xonsh/xonshrc|C:\ProgramData\xonsh\xonshrc, ~/.xonshrc, ~/.config/xonsh/rc.xsh, ~/.config/xonsh/rc.d> <res>/shell/shellIntegration.xsh`, plus `--login` | `$PROMPT = "{__is_prompt_start}{__is_update_cwd}" + $PROMPT + "{__is_prompt_end}"`, using `\001`...`\002` to mark non-printing spans. |

### 10.3 Auto-start from the user profile (instead of `exec` in a pre block)

`is init <shell>` writes per-shell snippet files to `<resources>/init/<shell>/init.{sh,zsh,fish,ps1,xsh,nu}` (`shell.ts:134-161`). It then prints a one-line source command that **the user** appends to the end of their profile. ish never edits dotfiles itself. Source commands (`shell.ts:300-325`):
```
bash:  [ -f '<p>' ] && source '<p>'
zsh:   [[ -f '<p>' ]] && source '<p>'
fish:  test -f '<p>' && source '<p>'
pwsh:  if ( Test-Path '<p>' -PathType Leaf ) { . '<p>' }
xonsh: p"<p>".exists() && source "<p>"
nu:    if ( "<p>" | path exists ) { source "<p>" }
```
Snippet contents (`shell.ts:327-376`) start the wrapper as a **child** and then exit the outer shell:
```sh
# zsh
if [[ -z "${ISTERM}" && $- = *i* && ${+ZSH_EXECUTION_STRING} -eq 0 && -z "${VSCODE_RESOLVING_ENVIRONMENT}" ]]; then
  if [[ -o login ]]; then is -s zsh --login ; exit; else is -s zsh ; exit; fi
fi
# bash
if [[ -z "${ISTERM}" && $- = *i* && $- != *c* && -z "${VSCODE_RESOLVING_ENVIRONMENT}" ]]; then
  shopt -q login_shell; login_shell=$?
  if [ $login_shell -eq 0 ]; then is -s bash --login ; exit; else is -s bash ; exit; fi
fi
# fish
if test -z "$ISTERM" && status --is-interactive && test -z "$VSCODE_RESOLVING_ENVIRONMENT"
  if status --is-login; is -s fish --login ; kill %self; else; is -s fish ; kill %self; end
end
```
```powershell
$__IsCommandFlag = ([Environment]::GetCommandLineArgs() | Where-Object { $_ -imatch '^-c(ommand)?$' }).Count -gt 0
$__IsNoExitFlag = ([Environment]::GetCommandLineArgs() | Where-Object { $_ -imatch '^-noe(xit)?$' }).Count -gt 0
$__IsInteractive = -not $__IsCommandFlag -or ($__IsCommandFlag -and $__IsNoExitFlag)
if ([string]::IsNullOrEmpty($env:ISTERM) -and [Environment]::UserInteractive -and $__IsInteractive -and [string]::IsNullOrEmpty($env:VSCODE_RESOLVING_ENVIRONMENT)) {
  is -s pwsh      # or powershell
  Stop-Process -Id $pid
}
```
- xonsh: `if 'ISTERM' not in ${...} and $XONSH_INTERACTIVE and 'VSCODE_RESOLVING_ENVIRONMENT' not in ${...}:` then `is -s xonsh [--login]; exit`.
- nu: `if "ISTERM" not-in $env and $nu.is-interactive and "VSCODE_RESOLVING_ENVIRONMENT" not-in $env { if $nu.is-login { is -s nu --login ; exit } else { is -s nu ; exit } }`.
- Running `is` while `ISTERM=1` only prints a status confirmation and exits 0 (`ish/src/commands/root.ts:24-30`), which is a second nesting guard.

Profile paths and status (`shell.ts:93-132`):
- Profile paths: bash `~/.bashrc`; zsh `~/.zshrc`; fish `~/.config/fish/config.fish`; xonsh `~/.xonshrc`; pwsh/powershell via `<shell> -c 'echo $profile'` (5 s timeout); nu via `echo $nu.env-path`.
- Doctor: a shell is "without plugin" if the profile does not contain the source command, and "bad plugin" if the profile does not **end** with it (it must be last). Legacy detection: the profile contains "inshellisense shell plugin" or `~/.inshellisense/<shell>/init.`.
- Resources live in `$XDG_DATA_HOME/inshellisense` or `~/.inshellisense` (legacy if that exists), with `shell/`, `init/`, `spec/`, `log/`, `native/` and `version.txt` subdirs. Config is at `$XDG_CONFIG_HOME/inshellisense/rc.toml` (`ish/src/utils/constants.ts:8-53`). `is uninstall` just deletes the resources folder (`ish/src/ui/ui-uninstall.ts`).
- Shell inference order (`shell.ts:185-212`): `NU_VERSION` → nu, `XONSHRC` → xonsh, `FISH_VERSION` → fish, `ZSH_VERSION` → zsh, `BASH_VERSION` → bash; then `$SHELL` basename; then the parent process name.

### 10.4 Edit-buffer extraction in inshellisense (`ish/src/isterm/commandManager.ts`)

This is grid-based, like qterm, but the markers come from PS/PE:
- `handlePromptStart` registers an xterm marker. `handlePromptEnd` registers the command-start marker plus `promptEndX = cursorX` when on the same row (`:56-93`).
- Right-prompt heuristic: if any of the last 5 cells of the prompt line are non-blank, the command ends at the first run of 4 spaces (`rightPromptLookbackWidth=5`, `commandWhitespaceTerminationWidth=4`).
- `_getCommandLines` follows wrapped lines (`isWrapped`, or the cursor being below).
- `_getCommandText` splits cells into preCursor / postCursor / suggestion. Once a cell is a "suggestion", everything after it is suggestion. The scan stops when postCursor ends with 4 spaces (`:182-213`).
- **Ghost-text detection** (`_isSuggestion`, `:112-122`):
  - pwsh/powershell: the cell is dim **or** italic (PSReadLine predictions).
  - Other shells: the fg palette color is 8 or 7, or > 235, or (15 and dim). RGB colors are converted to ansi256 first.
  - This is a heuristic alternative to qac's reported colors.
- `CSI 2J` or `3J` resets the state (`handleClear`). Alternate-screen buffers suspend tracking.
- Windows bits:
  - Backspace is sent as `\x7f` for pwsh, powershell, cmd and nu (`shell.ts:263-264`).
  - The path separator is `/` for bash, xonsh and nu, else the platform separator.
  - `CSI 6n` (DSR) requests from the shell are counted, so the wrapper can answer or forward cursor position reports (ConPTY queries them at startup; `pty.ts:93-100`).
  - The stdin proxy strips win32-input-mode (`CSI ?9001h/l`), kitty keyboard and modifyOtherKeys upgrade requests (`ish/src/ui/stdioProxy.ts:15-17`).

---

## 11. Recommendations for tabsmith (synthesis; all (inferred) design advice)

1. **Pick one protocol.** Use OSC 697 key names (they cover exit code, session lock and preexec) with inshellisense-style value escaping (`\\`, `\x3b`, `\x0a`, `\x1b`, `\x07`) and a parser that splits only at the first `;` after `697`, so paths with `;` survive. Suggested events: `StartPrompt`, `EndPrompt`, `NewCmd=<sid>`, `PreExec`, `OSCLock=<sid>`, `OSCUnlock=<sid>`, `Dir=`, `ExitCode=`, `Shell=`, `ShellPath=`, `PID=`, `TTY=`, `User=`, `<Shell>SuggestionColor=`, `DoneSourcing`. Optionally strip these OSCs before forwarding output to the real terminal.
2. **Keep the dotfile model** (pre at top, post at bottom, the script file in the data dir doing `eval "$(tabsmith init <shell> <when> --rcfile X)"`), including backup, idempotent `is_installed` and exact-marker uninstall. Use `[ -f ... ] && . ...` in `.profile`. Drop the aggressive `fig.*var` comment regex.
3. **Launch guards**, in one Rust function returning 0/1/2:
   - Force var and disable var.
   - `TABSMITH_TERM` already set. In tmux, also check `TABSMITH_TERM_TMUX`.
   - stdout is not a TTY; the shell is non-interactive or `-c`.
   - `INSIDE_EMACS`; `TERM_PROGRAM=WarpTerminal`; `VSCODE_RESOLVING_ENVIRONMENT` (VSCode/Kiro/Cursor env probing); SecureCRT; `__PWSH_LOGIN_CHECKED`; `CI`; SSH without opt-in; `TERM=dumb` (not in qac; recommended).
   - Optionally a `TERM_PROGRAM` deny-list (`kiro`/`vscode`) configurable via a setting.
   - Always fall back to `exec`-ing the plain shell if the wrapper fails.
4. **Windows.** qac has no PowerShell or cmd integration. Port inshellisense's `shellIntegration.ps1` (Prompt function wrapping) and the cmd `PROMPT` env trick, adding the qac events (exit code via `$?`/`$LASTEXITCODE`; preexec via PSReadLine `AcceptLine` handler or `PSConsoleHostReadLine` override, inferred). Install by appending the source line to `$PROFILE` (`pwsh -NoProfile -c '$PROFILE'`). Use ConPTY via `portable-pty`/`conpty` and apply the "delay EndPrompt/NewCmd until the next read" logic from §6.1.
5. **zsh.** Ship the zsh-autosuggestions-derived plugin unchanged except for renaming. Keep the "real zsh-autosuggestions loaded → disable ours" guard and the per-precmd rebind. Consider inshellisense's `zle-line-init` PS/PE emission as an extra precise command-start marker.
6. **nushell.** Rewrite using `$env.config.hooks.pre_prompt`/`pre_execution` and closure prompts (ish §10.2). Do not reuse `post.nu`/`pre.nu`.
7. **Ghost-text exclusion.** Prefer reported colors (qac), but fall back to ish's dim/italic/gray heuristic for shells that report nothing (pwsh, bash with ble.sh, etc.).
