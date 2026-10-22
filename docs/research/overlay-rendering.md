# In-terminal overlay rendering & cross-platform PTY handling

Research for **tabsmith** (Rust re-implementation of Kiro CLI autocompletion). Main subject: how
microsoft/inshellisense ("ish") draws an autocomplete dropdown *inside* the terminal through a PTY
wrapper. Contrasts with Fig/Kiro `figterm` (Rust) where relevant. Ends with a concrete Rust design.

Citation roots (read-only):

- `ish/` = `.../scratchpad/ref/ish` (inshellisense, `package.json:3` version `0.0.4`, HEAD `9ba441d`)
- `qac/` = `.../scratchpad/ref/qac` (amazon-q-developer-cli-autocomplete)
- `ansi-escapes` / `ansi-styles` npm sources were checked from local copies outside the repo
  (ansi-escapes 4.3.2 and 7.3.0, ansi-styles 6.x). ish pins `ansi-escapes ^6.2.0`. The sequences
  quoted below are the same in all of those versions.

Anything marked **(inferred)** is my reasoning or memory, not something read in the code.

---

## 0. TL;DR

1. ish spawns the shell in a PTY (node-pty; on Windows ConPTY with the **bundled** `conpty.dll`).
   It feeds **every** output byte into two places: an **xterm.js headless** emulator (the model)
   and the real terminal, almost unchanged.
2. Shell integration is a private OSC: `ESC ] 6973 ; PS BEL` (prompt start), `...;PE BEL`
   (prompt end) and `...;CWD;<escaped path> BEL`. The command line is **read back from the
   headless screen**: it is the cells after the prompt-end marker up to the cursor's line. Gray,
   dim or italic cells are classified as the shell's own autosuggestion (ghost text).
3. The menu is drawn as a set of **line patches**. For each of the next N rows below (or above)
   the cursor, ish erases the line (`CSI 2K`) and **re-emits that row from the headless buffer**,
   with SGR rebuilt per cell. The box text is spliced in at column `startX`. The cursor is saved
   and restored around the drawing (`CSI s`/`CSI u`, or `ESC 7`/`ESC 8` on JetBrains and Apple
   Terminal). The cursor is hidden while drawing.
4. **Restoring the screen when the menu hides is the same operation with no splice.** ish re-emits
   the rows from the headless buffer with their original attributes. It keeps no saved copy of
   what was under the menu.
5. It never scrolls the terminal to make room. If there is no room below, it draws above. If
   neither fits, it does not show the menu.
6. Input: stdin is put in raw mode and decoded into keypress events (Node readline). Tab, Up,
   Down and Esc are swallowed **only while the menu is visible**; every other key is forwarded
   byte for byte. Accepting a suggestion writes `DEL (0x7f) × tokenWidth + text` to the PTY.
7. Resize: forward the new size to the PTY and the model, wait 150 ms (debounced), then clear the
   real screen and repaint the whole viewport from the model. In alternate-screen apps (vim,
   less) the overlay is turned off and all bytes pass straight through.

---

## 1. Architecture and data flow

```
 keyboard ──stdin(raw)──► StdioProxy ──keypress──► SuggestionManager.update()
                              │ CPR replies             │ handled?  no → term.write(seq) ─┐
                              └──► (if requested) ──────┼──────────────────────────────────┤
                                                        │ yes → term.noop() (re-render)    ▼
                                                                                    node-pty / ConPTY
 shell ◄────────────────────────────────────────────────────────────────────────────  (child)
   │ output bytes
   ▼
 ISTerm.onData ──► xterm-headless.write(data, cb) ──(parse; OSC/CSI hooks fire)──► cb:
                     ├─ cursorHidden / cursorShift bookkeeping
                     ├─ CommandManager.termSync()   (re-read command line from model)
                     └─ emit "data"
 ui-root onData ─► StdioProxy.handleOutput (strip kbd-protocol upgrades, LF→IND)
                ─► SuggestionRenderer.renderPtyData(data)   → stdout (data + overlay patches)
                ─► if command-state version changed → BatchScheduler.request()
                         └► SuggestionManager.exec() → renderer.renderSuggestionUpdate()
```

Wiring: `ish/src/ui/ui-root.ts:23-113`, `ish/src/isterm/pty.ts:108-132`.

Key design point: the bytes that reach the real terminal are **the original PTY bytes**. Lightly
filtered, but not re-rendered. The headless model is used only (a) to read the command line and
(b) as the source for the line patches that draw and erase the overlay. The model and the real
terminal stay in sync because they parse the same stream from the same starting state. That is why
ish **clears the real screen at startup** (`ui-root.ts:42`, `ansi.clearTerminal`), so both begin
empty with the cursor at (0,0).

---

## 2. Spawning the shell

### 2.1 node-pty options (`ish/src/isterm/pty.ts:75-83`)

```ts
pty.spawn(shellTarget, shellArgs ?? [], {
  name: "xterm-256color",   // TERM on unix
  cols, rows,               // process.stdout.columns/rows (ui-root.ts:25)
  cwd: process.cwd(),
  env: { ...convertToPtyEnv(shell, underTest, login), ...env },
  useConpty: true,          // Windows: ConPTY (not winpty)
  useConptyDll: true,       // Windows: use node-pty's bundled conpty.dll + OpenConsole.exe, not kernel32's inbox ConPTY
});
```

The headless model is created with the same size: `new xterm.Terminal({allowProposedApi:true, rows, cols})`
plus the Unicode 11 width addon (`pty.ts:89-92`). Scrollback is xterm's default (1000 lines, **(inferred)**).

Before spawning, ish checks that the shell exists (`fs.existsSync(target) || which(target)`).
Otherwise it fails with `shell not found on PATH: <target>` (`pty.ts:418-430`).

### 2.2 Per-shell target, args and env (`pty.ts:432-508`, `shell.ts:29-55`)

Supported shells (`shell.ts:40-49`): `bash, pwsh, zsh, fish, xonsh, nu` everywhere. Windows also
has `powershell` and `cmd`. `init` supports all of these except `cmd` (`shell.ts:51`).

Target binary: on Windows it is `<shell>.exe`, except bash, which is resolved to **Git Bash**
(`shell.ts:218-261`). The search order is: `git.exe` on PATH → `../../`, then `%ProgramW6432%`,
`%ProgramFiles%`, `%ProgramFiles(X86)%` and `%LocalAppData%\Program`, each with `\Git\bin\bash.exe`,
`\Git\usr\bin\bash.exe` and `\usr\bin\bash.exe`. Scoop installs are also checked:
`%UserProfile%\scoop\apps\git[-with-openssh]\current\bin\bash.exe`. On other platforms the target
is the bare shell name.

| shell | args | env additions | integration file |
|---|---|---|---|
| bash | `--init-file <res>/shell/shellIntegration.bash` | — | sources `~/.bashrc`, or the login files if `ISTERM_LOGIN` (`shellIntegration.bash:1-17`) |
| zsh | none | `ZDOTDIR=<tmp>/is-zsh-<pid>`, `USER_ZDOTDIR=$ZDOTDIR or $HOME` (`pty.ts:502-504`, `shell.ts:54-55`) | temp ZDOTDIR gets `.zshrc/.zprofile/.zshenv/.zlogin` copies (`shell.ts:165-175`), deleted on exit |
| fish | `--init-command ". <path>"` (spaces escaped); Windows: `". \"$(cygpath -u '<path>')\""` | — | `shellIntegration.fish` |
| pwsh / powershell | `-noexit -command "try { . \"<ps1>\" } catch {}"` | — | `shellIntegration.ps1` |
| nu | `-e "source \`<nu>\`"` (+ `-n` under test) | — | `shellIntegration.nu` |
| xonsh | `--rc <existing of: shared, ~/.xonshrc, ~/.config/xonsh/rc.xsh, ~/.config/xonsh/rc.d> <xsh>` | — | `shellIntegration.xsh`. Bug: unix shared path is `path.join("etc",...)`, which is relative (`pty.ts:452`) |
| cmd | none | `PROMPT=<OSC PS>` + (`%PROMPT%` or `$P$G`) + `<OSC PE>` (`pty.ts:495-501`) | none, prompt only. **No CWD reporting** |

All shells get the env `ISTERM=1` (used for nesting detection), plus `ISTERM_TESTING=1` with `-T`
and `ISTERM_LOGIN=1` with `-l` (`pty.ts:486-493`).

Login (`-l`): pwsh/powershell get `-login` prepended. zsh, fish, xonsh and nu get `--login`. bash
gets no flag and relies on `ISTERM_LOGIN` inside the init file (`pty.ts:468-481`).

Resources (the integration scripts) are unpacked to `<res>/shell`. `<res>` is `~/.inshellisense`
when that legacy folder exists (and always on Windows); otherwise it is
`$XDG_DATA_HOME/inshellisense`, defaulting to `~/.local/share/inshellisense`
(`ish/src/utils/constants.ts:10-53`). If `<res>/version.txt` does not match the binary version,
startup aborts with `resources out of date, run "is reinit"` (`commands/root.ts:32-38`,
`utils/node.ts:122-129`).

### 2.3 Auto-start plugin (profile snippet), for reference

`is init <shell>` prints a source line (`shell.ts:300-325`). The generated init file starts `is`
only when `ISTERM` is empty, the shell is interactive and `VSCODE_RESOLVING_ENVIRONMENT` is unset
(`shell.ts:327-376`). Example for zsh:
`if [[ -z "${ISTERM}" && $- = *i* && ${+ZSH_EXECUTION_STRING} -eq 0 ...]]; then is -s zsh [--login]; exit; fi`.
Running `is` inside a session (`ISTERM=1`) prints `inshellisense session [live]` and exits
(`commands/root.ts:24-30`).

---

## 3. Shell-integration escape sequences

Defined in `ish/src/utils/ansi.ts:11-21`:

```
OSC  = ESC ]   (0x1b 0x5d)        BEL = 0x07
PromptStart : ESC ] 6973 ; PS BEL
PromptEnd   : ESC ] 6973 ; PE BEL
Cwd         : ESC ] 6973 ; CWD ; <escaped-path> BEL
```

- They are registered as an xterm OSC handler with id `6973` (`pty.ts:103`). The handler returns
  `true` (consumed) for PS, PE and CWD, and `false` otherwise (`pty.ts:177-198`).
- **The OSCs are not stripped from the stream that reaches the real terminal.** Real terminals
  ignore unknown OSCs.
- CWD escaping, as done by the shell scripts: `\` → `\\`, `;` → `\x3b`, LF → `\x0a`, ESC → `\x1b`,
  BEL → `\x07`. bash and zsh work byte by byte with `LC_ALL=C` (`shellIntegration.bash:37-63`).
  pwsh hex-escapes those characters as UTF-8 bytes (`shellIntegration.ps1:10-16`).
- Decoding uses the regex `/\\(\\|x([0-9a-f]{2}))/gi` and replaces each match with the char or the
  hex byte (`pty.ts:158-160`). The result is then sanitized (`pty.ts:162-175`): surrounding quotes
  are removed. On Windows, Git-Bash `/c/...` becomes `C:\...` and the drive letter is uppercased.
  Finally `path.resolve`.

Where each shell emits the markers:

| shell | PS | PE | CWD |
|---|---|---|---|
| bash | `PS1="\[$(PS)\]$PS1\[$(PE)\]"`, re-wrapped in `precmd` whenever `PS1` changes. Uses bash-preexec if present, otherwise `PROMPT_COMMAND` chaining (`shellIntegration.bash:69-120`) | end of PS1 | `precmd` |
| zsh | `zle-line-init` widget: `PS`, then the original widget, then `PE`. **Both fire after the prompt is drawn**, so `promptEndX` = cursor X (`shellIntegration-rc.zsh:57-70`). Emitted to `/dev/tty` | same widget | `precmd` hook |
| fish | wraps `fish_prompt` (old one copied to `is_user_prompt`): `PS + prompt + PE` (`shellIntegration.fish:1-29`) | end of prompt | `--on-event fish_prompt` |
| pwsh | `function Prompt { PS + original + PE + CWD }` (`shellIntegration.ps1:18-26`) | before CWD | in prompt, FileSystem provider only |
| nu | `PROMPT_COMMAND` = `PS + original + CWD` | `PROMPT_INDICATOR` = `indicator + PE` (`shellIntegration.nu:9-29`) | in PROMPT_COMMAND |
| xonsh | `$PROMPT = "{PS}{CWD}" + $PROMPT + "{PE}"`, using `\001`/`\002` non-printing guards (`shellIntegration.xsh:4-32`) | end | in prompt |
| cmd | `PROMPT` env var (`pty.ts:495-501`) | end | none |

Compare with Fig/Kiro: `figterm` uses `OSC 697 ; NewCmd|StartPrompt|EndPrompt|PreExec ...` in a
forked alacritty_terminal (`qac/crates/alacritty_terminal/src/ansi.rs:470-481, 1175-1181`).

---

## 4. Command-line tracking (`ish/src/isterm/commandManager.ts`)

State exposed to the UI (`commandManager.ts:20-26`):
`{ promptText?, commandText?, suggestionsText?, hasOutput?, cursorTerminated? }` plus a
monotonically increasing `stateVersion`. The version is bumped only when one of those five fields
changes (`:132-152`).

Constants: `rightPromptLookbackWidth = 5`, `commandWhitespaceTerminationWidth = 4` (`:10-11`).

### 4.1 Events

- **PS** (`handlePromptStart`, `:56-60`): ignored while on the alternate screen. Otherwise
  `activeCommand = { promptStartMarker: registerMarker(0) (cursor line), hasOutput:false, cursorTerminated:false }`.
- **PE** (`handlePromptEnd`, `:62-93`):
  1. For **nu and xonsh only** (`getShellPromptRewrites`, `shell.ts:287-289`): if the
     prompt-start line is in `acceptedCommandLines`, this is the shell re-drawing an already
     accepted command. ish resets `activeCommand = {}` and stops.
  2. `promptEndMarker = registerMarker(0)`. If the marker line equals the cursor line,
     `promptEndX = cursorX`.
  3. `promptText = promptLine.translateToString(true)` (right-trimmed).
  4. **Right-prompt heuristic**: if any of the last 5 cells of the prompt line is non-blank, find
     the first run of 4 spaces in `promptText`. If found, `promptText = text[0..idx+1]` and
     `promptEndX = idx+1`.
- **Enter / Ctrl-C** (`clearActiveCommand`, `:154-159`, called from `SuggestionManager.update`):
  adds the prompt-end line to `acceptedCommandLines` and sets `activeCommand = {}`. Until the next
  PS there are no markers, so no command text and no suggestions while a program runs.
- **`CSI 2J` / `CSI 3J`** (registered CSI `J` handler, `:49-54`): `handleClear()` = `handlePromptStart()`
  plus clearing `acceptedCommandLines`. The handler returns `false` so xterm still performs the
  erase.
- **Alternate buffer**: `onBufferChange` sets `suspended = (type === "alternate")` (`:44-47`).
  While suspended the public state is `{}` (`:133-134`). The test
  `ish/src/tests/isterm/commandManager.test.ts:31-61` shows the normal-buffer command coming back
  after `CSI ?1049l`.

### 4.2 `termSync()`, run after every PTY chunk is parsed (`:231-278`)

1. If suspended, sync the snapshot and return. If either marker is missing, return.
2. If a marker was disposed (trimmed out of scrollback), **re-register both at the cursor line**.
3. If `promptText != null`:
   - `commandLines` (`:161-180`): start at the prompt-end line. Keep adding the next line while it
     `isWrapped` **or** the cursor is further down (`absCursorY > lineY-1`). Stop at the bottom of
     the viewport. Multi-line edits are therefore covered as long as the cursor is on or below them.
   - `hasOutput` (`:215-229`): the line right after the command lines (if inside the viewport) has
     any non-empty cell.
   - If not `persistentOutput`, parse the text (`:182-213`):
     - First line starts at `promptEndX ?? promptText.length`, later lines at 0. Go up to `cols`.
     - Stop when `postCursorCommand` ends with 4 spaces (this avoids capturing right prompts).
     - An empty cell (`getChars()==""`) becomes a space.
     - `beforeCursor = y < cursorLine || (y == cursorLine && x < cursorX)`.
     - A cell counts as command text if `!isSuggestion(cell)` and no suggestion cell has been seen
       yet. Once one ghost cell is seen, **everything after it is suggestion**.
     - `commandText = pre + post.trim()`, `cursorTerminated = post.trim() == ""`,
       `suggestionsText = ghost text`.
   - `persistentOutput = prev.hasOutput && hasOutput`. `hasOutput = hasOutput`.

### 4.3 Ghost-text (autosuggestion) detection, `_isSuggestion` (`:106-122`)

```
fgIndex = default → 0 ; palette → index ; RGB → color-convert hex→ansi256
dull     = fg==8 || fg==7 || fg>235 || (fg==15 && dim)
pwsh / powershell : isSuggestion = dim || italic       (PSReadLine predictions)
all other shells  : isSuggestion = dull                (zsh-autosuggestions, fish, nu, ...)
```

Expected results are in `ish/src/tests/isterm/__snapshots__/pty.test.ts.snap`. For fish, `l` with
ghost text gives `suggestionsText:"s -la"`. For pwsh, `ls` gives `"-Force"`.

### 4.4 Fallback when there is no shell integration

**There is none in this checkout.** No marker means no prompt, which means no suggestions.
cmd.exe gets "integration" only through the injected `PROMPT` variable.

**(inferred, from memory of older ish releases, not in this tree):** earlier versions scanned up
to ~10 lines around the cursor with regexes on Windows to find prompts:
pwsh `/(?<prompt>(\(.+\)\s)?(?:PS.+>\s?))/`, cmd `/^(?<prompt>(\(.+\)\s)?(?:[A-Z]:\\.*>))/`. They
did this because the old inbox ConPTY did not place OSCs reliably. The heuristics were dropped once
ish switched to the bundled `conpty.dll` (`useConptyDll: true`). Fig solved the same problem
differently. On Windows it **delays EndPrompt/NewCmd** until a later PTY read brings no new
delayed events (`qac/crates/figterm/src/main.rs:583-584, 859-869`;
`qac/crates/alacritty_terminal/src/term/mod.rs:796-880`).

---

## 5. Cursor tracking and terminal queries

`getCursorState(buffer="active")` (`pty.ts:251-261`):

```
onLastLine     = cursorY >= rows - 2
remainingLines = max(rows - 2 - cursorY, 0)      // the last row is never used below
cursorX, cursorY (viewport-relative, from xterm buffer)
hidden         = ISTerm.cursorHidden
shift          = ISTerm.cursorShift
```

- `cursorHidden` is updated after each chunk by **substring search**: contains `CSI ?25l` → true,
  then contains `CSI ?25h` → false (`pty.ts:112-117`). If a chunk contains both, the result is
  "shown", whatever the order.
- `cursorShift` = how far the cursor moved **down** during this chunk (0 if it did not move down)
  (`pty.ts:110, 118-119`). Any non-zero shift hides the menu for that frame (e.g. Enter, output).
- **DSR/CPR** (`CSI 6n` and `CSI ?6n`): a CSI `n` handler counts pending cursor-position queries
  (`pty.ts:93-100`) and returns `false`, so the query still reaches the real terminal. The real
  terminal answers on stdin as `ESC [ ?row;col R`. `StdioProxy` pulls those replies out of the
  input stream (`stdioProxy.ts:12-14, 97-105`, including replies split across chunks). They are
  written to the PTY only if a query is pending (`ui-root.ts:33-39`, `pty.ts:241-245`), otherwise
  dropped. The headless emulator's own replies are never wired to the PTY. Test:
  `ish/src/tests/utils/stdioProxy.test.ts:19-45`.

---

## 6. Menu layout

### 6.1 Constants (`ish/src/ui/suggestionManager.ts:18-24`)

```
maxSuggestions   = config.maxSuggestions ?? 5
suggestionWidth  = 40        // whole suggestion box incl. borders
descriptionWidth = 30        // whole description box incl. borders
descriptionHeight= 5         // max wrapped description lines
borderWidth      = 2
maxLines         = 2 + max(maxSuggestions, 5) + 1 = 8 by default   // "+1: unhandled newline at end of command"
MIN_WIDTH        = 32 (exported, unused)
```

### 6.2 Vertical: direction and rows (`suggestionRenderer.ts:110-116`)

```
direction = remaining >= maxLines ? "below"
          : cursorY   >= maxLines ? "above"
          : remaining >= cursorY  ? "below" : "above"
lines     = direction=="above" ? min(maxLines, cursorY) : min(maxLines, remaining)
```

`remaining = max(rows-2-cursorY, 0)`.

**There is no "scroll the terminal to make room".** If the boxes need more rows than `lines` (5
suggestions = 7 rows), `fitsViewport` is false and nothing is drawn (`suggestionRenderer.ts:137-149`).
The e2e test "ui on bottom of the screen" (`ish/src/tests/ui/autocomplete.test.ts`) relies on the
flip to "above".

The overlay always paints exactly `lines` rows. Rows without a box segment are restored from the
model, which wipes stale menu rows.

### 6.3 Horizontal: anchor at the cursor column, clamp, swap (`suggestionManager.ts:157-171, 177-220`)

```
wrapped     = cursorX % cols                        // menu's left edge = cursor column (not token start)
maxPadding  = cols - 40 - (hasDescription ? 30 : 0)
swap        = wrapped > maxPadding && hasDescription        // description box moves to the LEFT
swapped     = swap ? max(wrapped - 30, 0) : wrapped
padding     = min(wrapped, swapped, maxPadding)             // may go negative on narrow terminals
```

`hasDescription` uses the active suggestion's description, falling back to the argument's
description.

Each output row is a patch `{startX, length, data}`:
- `data` = suggestion row + description row (reversed if swapped).
- `length` = (40 if the suggestion row exists) + (30 if the description row exists).
- `startX` = `padding`. Exception: when the left-hand box has no row at this height, the row is
  shifted by that box's width, so the other box stays aligned. If not swapped and the suggestion
  row is missing: `padding+40`. If swapped and the description row is missing: `padding+30`.

The two boxes are **top-aligned** below the cursor and **bottom-aligned** above it, so both touch
the cursor line: `suggestionUI[i - maxRows + suggestionUI.length]` (`:204-209`).

Fit check (`suggestionRenderer.ts:137`): `patches.length <= lines` and every
`startX >= 0 && startX+length <= cols`. With a description that needs `cols >= 70`. Without one,
`cols >= 40`. If the check fails the menu is hidden (snapshots of the 40- and 21-column views:
`ish/src/tests/ui/__snapshots__/autocomplete.test.ts.snap`).

Worked example from that snapshot (80×30, prompt `> `, typed `git st`, cursorX = 8):
`maxPadding = 10`, no swap, `padding = 8`.

```
> git st
        ┌──────────────────────────────────────┐┌────────────────────────────┐
        │📦 stage                              ││Add file contents to the    │
        │📦 status                             ││staging area                │
        │📦 stash                              │└────────────────────────────┘
        └──────────────────────────────────────┘
```

### 6.4 Paging and selection (`suggestionManager.ts:183-192, 246-249`)

```
page        = min(floor(idx/max)+1, floor(len/max)+1)
visible     = suggestions[(page-1)*max .. page*max)
activeOnPage= idx % max
up          : idx = max(0, idx-1)                // no wrap-around
down        : idx = min(idx+1, len-1)
```

This is page-flip scrolling, not a sliding window. A new suggestion list resets `idx = 0`.

### 6.5 Box content (`ish/src/ui/utils.ts`)

- `renderBox(rows, width)` (`utils.ts:15-25`):
  `CSI0m + "┌" + "─"*(w-2) + "┐"`, then each row `CSI0m + "│" + row + CSI0m + "│"`, then
  `CSI0m + "└" + ... + "┘"`. With `boxBorderStyle="rounded"` the corners are `╭╮╰╯`. No border
  color is passed in this version.
- Suggestion row: `${icon} ${name}` passed through `truncateText(text, 38)`. The active row is
  wrapped in `chalk.bgHex(activeSuggestionBackgroundColor)`, default `#7D56F4`. That gives
  `CSI 48;2;125;86;244m … CSI 49m` on truecolor terminals. chalk drops to 256 or 16 colors
  according to `supports-color` **(inferred)**.
- `truncateText(text, w)` (`utils.ts:42-63`): accumulate display width (`wcswidth`, a Unicode 11
  fork, `ish/src/utils/unicode.ts:483-547`). On overflow keep points that fit in `w-1` and add `…`.
  Pad with spaces to exactly `w` columns (`美国人美国人` at 10 → `美国人美… `).
- `truncateMultilineText(desc, 28, 5)` (`utils.ts:27-38`): `wrap-ansi` with `{hard:true, trim:false}`
  and max 5 lines. If longer, the last char of line 5 is replaced by `…`. Each line is padded to
  28 by display width.
- Argument-only state (no suggestions but the argument has a description): a 1-row box, 30 wide
  (`suggestionManager.ts:136-139, 194-198`).
- Icons (`ish/src/runtime/suggestion.ts:16-26, 63-97`): file 📄, folder 📁, subcommand 📦,
  option 🔗, arg 💲, mixin 🏝️, shortcut 🔥, special ⭐, default 📀. If the spec icon is non-Latin-1
  it is used as is. `fig://icon?type=X` maps to a Nerd Font glyph when `useNerdFont`. Emoji width
  differences between terminals can misalign the box **(inferred risk)**. 🏝️ includes VS16.

---

## 7. Drawing: exact byte sequences

### 7.1 Save/restore choice (`suggestionRenderer.ts:49`, `ansi.ts:9`, `dec.ts`)

```
default (ansi-escapes): save = CSI s  (ESC [ s)   restore = CSI u      — on Apple_Terminal ansi-escapes itself uses ESC 7 / ESC 8
TERMINAL_EMULATOR=="JetBrains-JediTerm": save = ESC 7 (DECSC)  restore = ESC 8 (DECRC)
```

Other primitives (ansi-escapes and `ish/src/utils/ansi.ts`): hide `CSI ?25l`, show `CSI ?25h`,
next line `CSI E`, previous line `CSI F`, erase line `CSI 2K`, reset SGR `CSI 0m`, cursor forward
`CSI <n> C`. Two different `cursorTo` helpers exist: ish's `cursorTo({x,y})` = `CSI y;x H`
(1-based, as given), and ansi-escapes' `cursorTo(x,y)` = `CSI (y+1);(x+1) H` (0-based).

### 7.2 Frame layout (`suggestionRenderer.ts:175-186`)

```
below: CSI?25l  SAVE  CSI E  <patch>  RESTORE  [CSI?25h]  <pty-data>
above: <pty-data>  CSI?25l  SAVE  (CSI F × lines)  <patch>  RESTORE  [CSI?25h]
```

- `CSI ?25h` is left out when the child app has hidden the cursor.
- With "below", the overlay is written **before** the new PTY data. With "above", after it.
  **(inferred)** reason: a chunk that scrolls the screen, such as Enter plus output, first erases
  the rows below so they cannot scroll into scrollback with the menu still painted. The patch is
  computed from the post-chunk model state. For "below" it is drawn relative to the pre-chunk real
  cursor. When the chunk moved the cursor (`cursorShift > 0`), the menu is hidden anyway and the
  patch only restores (mostly blank) rows, which the chunk's own output then overwrites.

### 7.3 `<patch>` = `ISTerm.getPatch(height, patches, direction)` (`pty.ts:384-415`)

```
cur = baseY + cursorY                                   // absolute buffer line
below: rows y = cur+1 .. cur+height, patch index 0..          joined with CSI E
above: rows y = cur-1 down to cur-height, patches from last;  reversed to top-down, joined with CSI E
```

Each row is `_getLinePatch(buffer, y, patch?)` (`pty.ts:334-369`):

```
if y outside viewport → ""            (the CSI E join still advances)
pre  = [CSI0m, CSI2K]                 // erase whole line first
post = patch ? [CSI0m] : []
for x in 0..line.length:
    if patch && startX <= x < startX+length: prev=undefined; target=post; continue
    cell = line.getCell(x)
    if !sameColor(prev,cell) || !sameAccent(prev,cell): target.push(CSI0m)
    if !sameColor: target.push(colorsSGR(cell))
    if !sameAccent: target.push(accentSGR(cell))
    target.push( chars=="" ? (prev.width==2 && cell.width==0 ? "" : CSI 1 C) : chars )
    prev = cell
return pre.join + (patch?.data ?? "") + post.join
```

- No absolute column positioning: the cursor reaches `startX` because every cell before it is
  re-emitted, and empty (never-written) cells become `CSI 1 C`.
- The second half of a wide character emits nothing.
- A full line of blanks therefore costs `cols × "ESC[1C"`. This is wasteful (§12).

SGR reconstruction (`pty.ts:263-328`):

```
colors:  bg default→CSI49m | palette→CSI48;5;Nm | RGB→CSI48;5;hexToAnsi256(hex)m
         fg default→CSI39m | palette→CSI38;5;Nm | RGB→CSI38;5;hexToAnsi256(hex)m
accents (in this order): bold 1, italic 3, underline 4 or 4:<style> (extended), inverse 7,
         dim 2, blink 5, invisible 8, strikethrough 9
```

**Fidelity bugs** to avoid in Rust:
1. Truecolor is downgraded to 256 colors.
2. `getFgColor().toString(16)` is not zero-padded, and ansi-styles' `hexToRgb` matches
   `/[a-f\d]{6}|[a-f\d]{3}/`. So `0x00ff00` → `"ff00"` → matches `ff0` → **yellow**, and
   `0x0000ff` → `"ff"` → no match → black.
3. Underline color, overline and OSC 8 hyperlinks are lost.

### 7.4 Viewport repaint (resize) (`suggestionRenderer.ts:233-237`, `pty.ts:375-382`)

```
CSI?25l  CSI2J  CSI1;1H  { for r in 0..rows: CSI (r+1);1 H + linePatch(row r) }
CSI (cy+1);(cx+1) H  (CSI?25l if app hid cursor else CSI?25h)
```

The overlay is then rendered on top.

---

## 8. Restoring what is under the overlay (the crucial part)

**ish never stores the pixels it covers.** Hiding, shrinking, moving or changing the menu all go
through the same mechanism: re-render the affected rows from the xterm-headless model, re-emitting
each cell's character and attributes (rebuilt SGR, §7.3), after `CSI 2K`.

- **Normal change** (`#render`, `suggestionRenderer.ts:127-173`): every frame repaints all `lines`
  rows in the current direction. Rows with a box segment are spliced. Rows without one are fully
  restored. A shrinking list therefore cleans itself up.
- **Cached clear** (`cache.clear`): whenever a menu is shown, ish also precomputes the "restore all
  rows" string from the model state at that moment (`:167, 188-201`). It is written:
  - when the next frame cannot fit, or has nothing to show while a menu was visible (`:139-142`),
    **before** the new data;
  - when entering the alternate screen (`#clearNormalBufferSuggestions`, `:209-213`), before the
    data that switches screens reaches the real terminal. The buffer-change event fires during the
    model parse, before ui-root writes the chunk.
- **Direction flip** while visible (`:90-93, 203-207`): restore the rows on the *previous* side,
  then render the new side.
- **Escape / dismiss**: `suggestBlob = undefined` → the next render has 0 patches → restore rows.

Correctness depends on two things: the model matching the real terminal, and every overlay byte
being undone by a model-based repaint. Divergence sources: model/terminal width tables (ish uses
Unicode 11 in both model and box math), unsupported sequences, and the SGR bugs above.

---

## 9. Render decision logic

`renderPtyData(data, backspaceEchoPending)` (`suggestionRenderer.ts:70-95`):

```
if alternate buffer        → write(data); return
if resize repaint pending  → write(data); re-arm 150 ms timer; return
if skipNextPtyRender       → write(data); skip=false; cache.patch=undefined; return   // first chunk after leaving alt screen
if visible && direction changed → clear previous direction
render(data, ...)
```

`#render` (`:127-173`):

```
patches = manager.render(direction, cursorX)
fits    = patches.len <= lines && all(startX>=0 && startX+len<=cols)
if !fits || (!wasVisible && patches.empty):
    if wasVisible && cache.clear: write(cache.clear)
    cache = {patch:"none"}; write(data); visible=false; return
terminated = backspaceEchoPending || command.cursorTerminated
show = patches.nonEmpty && fits && terminated && !command.hasOutput && cursorShift==0 && command.commandText
key  = `${dir}:${lines}:${cursorHidden}:` + join(show ? patches : [], "\0" , p => `${startX}:${length}:${data}`)
if data=="" && wasVisible==show && cache.patch==key: return              // nothing changed
write(frame(data, getPatch(lines, show?patches:[], dir)))
cache.patch = key; cache.clear = show ? clearOutput(dir) : undefined; visible = show
```

`backspaceEchoPending` is set when Backspace is forwarded and reset after the next PTY chunk
(`ui-root.ts:29, 58-79, 92-94`). While it is set, a visible menu stays up even if the echo
temporarily looks like "cursor not at end of line". **(inferred)**: shells echo `\b \b` or
`\b CSI K` in pieces, which would otherwise flicker the menu.

---

## 10. Resize

`process.stdout.on("resize")` → `renderer.resize(cols, rows)` (`ui-root.ts:109-111`,
`suggestionRenderer.ts:102-108`):

1. `resizePending = true`, cache reset, `visible = false`.
2. `ISTerm.resize`: `pty.resize(cols, rows)` **and** `xterm.resize(cols, rows)`. xterm reflows the
   normal buffer (`pty.ts:204-209`).
3. Debounce 150 ms (`:220-231`). PTY data arriving meanwhile passes through raw and re-arms the
   timer (`:76-80`). This covers the shell's SIGWINCH redraw and ConPTY's repaint.
4. When the timer fires (unless on the alternate screen): full viewport repaint from the model (§7.4),
   then `#render("")`. On the alternate screen the repaint is postponed until returning to the
   normal buffer (`:64-65`).

E2E test `resize recovery` (`ish/src/tests/ui/autocomplete.test.ts`): 80→40→21→80 columns, and
the final screen equals the initial one.

## 11. Alternate-screen apps (vim, less, htop)

Detection is xterm's buffer type (`CSI ?1049h/?1047h/?47h`); `ISTerm.onBufferChange` → renderer and
command manager.

- **Enter alt**: write the cached clear for the normal screen, `suggestions.suspend()` (abort the
  in-flight request, drop the blob, clear transient runtime state), `visible=false`
  (`suggestionRenderer.ts:57-62`; tests `suggestionManagerAlternateBuffer.test.ts`).
- **While in alt**: output passes through untouched (`:71-74`). Every keypress is forwarded
  directly, with backspace mapping (`ui-root.ts:83-86`). Command tracking is suspended (§4.1).
- **Leave alt**: `skipNextPtyRender = true`, recompute direction, run a pending resize repaint
  (`:62-66`).
- Apps that redraw inline without the alternate screen (ink CLIs, progress bars) are not
  special-cased. A suggestion only appears with an active prompt, so there are none while such a
  program runs (Enter cleared the command). The direction-flip clear only happens when a menu is
  visible, precisely for these apps (`:90`, test `suggestionRenderer.test.ts:71-94`).

---

## 12. Input: interception vs forwarding

### 12.1 Pipeline

1. `process.stdin.setRawMode(true)` (the original raw state is restored on exit) (`ui-root.ts:31-32, 104`).
2. `StdioProxy.handleInput` (`stdioProxy.ts:68-72, 97-105`): streaming UTF-8 decode, CPR
   extraction (keeping an incomplete `ESC[...` tail for the next chunk), then the remainder goes
   into a PassThrough with `readline.emitKeypressEvents`, which produces `{name, sequence, ctrl, shift}`.
   Node readline names: `return` (`\r`), `enter` (`\n`), `tab`, `backspace` (0x7f/0x08), `escape`,
   `up/down/left/right` (`CSI A..D` / `SS3 A..D`), shift+tab = `{name:"tab", shift:true}` (`CSI Z`).
   A lone ESC is decided after a ~500 ms timeout **(inferred: Node ESCAPE_CODE_TIMEOUT)**.
3. `stdio.onKeypress` (`ui-root.ts:81-99`):

```
if alt screen: write(name=="backspace" ? backspaceSeq(shell) : sequence); return
handled = suggestions.update(press, allowBindings = renderer.visible)
if visible && handled: term.noop()                 // emits "" data → re-render with new state
elif !handled: write(name=="backspace" ? (pendingEcho=true, backspaceSeq) : sequence)
```

`SuggestionManager.update` (`suggestionManager.ts:222-268`):

```
if name=="return": term.clearCommand()                       // always, even with no menu
if hideSuggestions: hideSuggestions = (name=="up"||name=="down")   // Esc-hidden survives history nav only
if !allowBindings || !suggestBlob: return false              // menu not visible → forward everything
match bindings (key name + exact shift/ctrl booleans):
  dismiss (escape) → suggestBlob=undefined; hideSuggestions=true; return true   // Esc is swallowed
  prev    (up)     → idx=max(0,idx-1); true
  next    (down)   → idx=min(idx+1,len-1); true
  accept  (tab)    → write(replacement); true (or false if nothing to insert → Tab forwarded)
  return | ctrl+c  → clearCommand(); false (forwarded)
  else             → false (forwarded)
```

Backspace mapping (`shell.ts:263-264`): `"\x7f"` for pwsh, powershell, cmd and nu. Otherwise the
original sequence. **(inferred)**: on Windows consoles `0x08` means Ctrl+Backspace (delete word)
in PSReadLine.

### 12.2 Output filtering (`stdioProxy.ts:15-43, 74-85`, applied to all PTY output written to stdout)

- **Removed**: win32-input-mode `CSI ?9001h/l`, kitty keyboard protocol `CSI ?u`, `CSI =…u`,
  `CSI >…u`, `CSI <…u`, xterm modifyOtherKeys `CSI >…m`. Partial tails are carried across chunks.
  Purpose: keep the outer terminal sending legacy key encodings the key parser understands. The
  model still sees the original bytes. Tests: `stdioProxy.test.ts:56-109`.
- **Bare LF → `ESC D` (IND)**: every `\n` not preceded by `\r` becomes IND, which moves down while
  keeping the column. On Windows consoles without `DISABLE_NEWLINE_AUTO_RETURN`, LF also does a
  carriage return (`ansi.ts:32`; tests `stdioProxy.test.ts:111-134`). This rewrite is applied on
  all platforms.

## 13. Inserting the selected suggestion

`calculateReplacement(activeToken, suggestion)` (`ish/src/runtime/replacement.ts:18-43`):

```
text = suggestion.insertValue ?? name + " "
if token == null || token.complete:          → {bs:0, insert:text}   (nothing if blank)
if insertValue && insertValue != name:        → {bs:token.tokenLength, insert:text}
target = name + " "; p = commonPrefix(token.token, target)
if p == token.token.length:                   → {bs:0, insert:target[p..]}  (nothing if blank)
else                                          → {bs:token.tokenLength, insert:target}
```

`applyReplacement` = `"\x7f".repeat(bs) + insert` written to the PTY (`replacement.ts:45-47`). The
same DEL is used for every shell. `tokenLength` is the **display width**, including the quotes of a
quoted token (`parser.ts:90, 129`).

Examples: typed `git ch` + `checkout` → writes `eckout `. Typed `"my f` + `my file` → if the prefix
diverges, `DEL×5` + `my file `.

Fig/Kiro, for contrast (`qac/crates/fig_proto/src/figterm.rs:4-30`): offset as `CSI C`/`CSI D` × n,
then **`0x08` (BS)** × deletion, then the text, then `\r` if `immediate`. figterm also "locks"
the expected buffer after an insertion (`qac/crates/figterm/src/message.rs:162-205`).

## 14. Performance tricks

- `BatchScheduler` (`ish/src/ui/batchScheduler.ts:16-41`): coalesces suggestion requests into a
  microtask. While a run is in flight, new requests set a flag that causes **one** follow-up run.
- Requests run only when the command-state version changed (`ui-root.ts:69-74`).
- Stale results are dropped with `AbortController` plus a monotonic `requestVersion`
  (`suggestionManager.ts:64-67, 94-118`). The cache key is `cwd \0 JSON(tokens)`, so the same
  tokens skip the request (`:89-92`).
- Render cache key: an identical frame with no data writes nothing (`suggestionRenderer.ts:154-162`).
- One `stdout.write` per frame. The cursor is hidden during the draw.
- 150 ms resize debounce. Only `lines` rows are patched per frame, never the whole screen.
- Optional metrics: `ISTERM_PERF=1`, output path `ISTERM_PERF_OUTPUT` (`ish/src/utils/performance.ts:13-14`).

## 15. Startup and exit

- Start: load config and infer the shell (§16). Spawn, raw mode, write
  `clearTerminal` = `CSI 2J CSI 3J CSI H` (Windows: `CSI 2J CSI 0f`). Initialize the suggestion
  runtime (`ui-root.ts:23-113`, `commands/root.ts:22-58`).
- Shell inference (`shell.ts:185-212`): env `NU_VERSION`, `XONSHRC`, `FISH_VERSION`, `ZSH_VERSION`,
  `BASH_VERSION`, then the basename of `$SHELL`, then the parent process name.
- Exit (`ui-root.ts:101-107`): flush pending output, restore cooked mode, write **RIS `ESC c`**
  (full reset; clears the screen), `process.exit(code)`.
- Logging: `-V` → `<res>/log/inshellisense.log`, JSON lines (`ish/src/utils/log.ts:11-49`).

## 16. Config file and CLI

Paths, both loaded in order if present: `~/.inshellisenserc`, then `$XDG_CONFIG_HOME/inshellisense/rc.toml`.
The XDG directory falls back to `~/.config` when unset or not absolute; this fallback is not used
on Windows (`config.ts:110-113`, `constants.ts:10-28`).

Each file is validated with ajv, `additionalProperties:false`. An error prints
`<path> is invalid toml…` or `<path> is invalid: …` and exits. Merge semantics: `bindings.*` merge
per binding with the previous value. Every other key is **reset to its default** if the later file
omits it. Afterwards `~/.fig/autocomplete/build` is prepended to `specs.path` (`config.ts:132-165`).

```toml
# defaults (config.ts:115-129)
useAliases = false                     # bash/zsh alias expansion
useNerdFont = false
maxSuggestions = 5                     # number
activeSuggestionBackgroundColor = "#7D56F4"   # ^#[0-9A-Fa-f]{6}$
boxBorderStyle = "square"              # "square" | "rounded"
[specs]
path = []                              # extra spec dirs
[bindings.acceptSuggestion]   key = "tab"     # shift/control optional bools, default false
[bindings.nextSuggestion]     key = "down"
[bindings.previousSuggestion] key = "up"
[bindings.dismissSuggestions] key = "escape"
```

Key names are Node readline keypress names. A binding matches when `name == key && shift == !!shift && ctrl == !!control`.

CLI (`ish/src/index.ts:27-44`): `-l/--login`, `-s/--shell <shell>`, `-c/--check`, `-T/--test`
(hidden), `-V/--verbose`, `-v/--version`. Subcommands: `init [shell]`, `reinit`, `doctor`, `specs`,
`complete <input>`, `uninstall`.

**Kiro default keybindings differ** and should drive tabsmith (`qac/crates/fig_settings/src/actions.json`):
`insertSelected=enter`, `insertCommonPrefix=tab`,
`navigateUp=shift+tab|up|control+k|control+p`, `navigateDown=down|control+j|control+n`,
`hideAutocomplete=esc`, `selectSuggestion1..10=control+1..0`, `toggleHistoryMode=control+r`. The
`autocomplete.*` settings referenced in qac include `width`, `height`, `theme`, `disable`,
`scrollWrapAround`, `insertSpaceAutomatically`, `onlyShowOnTab`, `fuzzySearch`, `sortMethod`,
`hidePreviewWindow`, `keybindings.<key>` and others.

---

## 17. Rust implementation proposal for tabsmith

### 17.1 Crates

| Concern | Choice | Why / notes |
|---|---|---|
| PTY | **`portable-pty`** (wezterm) | One API for Unix openpty and Windows ConPTY: `native_pty_system().openpty(PtySize{rows,cols,pixel_width:0,pixel_height:0})`, `slave.spawn_command(CommandBuilder)`, `master.try_clone_reader()`, `master.take_writer()`, `master.resize()`. figterm uses it too (`qac/crates/figterm/Cargo.toml`). Its Windows backend loads a **side-loaded `conpty.dll`** if one sits next to the exe, otherwise kernel32's. figterm's own copy does the same (`qac/crates/figterm/src/pty/win/pseudocon.rs:66-80`) with flags `RESIZE_QUIRK(0x2)\|WIN32_INPUT_MODE(0x4)\|PASSTHROUGH_MODE(0x8)` (`:50-52, 107`). Ship `conpty.dll` + `OpenConsole.exe` (Microsoft.Windows.Console.ConPTY NuGet) next to `tabsmith.exe` to match ish's `useConptyDll` **(verify: portable-pty's exact flags and side-load logic in the version you pin)**. |
| Screen model | **`alacritty_terminal`** (upstream) | Full SGR (`Flags::{BOLD,ITALIC,DIM,UNDERLINE*,INVERSE,HIDDEN,STRIKEOUT}`, `Color::{Named,Indexed,Spec(Rgb)}`). `DIM` and `ITALIC` are needed for pwsh ghost-text detection. It has `WIDE_CHAR`/`WIDE_CHAR_SPACER`, `WRAPLINE` (= xterm `isWrapped`), `TermMode::{ALT_SCREEN,SHOW_CURSOR,BRACKETED_PASTE,APP_CURSOR}` and **reflow on resize** like xterm.js. Fig forked this same crate (`qac/crates/alacritty_terminal`). Alternative `vt100`: smaller API, has `rows_formatted()`, but no reflow, and (0.15) no dim/strike **(verify newer versions)**. `wezterm-term` has stable row indices but is not a normal crates.io dependency **(verify)**. |
| Raw mode / size / small escapes | **`crossterm`** | `terminal::{enable_raw_mode, disable_raw_mode, size}`. `cursor::SavePosition`/`RestorePosition` emit `ESC 7`/`ESC 8`. **Do not use `crossterm::event::read`** for the main input path: it loses the raw bytes we must forward unchanged. |
| Unix resize | `signal-hook` (SIGWINCH iterator thread) | Or `crossterm`'s resize event in a dedicated loop. |
| Windows console | `windows-sys` (Console APIs) | Needed for VT input mode, `ReadConsoleInputW` and `DISABLE_NEWLINE_AUTO_RETURN` (§17.6). |
| Width | `unicode-width` | The same table must be used for box math and the model. alacritty uses it. |
| Channels/timers | `std::sync::mpsc` + `recv_timeout` (or `crossbeam-channel`) | No async runtime is needed. |

### 17.2 Threads and event loop (one owner of all state)

```
thread pty-reader : loop read(master) → tx.send(Msg::Pty(bytes))   ; EOF → Msg::Exit
thread stdin      : unix: read(fd 0) ; windows: ReadConsoleInputW (§17.6) → Msg::Stdin / Msg::Resize
thread sigwinch   : unix only → Msg::Resize(cols, rows)
thread child-wait : child.wait() → Msg::Exit(code)         (Windows: then drop master to unblock reader)
thread engine     : owns QuickJS runtime (!Send); req(version, cmd, cwd) → Msg::Suggestions{version, blob}
main              : owns Term, CommandTracker, Overlay, writers; select over rx with resize-debounce timeout
```

On each `Msg::Pty`, drain the other queued `Pty` messages with `try_recv` and process them as one
batch. That means one model advance and **one** stdout `write_all` + flush per batch, an
improvement over ish's per-chunk render. Keep ish's frame order (§7.2) for the batch.

### 17.3 Spawning (port of `pty.ts:432-508`)

```rust
// sketch — verify portable-pty API of the pinned version
let pair = native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })?;
let mut cmd = CommandBuilder::new(&target);            // inherits current env
cmd.args(&args);
cmd.cwd(std::env::current_dir()?);
cmd.env("TERM", "xterm-256color");                     // node-pty `name`
cmd.env("TABSMITH", "1");                              // = ISTERM; nesting guard
if login { cmd.env("TABSMITH_LOGIN", "1"); }
for (k, v) in shell_env(shell) { cmd.env(k, v); }      // zsh: ZDOTDIR=<tmp>/tabsmith-zsh-<pid>, USER_ZDOTDIR=$ZDOTDIR|$HOME
                                                       // cmd: PROMPT=<PS>%PROMPT%|$P$G<CWD><PE>  (see 17.7)
let child = pair.slave.spawn_command(cmd)?;
drop(pair.slave);                                      // unix: reader gets EOF when child exits
let reader = pair.master.try_clone_reader()?;
let writer = pair.master.take_writer()?;
```

Reuse ish's per-shell table (§2.2) as is, including Git Bash discovery and the fish `cygpath`
wrapper on Windows. Use our own OSC id (keep `6973` or adopt `697` like Kiro), and **also accept
`OSC 133;A/B`** (FinalTerm). Many prompts (starship, p10k, WezTerm/VS Code integrations) already
emit it, which gives a free fallback **(recommendation)**.

### 17.4 Model + OSC splitter + command tracker

Upstream alacritty_terminal ignores unknown OSCs, so add a small byte-level splitter in front of
the parser. It must keep state across chunks:

```rust
// states: Ground, Esc, Osc(buf), OscEsc(buf)
// ESC ']' starts collecting; BEL or ESC '\' ends; any other ESC x returns to Ground.
// Collected OSC whose payload starts with b"6973;" / b"697;" / b"133;" → yield Piece::Marker(payload)
// everything else → Piece::Bytes (also any OSC > 4 KiB is flushed as Bytes).
for piece in splitter.feed(&chunk) {
    match piece {
        Piece::Bytes(b) => { parser.advance(&mut term, b); out.extend_from_slice(b); }
        Piece::Marker(p) => tracker.on_marker(&term, p),   // cursor is exact at this byte offset
    }
}
// Markers are NOT copied to `out` (unlike ish), so they never reach the real terminal.
```

Splitting at the marker matters. xterm's OSC hook fires mid-parse with the cursor at that exact
point, and `promptEndX = cursorX` depends on it (`commandManager.ts:70-73`).

Markers without xterm's `registerMarker`: store `abs = history_size + cursor_line` and convert back
with the current `history_size`. When scrollback is saturated (`history_size == max`), treat the
markers as disposed and re-anchor at the cursor line, as ish does (`commandManager.ts:244-248`).
Port `termSync`, `_getCommandLines`, `_getCommandText`, `_getCommandOutputStatus` and
`_isSuggestion` exactly (§4). Use `Flags::WRAPLINE` on the previous row's last cell for `isWrapped`.
Map `Color::Spec(rgb)` to ansi256 for the "dull" test.

DSR: alacritty answers `CSI 6n` through `Event::PtyWrite`. **Ignore it**, because the real
terminal answers (as in ish). Forward all CPR replies from stdin to the PTY. Without Node's
readline in the way, unsolicited CPRs are harmless, so ish's pending-query counter can be dropped
(**(recommendation)**; keep the counter if testing shows stray CPRs).

### 17.5 Overlay renderer (direct port with fixes)

- Port §6 (layout, constants, swap/clamp, paging) and §9 (decision logic + render key cache)
  verbatim. Make widths, heights and the color configurable through `autocomplete.width/height/theme`.
- Line patch from alacritty cells:

```rust
fn line_patch(row: &Row<Cell>, cols: usize, splice: Option<(usize /*start*/, usize /*len*/, &str)>) -> String {
    let mut s = String::from("\x1b[0m\x1b[2K");
    let (start, end) = splice.map(|(a, l, _)| (a, a + l)).unwrap_or((usize::MAX, usize::MAX));
    let last = last_non_blank(row, cols);           // stop early: CSI 2K already blanked the rest
    let mut pen = None; let mut skip = 0usize;      // pending blank run → one CSI n C
    for x in 0..cols {
        if x == start { flush_skip(&mut s, &mut skip); s += splice.unwrap().2; s += "\x1b[0m"; pen = None; }
        if (start..end).contains(&x) { continue; }
        if x > last && x >= end { break; }
        let c = &row[Column(x)];
        if c.flags.contains(Flags::WIDE_CHAR_SPACER) { continue; }
        if is_blank_default(c) { skip += 1; continue; }
        flush_skip(&mut s, &mut skip);
        if pen != Some(style_of(c)) { s += "\x1b[0m"; s += &sgr(c); pen = Some(style_of(c)); } // truecolor: 38;2;r;g;b
        s.push(c.c); if let Some(z) = c.zerowidth() { s.extend(z) }
    }
    s
}
```

  Fixes compared with ish: truecolor is emitted as `38;2`/`48;2` (no lossy 256 mapping, no
  zero-pad bug); blank runs become a single `CSI n C`; output stops at the last non-blank cell.
- Frames: the same `below`/`above` byte layout as §7.2. Wrap each frame in **synchronized output**
  (`CSI ?2026h … CSI ?2026l`) to prevent tearing on terminals that support it. Others ignore it.
  This is a **(recommendation)**, not in ish.
- Save/restore: use `ESC 7`/`ESC 8` everywhere (ish's JetBrains path). DECRC also restores the
  app's SGR pen, which `CSI u` does not. Caveat: DECSC has a single slot, so an app that does
  `ESC 7` in one chunk and `ESC 8` in a later one gets its slot clobbered. A more robust option:
  restore with `CSI row;col H` taken from the model cursor, plus the model's current pen SGR
  (`term.grid().cursor.template`). Skip drawing while the model cursor is in pending-wrap state
  **(recommendation)**.
- Room at the bottom: keep ish's "flip above". Instead of hiding when neither side fits, **shrink
  the page**: `max_visible = available_rows - 2`, minimum 1 **(recommendation)**. Do not inject
  newlines to scroll. That would make the real terminal and the model diverge, and it confuses the
  shell's line editor about its row.

### 17.6 Input per OS

Unix:
- `crossterm::terminal::enable_raw_mode()` (cfmakeraw: ISIG off, so Ctrl-C/Z become bytes for the
  PTY; OPOST off, so LF is not translated).
- A blocking `read(0)` thread forwards bytes. SIGWINCH comes from `signal-hook` → `terminal::size()`.

Windows (figterm does the equivalent, `qac/crates/figterm/src/term/windows.rs:553-578`):

```rust
// stdin handle
mode_in = (old_in & !(ENABLE_ECHO_INPUT|ENABLE_LINE_INPUT|ENABLE_PROCESSED_INPUT))
          | ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_WINDOW_INPUT;
// stdout handle
mode_out = old_out | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN;
SetConsoleCP(CP_UTF8); SetConsoleOutputCP(CP_UTF8);   // restore all four on exit
```

Read with `ReadConsoleInputW`. KEY_EVENT records with `bKeyDown` carry the VT bytes in `uChar`;
join UTF-16 surrogate pairs. `WINDOW_BUFFER_SIZE_EVENT` means resize (figterm:
`qac/crates/figterm/src/input/mod.rs:1176-1187`). Also poll the size every ~250 ms, because legacy
conhost with a large scrollback buffer does not raise that event on window resize **(inferred)**.

Key classifier (shared by both platforms): match only bound keys on **whole sequences** at chunk
granularity. Everything else is forwarded byte for byte. Defaults should follow Kiro (§16):
`\r` Enter, `\t` Tab, `ESC[Z` Shift-Tab, `ESC[A`|`ESC O A` Up (DECCKM!), `ESC[B`|`ESC O B` Down,
`\x0e`/`\x10`/`\x0b`/`\n` for Ctrl-N/P/K/J, and a lone `ESC` = Escape when no further byte arrives
within ~30 ms. Alt-x arrives as `ESC x` in one chunk. Port ish's backspace normalization (§12.1)
and its output stripping of `CSI ?9001h/l`, kitty and modifyOtherKeys sequences (§12.2), with
partial-sequence carry.

### 17.7 Windows gotchas checklist

1. **ConPTY re-renders.** The child's bytes are not passed through verbatim. ConPTY repaints from
   its own buffer, normalizes SGR, repaints the whole viewport after a resize, and older inbox
   versions may drop or reorder private OSCs relative to text. Mitigations: the side-loaded
   `conpty.dll` (ish `pty.ts:81-82`; figterm `pseudocon.rs:66-80`), Fig's delayed EndPrompt on
   Windows (`figterm/src/main.rs:583-584, 859-869`), and the 150 ms post-resize full repaint (§10).
2. **win32-input-mode.** ConPTY created with `WIN32_INPUT_MODE` emits `CSI ?9001h`. Strip it from
   output (ish), otherwise Windows Terminal switches to `CSI …_` key records.
3. **LF auto-return.** Set `DISABLE_NEWLINE_AUTO_RETURN`. If `SetConsoleMode` fails (old conhost),
   fall back to ish's bare-LF → `ESC D` rewrite.
4. **Split UTF-8.** Rust's Windows console stdout rejects invalid UTF-8, and PTY reads split
   multibyte characters. Keep a ≤3-byte carry buffer before every stdout write (ish uses
   `StringDecoder`, `stdioProxy.ts:52,69`) **(inferred: verify std behaviour)**.
5. **Exit detection.** The ConPTY output pipe does not reach EOF when the child exits. Wait on the
   child handle, then drop the master (`ClosePseudoConsole`) to unblock the reader thread
   **(inferred)**.
6. **Resize.** There is no SIGWINCH: use `WINDOW_BUFFER_SIZE_EVENT` plus polling. Call
   `master.resize()` and `term.resize()` together, as ish does (`pty.ts:204-209`).
7. **cmd.exe** has no hooks except `PROMPT`. ish has no CWD for cmd (e2e test skipped,
   `autocomplete.test.ts` "generator results lead suggestions"). Improvement: put CWD into PROMPT
   with ST, using the cmd escape codes `$E` = ESC and `$P` = path:
   `PROMPT=$E]6973;PS$E\` + user prompt + `$E]6973;CWD;$P$E\$E]6973;PE$E\`. The splitter must
   accept the ST terminator. Paths containing `;` are not escaped **(recommendation)**.
8. **Backspace/Ctrl-H.** Send DEL `0x7f` for deletions (ish), not Fig's `0x08`, which PSReadLine
   reads as Ctrl+Backspace (word delete) **(inferred)**.
9. **Ctrl-C.** With processed input off, it arrives as `0x03`. Forward it; ConPTY raises
   CTRL_C_EVENT in the child.

### 17.8 Startup and exit in Rust

Start: check `TABSMITH=1` (nested session → print "live" and exit). Load settings. Enable raw
mode and the console modes. **Clear the screen** (`CSI 2J CSI 3J CSI H`) so the model and the
screen start equal, as ish does. Spawn at `terminal::size()`.

Exit: restore console modes and cooked mode, then `CSI 0m CSI ?25h`. Prefer this over ish's `ESC c`,
which wipes the user's screen; RIS is acceptable if the result differs in practice. Exit with the
child's code.

### 17.9 The one test worth having

A round-trip invariant (no PTY needed): feed `output` into model A. Feed `output + overlay_frame + hide_frame`
into model B, both alacritty Terms of the same size. Assert that the visible grids (chars + fg/bg + flags)
are equal. Then again for a direction flip and a resize repaint. This catches every restoration bug in §7.3.

---

## 18. Open questions / to verify

- Exact `portable-pty` ConPTY flags and side-load behaviour in the pinned version. Whether
  `PASSTHROUGH_MODE` (figterm uses it) improves OSC ordering.
- `vt100` ≥0.16 features (dim, callbacks), if a lighter model than alacritty is wanted.
- Rust std console stdout behaviour with UTF-8 split across writes.
- Whether to strip kitty/modifyOtherKeys **only** outside the alternate screen, so neovim keeps
  the enhanced protocol. ish strips them always (`stdioProxy.ts:74-85`).
