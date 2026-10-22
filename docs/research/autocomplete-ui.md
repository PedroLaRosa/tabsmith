# Autocomplete dropdown behavior (Fig / Amazon Q / Kiro) — re-implementation spec for tabsmith

Scope: the dropdown ("autocomplete window") logic in `packages/autocomplete`, the key interception that
feeds it (figterm), settings, keybindings, ranking, insertion, history, themes.
Everything is read from code unless marked **(inferred)**.

## 0. Path conventions

- `Q/` = `.../scratchpad/ref/qac` (aws/amazon-q-developer-cli-autocomplete)
- `AC/` = `Q/packages/autocomplete/src`. `packages/autocomplete-app/src` is **byte-identical** to it (checked with `diff -rq`), so cite either.
- `ISH/` = `.../scratchpad/ref/ish/src` (inshellisense, for in-terminal sizing comparison only)

Architecture as shipped: figterm (a PTY wrapper, Rust) parses keystrokes and knows the edit buffer → desktop app → webview (React app in `AC/`) which computes suggestions, renders a native floating window, and sends back `InsertText` and `setInterceptKeystrokes`. In tabsmith all of this collapses into one process: the PTY wrapper owns the state machine and draws the menu as an in-terminal overlay. The rules below are what has to be ported.

---

## 1. Core state (store)

`AC/state/types.ts:47-72`, initial values in `AC/state/index.ts:41-59`:

| field | type | initial | meaning |
|---|---|---|---|
| `figState` | `{buffer, cursorLocation, cwd, processUserIsIn, sshContextString, aliases, environmentVariables, shellContext}` | empty, `cwd=null` | Snapshot of the shell edit buffer (`AC/fig/hooks.ts:25-45`) |
| `parserResult` | `{completionObj, currentArg, passedOptions, searchTerm, commandIndex, suggestionFlags, annotations}` | `initialParserState` | Output of the spec parser (`Q/packages/autocomplete-parser/src/parseArguments.ts:92-100`) |
| `generatorStates` | `GeneratorState[]` | `[]` | One per generator on `currentArg`: `{generator, context, loading, result, request?}` (`AC/generators/helpers.ts:17-23`) |
| `command` | tokenized command or null | `null` | From the shell parser |
| `visibleState` | `Visibility` | `HIDDEN_UNTIL_KEYPRESS` | See §2 |
| `lastInsertedSuggestion` | Suggestion or null | `null` | |
| `justInserted` | bool | `false` | Set by any insertion and cleared on the next parser result |
| `suggestions` | Suggestion[] | `[]` | The filtered, ranked list on screen |
| `selectedIndex` | number | `0` | |
| `hasChangedIndex` | bool | `false` | The user navigated, so the selection is sticky across recomputes |
| `historyModeEnabled` | bool | `false` | Toggled by `toggleHistoryMode` |
| `userFuzzySearchEnabled` | bool | `autocomplete.fuzzySearch` or false | The user's preference |
| `fuzzySearchEnabled` | bool | `false` | Effective value after the spec's `filterStrategy` override |
| `settings` | map | `{}` | The full settings map |

`error(msg)` (`AC/state/index.ts:505-513`) resets **everything** to `initialState` and keeps only `figState` and `settings`. That means `visibleState=HIDDEN_UNTIL_KEYPRESS`, `suggestions=[]`, `historyModeEnabled=false`. It fires when:
- the parse of the buffer fails. This includes an empty buffer (`parseArguments` throws `"Invalid token array"` when there are 0 tokens, `parseArguments.ts:1096-1098`), a disabled spec (`autocomplete.disableForCommands`, `loadSpec.ts:222-228`) and any other parser exception (`AC/parser/hooks.ts:69-73`).
- `setFigState` finds the **cursor in the middle of a word**: `buffer.charAt(cursorLocation).trim() !== ""` (`AC/state/index.ts:486-489`). Same reset, with the message "Cursor is in the middle of a word".

The parser only gets `buffer.slice(0, cursorLocation)` (`AC/state/index.ts:491-499`).

Tokens: a trailing space produces an extra **empty token**, so `searchTerm === ""` right after a space (`Q/packages/shell-parser/src/command.ts:82-89`). Token `text` is the node's `innerText` (unquoted).

---

## 2. Visibility state machine

### 2.1 Values (`AC/state/types.ts:8-24`)

| enum | string | meaning |
|---|---|---|
| `VISIBLE` | `"visible"` | The menu is drawn |
| `HIDDEN_UNTIL_KEYPRESS` | `"hidden_until_keypress"` | Hidden. The next parser result (any buffer edit) makes it VISIBLE. Entered after a non-advancing insertion, a backspace into the previous token, a large buffer change (paste or shell history), any error, and at startup |
| `HIDDEN_UNTIL_SHOWN` | `"hidden_until_shown"` | Hidden until explicitly shown: Esc / `hideAutocomplete`, `toggleAutocomplete` from visible, or a new token while `onlyShowOnTab` is on |
| `HIDDEN_BY_INSERTION` | `"insertion"` | Transient state right after a **full** insertion. Resolved on the next parser result |

"Shown" means `visibleState === VISIBLE` (`AC/App.tsx:235`). When it is not, the app renders an empty div (`AC/App.tsx:370-372`).

### 2.2 Transition on every parser result (`setParserResult`, `AC/state/index.ts:297-392`)

Inputs: `parserResult`, `hasBackspacedToNewToken` and `largeBufferChange` (computed in `AC/parser/hooks.ts:59-67`), plus the previous state.

```
hasNewArg = !fieldsAreEqual(new, old, ["currentArg","completionObj"])   // deep compare of those 2 fields
newGeneratorStates = triggerGenerators(parserResult)                      // §9
orig = state.visibleState
vis  = orig
match orig:
  HIDDEN_UNTIL_KEYPRESS -> vis = VISIBLE
  HIDDEN_BY_INSERTION   ->
      insertionTriggeredGenerator = any old generatorState g[i] where
           g[i].generator === lastInsertedSuggestion.generator && newGeneratorStates[i] !== g[i]
      vis = (hasNewArg || insertionTriggeredGenerator) ? VISIBLE : HIDDEN_UNTIL_KEYPRESS
  else (VISIBLE, HIDDEN_UNTIL_SHOWN) -> unchanged
if setting autocomplete.onlyShowOnTab:
      vis = orig                                   // undo the auto-show above
      if hasNewArg: vis = HIDDEN_UNTIL_SHOWN
if hasBackspacedToNewToken:            vis = HIDDEN_UNTIL_KEYPRESS     // overrides even HIDDEN_UNTIL_SHOWN
if largeBufferChange && !justInserted: vis = HIDDEN_UNTIL_KEYPRESS     // overrides even HIDDEN_UNTIL_SHOWN
userFuzzy = hasNewArg ? setting(autocomplete.fuzzySearch,false) : state.userFuzzySearchEnabled
fuzzy = match (currentArg ? currentArg.filterStrategy : completionObj.filterStrategy):
          "prefix" -> false ; "fuzzy" -> true ; _ -> userFuzzy
set { parserResult, visibleState: vis, fuzzySearchEnabled: fuzzy, userFuzzySearchEnabled: userFuzzy,
      generatorStates, justInserted: false }
```

Helper definitions (`AC/parser/hooks.ts:17-29, 59-65`):
- `hasBackspacedToNewToken = tokens.len < oldTokens.len && oldTokens[tokens.len-1].text == tokens[tokens.len-1].text`. Example: `"git "` → `"git"` removes the empty token, so the menu hides until the next key.
- `largeBufferChange = !isBufferDifferenceFromTyping(command.originalTree.text, oldCommand.originalTree.text)`. It counts as typing iff one string is a prefix of the other **and** `|lenA - lenB| < 2`. Anything else is "large": a paste, history navigation, Ctrl-U, a multi-char insertion.

### 2.3 Behavioral summary (derived from the above)

- **Typing** (each char is ≤1-char diff): HIDDEN_UNTIL_KEYPRESS becomes VISIBLE, and VISIBLE stays VISIBLE.
- **Space** creates a new empty token, so `hasNewArg` is usually true. VISIBLE stays VISIBLE and the menu immediately shows the next level (subcommands, options, args). There is no special "after space" hide.
- **After a full insertion** (§5) the state becomes `HIDDEN_BY_INSERTION`. On the next parser result (the echo of the inserted text):
  - The menu re-opens immediately if a new argument began (e.g. `checkout` + auto-space → branch list) or if the inserted suggestion's generator re-triggered (e.g. inserting folder `src/`: the filepaths generator re-runs and lists `src/`).
  - Otherwise it becomes `HIDDEN_UNTIL_KEYPRESS`: no menu until the user types again, e.g. after inserting a flag with no args and no auto-space.
  - The insertion is multi-char, but `justInserted=true` suppresses the large-change hide.
- **Partial insertion** (common prefix): only `justInserted=true`. The menu stays VISIBLE.
- **Esc** (`hideAutocomplete`) gives `HIDDEN_UNTIL_SHOWN`. It stays hidden across typing and new tokens, for the rest of the command line. It leaves that state only through:
  1. `showAutocomplete` or `toggleAutocomplete` (no default bindings);
  2. `onlyShowOnTab` + Tab;
  3. backspacing into the previous token or a large buffer change, which go to HIDDEN_UNTIL_KEYPRESS and then become visible on the next key;
  4. an error reset, e.g. the buffer becomes empty after executing or clearing the line (new prompt), or the cursor moves into the middle of a word.

  There is **no timer**. The type comment says "Hide until explicitly shown (or we enter a new line)" (`types.ts:16-20`). figterm also stops intercepting immediately when Esc is pressed (§4.4), and the Esc keystroke itself is swallowed, not forwarded to the shell.
- **Up at the top of the list** without wrap-around: hides the menu (`HIDDEN_UNTIL_KEYPRESS`). The next Up goes to the shell (history), which is a large change, so the menu stays hidden. With `autocomplete.navigateToHistory=true` it toggles history mode instead (§6.2).
- **Startup / new prompt**: HIDDEN_UNTIL_KEYPRESS with no suggestions.
- **onlyShowOnTab=true**: the menu never auto-opens. Tab opens it. If there is exactly one suggestion and the keypress carries the macOS keycode 48, Tab inserts it directly (`AC/hooks/keypress.ts:90-101`). The menu then stays open while typing within the same argument, and every new argument returns to `HIDDEN_UNTIL_SHOWN`. Auto-execute rows are also suppressed (§7.4).
- **Visible but 0 suggestions**: only an "argument hint" may render (§11.3). Keys are **not** intercepted.

### 2.4 When suggestions are (re)computed (`AC/state/index.ts:189-266`)

A middleware wraps every `set`:
- If the new `visibleState === HIDDEN_UNTIL_KEYPRESS`, it applies the update and **does not recompute** (suggestions stay stale).
- Else it recomputes iff visibility crosses VISIBLE↔not-VISIBLE, or `suggestionsMayChange`. That is: any of `command`, `fuzzySearchEnabled`, `parserResult`, `generatorStates`, `historyModeEnabled` changed (deep compare), or `figState.cwd` / `figState.buffer` changed.
- Settings changes alone do not trigger a recompute.
- On recompute, `selectedIndex=0`, `hasChangedIndex=false`. Exception: the old state had `hasChangedIndex` and the menu will be visible. Then it looks for the previously selected item in the new list by fields `name,type,insertValue,description` and keeps that index (and `hasChangedIndex`) if found.

---

## 3. Actions and default keybindings

### 3.1 Action identifiers (`AC/actions.ts:11-47`, `AC/fig.json`)

| identifier | category | availability | default bindings (fig.json, active at runtime) | default (crates/fig_settings/src/actions.json, figterm boot-time) |
|---|---|---|---|---|
| `insertSelected` | Insertion | WHEN_FOCUSED | `enter` | `enter` |
| `insertCommonPrefix` | Insertion | WHEN_FOCUSED | `tab` | `tab` |
| `insertCommonPrefixOrNavigateDown` | Insertion | WHEN_FOCUSED | – | – |
| `insertCommonPrefixOrInsertSelected` | Insertion | WHEN_FOCUSED | – | – |
| `insertSelectedAndExecute` | Insertion | WHEN_FOCUSED | – | – |
| `execute` | Insertion | WHEN_FOCUSED | – | – |
| `hideAutocomplete` | General | ALWAYS | `esc` | `esc` |
| `showAutocomplete` | General | ALWAYS | – | – |
| `toggleAutocomplete` | (none) | ALWAYS | – | – |
| `navigateUp` | Navigation | WHEN_FOCUSED | `shift+tab`, `up`, `control+p` | `shift+tab`, `up`, `control+k`, `control+p` |
| `navigateDown` | Navigation | WHEN_FOCUSED | `down`, `control+n` | `down`, `control+j`, `control+n` |
| `selectSuggestion1..10` | Navigation | WHEN_FOCUSED | `command+1`..`command+9`, `command+0` | `control+1`..`control+9`, `control+0` |
| `hideDescription` | Appearance | WHEN_FOCUSED | – | – |
| `showDescription` | Appearance | WHEN_FOCUSED | – | – |
| `toggleDescription` | Appearance | WHEN_FOCUSED | `control+k` | `command+i` |
| `toggleHistoryMode` | General | WHEN_FOCUSED | `control+r` | `control+r` |
| `toggleFuzzySearch` | General | WHEN_FOCUSED | – | – |
| `increaseSize` | Appearance | WHEN_FOCUSED | – | `command+=` |
| `decreaseSize` | Appearance | WHEN_FOCUSED | – | `command+-` |

Human names and descriptions are in `AC/fig.json:5-227`. Examples: "Insert shared prefix of available suggestions. Shake if there's no common prefix."; "Scroll up one entry in the list of suggestions"; "Toggle between history suggestions and autocomplete spec suggestions".

**Which defaults win:** figterm boots with `actions.json` (`Q/crates/figterm/src/interceptor.rs:107-115`, `main.rs:588-589`). The first time the UI calls `setInterceptKeystrokes`, the desktop sends the fig.json `ACTIONS` list plus the user's keybinding settings with `override_actions: true` (`Q/crates/fig_desktop/src/request/properties.rs:33-74`). figterm then **clears** its mappings and re-inserts (`interceptor.rs:132-144`). Effective runtime defaults are therefore the **fig.json** column. Recommendation for tabsmith: use the fig.json set.

`command+…` (Meta) bindings are generally not deliverable inside a terminal. **(inferred)** Treat them as macOS-GUI only.

### 3.2 User overrides: `autocomplete.keybindings.<keystring>` = `"<actionIdentifier>"`

- Parsed by `KeyBindings::load_from_json_map` (`Q/crates/fig_settings/src/keybindings.rs:65-83`). Every settings key starting with `autocomplete.keybindings.` becomes a binding: suffix is the key string, value (must be a JSON string) is the action id.
- Merge order: defaults first, then user bindings. Inserting into a map overwrites, so **a user binding for a key replaces the default action for that key**. A default binding cannot be "removed" except by rebinding its key, e.g. to `"ignore"`.
- Special value `"ignore"` (`interceptor.rs:21, 205, 216`): the key is never intercepted and passes through to the shell.
- Binding a key to an unknown action string: the key **is** intercepted (swallowed) and the app logs "Unrecognized action" and does nothing (`AC/hooks/keypress.ts:192-194`).
- Examples from the test (`keybindings.rs:116-142`): `"autocomplete.keybindings.command+i": "toggleDescription"`, `"autocomplete.keybindings.control+-": "increaseSize"`, `"autocomplete.keybindings.control+=": "decreaseSize"`.
- `updateSelectSuggestionKeybindings` (`AC/actions.ts:83-103`) builds a map `selectSuggestionN → [keystrings]` from settings whose value starts with `selectSuggestion`, merged over the defaults. It is only consumed by an unused getter (`getSelectSuggestionKeybinding`), presumably meant for showing digit hints. Nothing renders it.

### 3.3 Key string grammar (`interceptor.rs:26-87`)

- Split on `+` from the left; every segment except the last is a modifier.
  - `ctrl|control` → CTRL, `shift` → SHIFT, `alt|option` → ALT, `meta|command` → META.
  - Unknown modifier → none (silently ignored).
  - The `Some(("", ""))` case lets `+` itself be a key: `"+"` and `"ctrl++"` (CTRL + `+`) both work.
- Key names: `backspace`, `enter`, `arrowleft|left`, `arrowright|right`, `arrowup|up`, `arrowdown|down`, `home`, `end`, `pageup`, `pagedown`, `tab`, `delete`, `insert`, `esc`, `f<N>` (N parsed as u8), otherwise exactly **one character**. Multi-char unknown names return None and the binding is dropped.
- `shift+<lowercase ascii letter>` becomes the uppercase letter with SHIFT removed.
- When registering (`interceptor.rs:146-187`):
  - Arrow keys are additionally registered as their *application-cursor-mode* variants (`ESC O A/B/C/D`).
  - For `ctrl`/`alt` + ASCII letter, the opposite-case letter is registered too (ctrl+r == ctrl+R).
- Shift+Tab arrives as `ESC [ Z` and is decoded as `Tab + SHIFT` (`Q/crates/figterm/src/input/mod.rs:1498-1504`).

### 3.4 What each action does (`AC/hooks/keypress.ts:83-196`)

Preamble on every keypress:
1. `setFigState({...figState, shellContext})`.
2. If `onlyShowOnTab` && keyCode==Tab(48) && not VISIBLE: with exactly 1 suggestion insert it, else set VISIBLE. Return.
3. `if (!selectedItem) return` — every action is a no-op when there are no suggestions.

| action | effect |
|---|---|
| `navigateUp` | `navigate(-1, hideOrShowHistoryOnNegative=true)` |
| `navigateDown` | `navigate(+1)` |
| `insertSelected` | `insertTextForItem(selected)` |
| `insertCommonPrefix` | `insertCommonPrefix()`; on failure → **shake** |
| `insertCommonPrefixOrNavigateDown` | common prefix; on failure → `navigate(+1)` |
| `insertCommonPrefixOrInsertSelected` | common prefix; on failure → `insertTextForItem(selected)` |
| `insertSelectedAndExecute` | `insertTextForItem(selected, execute=true)` |
| `execute` | `Shell.insert("\n")` |
| `hideAutocomplete` | visible := HIDDEN_UNTIL_SHOWN |
| `showAutocomplete` | visible := VISIBLE |
| `toggleAutocomplete` | VISIBLE → HIDDEN_UNTIL_SHOWN, anything else → VISIBLE |
| `toggleHistoryMode` | `historyModeEnabled = !historyModeEnabled` |
| `toggleDescription` | no-op if `autocomplete.alwaysShowDescription`; else toggles the description pop-out (§11.2) |
| `showDescription` / `hideDescription` | **not handled** in the switch: they fall to "Unrecognized action", a no-op. They only affect the hint label (§11.4) |
| `toggleFuzzySearch` | `userFuzzySearchEnabled = !userFuzzySearchEnabled` (see caveat below) |
| `increaseSize` / `decreaseSize` | multiply font size, row height, maxHeight and width by `1.1` or `1/1.1` (`AC/App.tsx:171-188`). Session-only; reset when `autocomplete.width/height` change |
| `selectSuggestionN` | **Bug:** `parseInt("selectSuggestion3",10)` is NaN, so the condition is false and it does nothing (`keypress.ts:181-189`). The condition also has `suggestionIndex <= 0`. Intended behavior **(inferred)**: select (not insert) item N-1 if it exists, with visible := VISIBLE |

`navigate(delta, hideOrShowHistoryOnNegative)` (`keypress.ts:59-81`):
```
newIndex = selectedIndex + delta ; vis = visibleState
if setting autocomplete.scrollWrapAround: newIndex = (newIndex + len) % len
elif newIndex == -1 && hideOrShowHistoryOnNegative:
    if setting autocomplete.navigateToHistory: historyModeEnabled = !historyModeEnabled; return
    vis = HIDDEN_UNTIL_KEYPRESS
scroll(newIndex, vis)   // scroll clamps to [0, len-1]; hasChangedIndex = (clamped != old)
```
Down at the bottom without wrap clamps and stays on the last row.

Caveat on `toggleFuzzySearch`: it changes `userFuzzySearchEnabled` only. The effective `fuzzySearchEnabled` is recomputed on the **next** parser result, so the effect shows on the next keystroke, and it is reset to the setting whenever a new argument starts (§2.2). Faithful port: same. Nicer **(inferred)**: recompute immediately.

### 3.5 Shake

`useShake(ms=200)` (`AC/hooks/helpers.ts:36-45`) adds CSS class `shake` for 200 ms. That is a 0.5 s infinite jitter animation, `AC/index.css:6-45`. Terminal equivalent **(inferred)**: ring BEL (`\x07`) or briefly invert the selected row.

---

## 4. Key interception (which keys are swallowed)

### 4.1 Intercept flags, set by the UI (`AC/App.tsx:235-249`)

```
interceptKeystrokes (bound) = visibleState == VISIBLE && suggestions.len > 0
globalIntercept             = suggestions.len > 0
```
These are sent on every change, together with the action list and the session id (`AC/hooks/keypress.ts:12-22`). figterm also tracks `window_visible`, which the desktop sets when the native window is actually shown or hidden (`Q/crates/fig_desktop/src/webview/window.rs:349,380`).

### 4.2 Decision (`interceptor.rs:195-226`)

```
match (intercept_global, intercept):
  (true, false):  // suggestions exist but menu hidden
      if key == Tab(no mods) && onlyShowOnTab -> "showAutocomplete"
      elif mapping[key] in GLOBAL_ACTIONS ["toggleAutocomplete","showAutocomplete"] -> that action
      else -> pass through
  (_, true):      // menu visible with suggestions
      if window_visible: mapping[key] (unless "ignore") else pass through
  _ -> pass through
```
`hideAutocomplete` is ALWAYS-availability, but it is not in the hardcoded GLOBAL_ACTIONS list. When the menu is hidden, Esc passes through.

### 4.3 Main loop (`Q/crates/figterm/src/main.rs:724-797`)

- No interception while a command is running (`preexec`).
- An intercepted key is **not written to the PTY**. figterm sends `InterceptedKey{action, context, raw}` to the UI.
- **Escape** additionally calls `key_interceptor.reset()`: both intercept flags become false at once.
- Unintercepted `Ctrl+C` or `Ctrl+D` also reset the interceptor before being forwarded.
- An `Insert{unlock:true}` resets it as well.

### 4.4 Net effect at default bindings

While the menu is visible with ≥1 row, figterm swallows Enter, Tab, Shift+Tab, Up, Down, Ctrl+P, Ctrl+N, Ctrl+K, Ctrl+R and Esc. Everything else (letters, Backspace, Left/Right, Ctrl+A/E/U/W, …) reaches the shell. The shell's echo updates the buffer, which re-runs the state machine.

---

## 5. Insertion

### 5.1 Full insertion: `insertTextForItem(item, execute=false)` (`AC/state/insertion.ts:193-224`)

```
text = getFullInsertion(item, searchTerm, fuzzySearchEnabled, settings["autocomplete.preferVerboseSuggestions"])
if execute && item.type != "auto-execute": text += "\n"
text = text.replace(/\n+$/, "\n")                       // collapse trailing newlines
if !text.endsWith("\n") && item.shouldAddSpace && (settings["autocomplete.insertSpaceAutomatically"] ?? true):
    text += " "
insertString(state, item, text, isFullCompletion=true)
```

`getFullInsertion` (`insertion.ts:165-191`):
```
queryTerm = getQueryTermForSuggestion(item, searchTerm)          // §7.1
isFolder = item.type=="folder"; isFileOrFolder = type in {file, folder}
if item.insertValue && !isFileOrFolder: return item.insertValue    // verbatim, NOT escaped
match = getNameMatch(item, queryTerm, fuzzy, preferVerbose)        // which alias to insert
if !match: return escapeInsertion("", isFolder)                    // = ""
return escapeInsertion(item.separatorToAdd ? match + separatorToAdd + "{cursor}" : match, isFolder)
```

`getNameMatch` (`AC/suggestions/helpers.ts:62-91`):
1. Candidate names are `makeArray(item.name)`.
2. Prefix mode: keep names where `name === q`, or case-insensitively equal, or `startsWith(q)`, or case-insensitive startsWith. Fuzzy mode: names where fuzzysort matches. Original array order is kept.
3. Pick: if `preferVerboseSuggestions` and type is `option` or `subcommand`, the **longest** matched name; else the **first** matched name.
4. If nothing matched but `displayName` exists (the row was shown because the displayName matched), pick from all names the same way.

So option `["-m","--message"]` + empty query inserts `-m`, or `--message` with preferVerbose. With query `--m` it inserts `--message`.

`escapeInsertion(str, isFolder)` (`insertion.ts:151-163`):
```
special = set("\\?*'\"#|<>()[]!&")          // NOTE: not $ ; ` ~ { } and not space
if no char of str in special:
    return str contains " " ? str.replace(/\s/g, "\\ ") : str      // backslash-escape every whitespace
if isFolder: return "'" + str[..-1].replace("'", "'\"'\"'") + "'/"   // quote all but trailing '/'
return "'" + str.replace(/'/g, "'\"'\"'") + "'"
```
`insertValue` (non file/folder) is never escaped. The `{cursor}` placeholder is added before escaping and removed later.

`separatorToAdd` for options (`AC/suggestions/index.ts:160-179`):
- `undefined` if the option's first arg `isOptional`.
- else if `requiresSeparator`: a string value is used as-is; `true` means the first of `completionObj.parserDirectives.optionArgSeparators`, or `"="`.
- else if `requiresEquals`: `"="`.
- else none.

`shouldAddSpace` exists only on subcommand and option suggestions (`AC/suggestions/index.ts:45-68`):
1. The first arg exists and is not optional → true.
2. Else, if the item has a boolean `requiresSubcommand`, return it.
3. Else true iff the item has ≥1 subcommand.

Args, generator results, files, folders and history never get an auto-space. A folder ends with `/` and the folder generator re-triggers on `/` (§2.3).

Quirk to replicate: an option with a separator and a mandatory arg gives text `--out={cursor} `, which ends up as `--out= ` with the cursor moved one left, right after `=`.

### 5.2 `insertString` → `sendTextToTerminal` (`insertion.ts:25-149`)

```
queryTerm = getQueryTermForSuggestion(item, searchTerm)
val = text
if (ci = val.indexOf("{cursor}")) > -1:
    val = val.replace("{cursor}", "")                 // first occurrence only
    moveCursor = "\x1b[D" * (val.len - ci)            // ESC [ D = cursor left, once per char after the placeholder
n = 0
if (item.type not in {auto-execute, special}) || !isFullCompletion:
    n = queryTerm.replace(/\s/, "\\ ").len            // NOTE: only FIRST whitespace counted as "\ " (no /g)
if item.type=="shortcut" && searchTerm.startsWith("?"): n += 1
if rootCommand == "?": n += 2                         // rootCommand = command.tokens[commandIndex].text
if val.startsWith(queryTerm) && n >= queryTerm.len:   // case-SENSITIVE: don't retype what's already typed
    n -= queryTerm.len; val = val[queryTerm.len..]
final = "\b" * n + val + moveCursor                   // \b = 0x08, the shell treats it as backward-delete-char
Shell.insert(final, {insertionBuffer: buffer}, sessionId)
```

Then:
- `isFullCompletion` → `updateAutocompleteIndexFromUserInsert(rootCommand, names[0])` (§8.3).
- `updateVisibilityPostInsert`:
  - full: `{visibleState: HIDDEN_BY_INSERTION, lastInsertedSuggestion: item, justInserted: true}`
  - partial: `{justInserted: true}`

Writing to the PTY (`Q/crates/figterm/src/message.rs:162-206`, `Q/crates/fig_proto/src/figterm.rs:4-29`):
- The insertion is dropped while a command runs (preexec), unless `insert_during_command`.
- **Buffer reconciliation**: `insertion_buffer` is the buffer the UI computed against. If the live buffer has moved on:
  - live buffer starts with it → first send `\x08` × (extra chars) to delete what the user typed meanwhile;
  - it starts with the live buffer → first type the missing chars.
- Then `to_term_string` = (`offset`>0: `ESC[C`×offset; <0: `ESC[D`×|offset|) + `\x08`×deletion + insertion + (`immediate` ? `\r` : ""). The autocomplete path only uses `insertion`; the backspaces are embedded in the text.
- tabsmith: write `final` to the PTY master directly, with the same reconciliation.

Auto-execute rows insert `"\n"` with no deletions, so selecting one sends `\n` = accept-line.

### 5.3 Common prefix: `insertCommonPrefix()` (`insertion.ts:226-304`)

Default binding is Tab.
```
sel = suggestions[selectedIndex]; q = getQueryTermForSuggestion(sel, searchTerm)
if suggestions.len == 1: return insertTextForItem(sel)                 // single → full insert
t = sel.type == "auto-execute" ? sel.originalType : sel.type
if t in {special, auto-execute}: throw InsertPrefixError                 // → shake
filterItem = sel with type=t (and names "/"-terminated if sel is an auto-execute folder)
names = suggestions.filter(isMatchingType(_, filterItem))
                   .map(firstName.toLowerCase())
                   .filter(n => n.startsWith(q.toLowerCase()))
if names.len == 1: (sel is auto-execute ? throw : return insertTextForItem(sel))
lcp = longestCommonPrefix(names)                     // sort, then compare first vs last
shared = firstName(sel).slice(0, lcp.len)            // keep the SELECTED item's casing
if !shared || shared == q: throw "No remaining prefix to insert"   // → shake
text = shared.replace(/\s/g, "\\ ")
if text == getFullInsertion(sel, ...): (sel is auto-execute ? throw : return insertTextForItem(sel))
insertString(state, sel, text, isFullCompletion=false)   // partial: menu stays visible
```
`isMatchingType(a,b)` (`helpers.ts:15-20`): if a's first name is `"../"`, then b's first name must be `"../"`. Otherwise both are file-or-folder, or `a.type === b.type`.

`longestCommonPrefix` (`Q/packages/shared/src/utils.ts:71-84`) **sorts the array in place** (JS default sort, UTF-16) and returns the common prefix of the first and last elements.

---

## 6. Suggestion sources

### 6.1 Spec suggestions: `getAllSuggestions` (`AC/suggestions/index.ts:193-290`)

The concatenation order (before priority sort) is:
1. If `suggestionFlags & Subcommands`: subcommands (deduped by identity), each with `type:"subcommand"` and `shouldAddSpace`. Sorted with `localeCompare` on the first name.
2. If `flags & Args`:
   - static `currentArg.suggestions`. A string becomes `{type:"arg", name, isDangerous: currentArg.isDangerous}`; an object keeps its own type or gets `"arg"`.
   - then each generator's `result`. A loading generator that has `getQueryTerm` is skipped, so its stale results are not shown. Without `getQueryTerm`, stale results stay visible while reloading.
3. `completionObj.additionalSuggestions`. These have no type. Without a type or icon they get `icon = fig://template?color=628dad&badge=➡️`. Alphabetical.
4. If `flags & Options`: options and persistentOptions (deduped).
   - Remove options whose names appear in any passed option's `exclusiveOn`.
   - Remove options already used `>= isRepeatable` times (`isRepeatable` undefined/false → 1; `true` → unlimited).
   - Options named in a passed option's `dependsOn` that are not yet passed get `priority: 75`.
   - Then `shouldAddSpace`, `separatorToAdd`, `type:"option"`. Alphabetical.

Option chains (`-abc` composite token whose last subtoken is an option), `index.ts:233-287`:
- Keep only option and arg rows.
- If options are not suggestable, still add the exact short option for the last letter.
- Args get `getQueryTerm: () => ""`.
- The short option equal to the last letter is renamed to the full chain (`-abc`).
- Other short options `-x` become `{name: insertValue: "-abc" + "x"}`.

### 6.2 History suggestions (`AC/history/index.ts:403-462`)

- `prefix` = `buffer.slice(0, start of the last token's inner text)`.
- Entries come from:
  - `beta.history.customCommand` set → the custom-command source;
  - `beta.history.allShells` && shell ∈ {bash, zsh} → zsh + bash entries;
  - else the source named after the current shell process basename (`zsh|bash|fish`).
- For each entry with `text.startsWith(prefix)`: `name = text.slice(prefix.len).trimEnd()`. Unique names in history order.
- Each becomes `{type:"history", description:"past command", name, insertValue:name, icon:"📚", priority}`, where `priority = freq(firstWord(name)) > 1 ? 75 + min(freq,10)/10 : 50`.

Mixing (`AC/state/index.ts:124-160`), driven by `beta.history.mode` ∈ `"off"|"history_only"|"show"`, effectively off when unset:
- `"show"`: append history rows whose name is not already a spec suggestion name. Apply recency priorities. Then each suggestion's priority = max(own, priority of the history row whose first word equals one of its names).
- `"history_only"`, or `historyModeEnabled` toggled by `toggleHistoryMode` / Ctrl+R: **only** history rows.

Loading (`AC/history/index.ts:252-305`, called once at startup unless `autocomplete.history.disableLoading`, `AC/App.tsx:208-216`):
- First Fig's own DB: `SELECT command, shell, session_id, cwd, start_time, exit_code FROM history`.
- Then either `beta.history.customCommand` run via `zsh -c <cmd>`, or the shells in order:
  - zsh: `fc -R; fc -ln 1`
  - bash: `fc -ln 1`
  - fish: `history search`
  
  each run through `executeLoginShell`. Output is split on `\n`.
- Entries are also parsed against specs to build an index of past argument values for the `template: "history"` generator (`getHistoryArgSuggestions`, `history/index.ts:319-401`). Those suggestions are `{name: value, type:"arg", context:{templateType:"history", ...entryContext}}`.

### 6.3 First token (command name) completion (`Q/packages/autocomplete-parser/src/parseArguments.ts:1026-1119`)

When the command has exactly one token:
- If the token contains `/`, use spec `dotslash`, or `php/bin-console` for `bin/console`. This applies regardless of the setting.
- Else use the built-in `firstTokenSpec`. Its single arg `command` has a custom generator, **but only if `autocomplete.firstTokenCompletion` is true**. Otherwise `args: []`, so there are **no suggestions for the first token** by default (history rows can still appear in history mode).

The generator:
- fish: `complete -C ""`, lines `name\tdescription`. Bug: the name slice includes the tab.
- bash: `compgen -c`
- zsh: `for key in ${(k)commands}; do echo $key; done && alias +r`

Results are `type:"subcommand"`, deduped, and cached `stale-while-revalidate` with a 10 s TTL.

---

## 7. Filtering and ranking

Pipeline (`AC/state/index.ts:98-187`):
```
s = spec suggestions (+ history per §6.2)
s = updatePriorities(s, command.tokens[0].text)        // §8; root = FIRST token (not commandIndex)
s = stable_sort_desc(s, priority)
s = filterSuggestions(s, searchTerm, fuzzySearchEnabled,
                      currentArg?.suggestCurrentToken ?? setting(autocomplete.alwaysSuggestCurrentToken,false), settings)
if 0 < s.len <= 50: s = dedupe(s, ["name","insertValue","displayName","args"])   // keep first; deep-equal
```

### 7.1 Query term per suggestion (`helpers.ts:102-128`)

- `getQueryTerm = suggestion.getQueryTerm || suggestion.generator.getQueryTerm`.
  - If it is a string `c`: `searchTerm.slice(searchTerm.lastIndexOf(c) + 1)`. When `c` is absent this is `slice(0)`, the whole term. The filepaths generator uses `"/"`: `src/ma` → `ma`.
  - If it is a function: its result, or the searchTerm when it throws.
- If the type is `"shortcut"` and the term starts with `?`, strip the `?`.
- Else the searchTerm.

### 7.2 Empty search term (`AC/suggestions/index.ts:407-427`)

Drop `hidden` rows. If `autocomplete.immediatelyExecuteAfterSpace` and ≥1 row and auto-execute is not hidden (§7.4), prepend `{name:"↪", type:"auto-execute", insertValue:"\n", description:"Immediately execute"}`. Otherwise keep priority order.

So by default, right after `git `, Enter inserts the first suggestion, because the menu is visible. This setting exists for users who habitually type a trailing space before Enter.

### 7.3 Non-empty search term (`index.ts:429-519`)

For each suggestion `s` (already priority-sorted):
- `q` = query term for `s`. `eq` = `q + "/"` if `s.type=="folder"`, else `q`.
- `exact` = the first name `n` with `n.toLowerCase() == eq.toLowerCase()`. Exact matching is case-insensitive.
- **If `exact`**:
  - `rows = [s]`.
  - Prepend an auto-execute clone if all of these hold, and no auto-execute row has been added yet:
    - `(!names[0] || !insertValue || names[0] == insertValue)`
    - `(!s.isDangerous || autocomplete.immediatelyRunDangerousCommands)`
    - `(!s.args[0] || s.args[0].isOptional)`
    - auto-execute is not hidden

    The clone is `{...s, type:"auto-execute", icon:"fig://icon?type=carrot", name: folder ? exact.slice(0,-1) : s.name, description: folder ? "folder" : s.description, insertValue:"\n"}`.
  - Non-folder exact rows go to bucket A; folder exact rows go to bucket B. The variables are named caseSensitive/nonCaseSensitive, but the actual split is `q === eq`, i.e. non-folder vs folder.
- **Else if `!s.hidden`**: partial match over `names` (below). On a match, push `{suggestion: s with fuzzyMatchData, score}` to C.
- **Additionally**, if `s.displayName`, run a partial match on `[displayName]` and push to C as well. This can duplicate; dedupe later removes it when ≤50 rows.

Partial match (`getPartialMatch`, `index.ts:357-391`):
- Prefix scores (`helpers.ts:40-53`), best = max over names:
  - `0` exact case-sensitive
  - `-1` exact case-insensitive
  - `-2` startsWith case-sensitive
  - `-3` startsWith case-insensitive
  - no match → null
- Prefix mode: score = best prefix score; a row with no prefix match is excluded.
- Fuzzy mode: score = best `fuzzysort.single(q, name).score` (§7.5); no fuzzy match → excluded. If it **also** prefix-matches, score = `prefixScore / 10` (∈ {0, -0.1, -0.2, -0.3}), so prefix hits always rank above pure fuzzy hits. Fuzzy scores are integers ≤ 0 and usually much lower.
- `bestScoreMatch` = highest score, nulls ignored.

C is sorted by score desc, then priority desc (a stable sort).

Result = `A ++ B ++ C`. If no auto-execute row was added, the result is non-empty, and auto-execute is not hidden, then `addAutoExecuteSuggestion` (`index.ts:297-339`) runs:
- If `searchTerm == "."`, or (`searchTerm` ends with `/` and some row is a template suggestion with `context.templateType == "folders"`): prepend `{name: "." or "↪", type:"auto-execute", originalType:"folder", insertValue:"\n", description:"Enter the current directory"}`.
- Elif `suggestCurrentToken`: prepend `{name: searchTerm, type:"auto-execute", insertValue:"\n", description:"Enter the current argument"}`.

### 7.4 Auto-execute visibility

Hidden when `autocomplete.hideAutoExecuteSuggestion || autocomplete.onlyShowOnTab` (`index.ts:400-402`).

**Dangerous commands**: `isDangerous` comes from the spec (subcommand, option, arg). It is copied onto static arg suggestions and generator results (`index.ts:70-93, 113-115`; `scriptSuggestionsGenerator.ts:85`). Its only effect is suppressing the exact-match auto-execute row unless `autocomplete.immediatelyRunDangerousCommands`. On a dangerous exact match, Enter therefore inserts the (already typed) suggestion plus an optional space, and a second Enter runs it ("hit enter twice").

### 7.5 fuzzysort (`Q/packages/fuzzysort/fuzzysort.js`, fork of farzher/fuzzysort)

`single(search, target)` returns null if either is empty. All comparisons are on lowercase char codes (`String.toLowerCase`, per UTF-16 unit).

1. **Simple pass** (`:130-140`): walk the target and greedily match search chars in order, recording indexes in `matchesSimple`. Fail (null) if not all are found.
2. **Beginning indexes** (`:216-235`): index i is a "beginning" if (upper && !prevUpper) || !prevAlnum || !isAlnum. Alnum is ASCII `[A-Za-z0-9]`, judged on the *original-case* target. `nextBeginningIndexes[i]` = the smallest beginning index > i, else targetLen (`:236-252`).
3. **Strict pass** (`:150-179`):
   - Start at `targetI = matchesSimple[0]==0 ? 0 : nextBeginning[matchesSimple[0]-1]`, and skip the pass if that equals targetLen.
   - Loop. On a char match, record it, advance search and target. On a mismatch, jump `targetI = nextBeginning[targetI]`.
   - On running off the end, backtrack: pop the last strict match `m`, `targetI = nextBeginning[m]`, and decrement searchI. Give up when searchI==0.
   - Success = all search chars matched only at consecutive positions or word beginnings.
4. **Score** (`:181-206`):
   - `best` = strict indexes if strict succeeded, else simple.
   - `score = 0; last = -1; for each idx: if last != idx-1: score -= idx; last = idx`.
   - If not strict: `score *= 1000`. Then `score -= (targetLen - searchLen)`.
   - Return `{target, score, indexes}`. Higher is better; 0 = exact.
   - Examples from the docs: `single('fs','Fuzzy Search')` → -16; `single('test','test')` → 0.

Note: the returned object is a cached, **mutated** shared object per target string. In Rust, return a fresh struct.

---

## 8. Priorities, recency, sortMethod (`AC/suggestions/sorting.ts`)

### 8.1 Indexes

Two maps `cmd → {param → number}` are persisted in localStorage:
- `term_frequency_index_fig_only`: count of inserts. It is written but **never used for ranking**.
- `term_frequency_index_fig_only_recency`: `Date.now()` ms of the last insert.

tabsmith **(inferred)**: persist to a JSON or sqlite file in the data dir.

### 8.2 `updatePriorities(suggestions, cmd)` (`sorting.ts:52-83`)

```
useRecency = setting("autocomplete.sortMethod") != "alphabetical"     // default/unset/"most recent" → recency
for s:
  name = firstName(s)
  recency = useRecency && name != "../" ? recencyIndex[cmd][name] : undefined
  boost = recency ? recency / 1e13 : 0              // ≈ 0.17 for 2023-2026 timestamps; newer → larger
  p = s.priority || 50                              // NB: priority 0 is treated as 50
  if s.type != "auto-execute": p = clamp(p, 0, 100)
  if recency && 50 <= p <= 75: p = 75 + boost       // used items jump into the 75..76 band, ordered by recency
  else: p += boost
```
`cmd` is `command.tokens[0].text` at ranking time. Insert-time updates use `command.tokens[commandIndex].text`, so they mismatch for wrappers like `sudo git` (a quirk).

Static lists are alphabetical (`localeCompare`) before the stable priority sort. So with `alphabetical` and equal priorities, the order within each category is alphabetical. The categories follow the concatenation order of §6.1.

### 8.3 Update on insert (`sorting.ts:20-50`)

On every **full** insertion: `freq[rootCommand][names[0]] += 1` and `recency[rootCommand][names[0]] = now`. Skipped if the name contains `"↪"`.

---

## 9. Generators, loading and debounce (summary; details belong to the generators report)

`triggerGenerators` (`AC/state/generators.ts:118-198`), per generator of `currentArg`:
- Trigger if there is no previous state for the slot, or `currentArg` changed (by identity).
- Else, with `trigger` undefined: trigger only if `currentArg.debounce`.
- `trigger: string`: fires when `lastIndexOf(trigger)` differs between the new and old searchTerm.
- `trigger: function(new, old)`: its result.
- `{on:"threshold", length}`: `new.len > length && !(old.len > length)`.
- `{on:"match", string|string[]}`: `indexOf(new)` vs `indexOf(old)` in the list differ.
- `{on:"change"}` / default: `new != old`.
- A throwing trigger counts as a trigger.

A new state keeps the previous `result` (so stale results can show) and sets `loading: true`. Debounce: `currentArg.debounce` (number > 0 → that many ms, else 200 ms) delays the run.

Generator kinds: script (timeout = max(`autocomplete.scriptTimeout` default **5000**, `generator.scriptTimeout`, `input.timeout`)), custom, template (`history`, `help`). Caching is in `AC/generators/helpers.ts:30-58` and `cache.ts`. `beta.autocomplete.auto-cache` makes every generator cache SWR with a 1 s TTL.

**Loading spinner** (`AC/App.tsx:123, 218-232, 437-439`):
- `isLoading` = true while `parseArguments` is in flight (`AC/parser/hooks.ts:52-58`), or once any generator has been `loading` for **200 ms** (setTimeout and cancel).
- While loading, the **whole window content is replaced** by the spinner, 4 dots animating over 0.66 s (`AC/components/LoadingIcon.tsx`).
- The inline spinner next to the description bar (`App.tsx:484-492`) is dead code, because that branch only renders when `!isLoading`.
- Recommendation **(inferred)**: in a terminal, keep the list and show a spinner glyph (e.g. `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) in the description bar after 200 ms. Never blank the list on every keystroke for parsing.

---

## 10. Row rendering (`AC/components/Suggestion.tsx`, `SuggestionIcon.tsx`)

### 10.1 Layout

Each row is a single line of fixed height `itemSize`, left padding 6 px. It contains:

**[icon]** (size `itemSize*0.75`, 5 px right margin)
- `icon` string shorter than 4 chars: render it as text or emoji at 0.8× size (e.g. `📚` for history).
- `icon` is a URL:
  - `fig://icon?type=<t>`: a named icon (alert, android, apple, asterisk, aws, azure, **box**, **carrot**, characters, **command**, commandkey, commit, cpu, database, discord, docker, firebase, **flag**, gcloud, gear, git, github, gitlab, gradle, heroku, invite, kubernetes, netlify, node, npm, okteto, **option**, package, slack, **string**, template, twitter, vercel, yarn; `AC/fig/icons.ts`). Fetched from `https://specs.q.us-east-1.amazonaws.com/icons/<t>.png`.
  - `fig://template?color=<hex>&badge=<text>`: a colored rounded square with the badge text.
  - `fig://path/...` or another URL: a file icon or image.
- No icon, by type: `folder`/`file` → OS file icon for `cwd/name`; `subcommand` → command; `option` → option; `shortcut` → template 3498db 💡; `auto-execute` → carrot; `arg` → box; `mixin` → template 628dad ➡️; anything else → box (`SuggestionIcon.tsx:150-167`).
- Terminal mapping **(inferred, suggestion)**: emoji or 1-col glyphs per type, e.g. subcommand `▸`/`⌘`, option `⚑`/`-`, arg `□`, file `📄`, folder `📁`, auto-execute `↪`, history `📚`, special `★`. Make this themeable and offer an ASCII fallback.

**[title]**
- `displayName` if present, else **all names joined with `", "`** (e.g. `-m, --message`).
- Match highlight (`getTitle`, `:35-271`):
  - prefix mode: `RegExp("^(" + queryTerm + ")(.*)", "i")` per name. The queryTerm is NOT regex-escaped; on a regex error it falls back to plain text. The matched head gets bg `matching-bg` (`selected-matching-bg` on the selected row).
  - fuzzy mode: highlight the fuzzysort `indexes`, with adjacent same-type runs merged. They are recomputed when a `displayName` exists.
  - No highlight when the queryTerm is empty.
- Common-prefix underline: `commonPrefix = getCommonSuggestionPrefix(selectedIndex, suggestions)` (`helpers.ts:130-168`). That is the LCP of the lowercased first names of all rows with the same type as the selected row. It is not filtered by query and is null if fewer than 2 such rows or the type is special/auto-execute.
  - It is applied only if non-empty and `!== "-"`.
  - If the first segment is plain text, underline `^(commonPrefix)` case-insensitively.
  - Else, if the first segment is the match highlight and the second is text, underline `^(commonPrefix.slice(queryTerm.len))` in the second.
  - This shows the user what Tab will insert.

**[args hint]** (`:226-232`) at 50% opacity, preceded by a space:
- For each arg with a `name`: base = `name` + (`isVariadic` ? `...` : ``). Optional → `[base]`; mandatory → `<base> ` (with a trailing space). Joined with `" "`.
- Example: `commit  <message> [paths...]`.

The row does **not** show the description; it is in the description bar (§11). Selected row: bg `selected-bg`, text `selected-text`. Clicking a row calls `insertTextForItem`. **(inferred)** Optionally support mouse clicks.

### 10.2 Sizing (`AC/window.ts`, `AC/App.tsx:102-140`)

- Width: `autocomplete.width` if it is a number, else `320` px (+50 if `beta.history.mode` ∈ {history_only, show}).
- Max height: `autocomplete.height` if a number, else `140` px.
- Font size: `autocomplete.fontSize` if > 0, else 12.8 px. Row height = `fontSize × 1.5625`, which is 20 px by default.
- Visible rows = max height / row height = **7** at defaults.
- The list is sized to `itemCount × itemSize` and shrinks and scrolls when that exceeds the max height (`AutoSizedList.tsx:50-65`). The scrollbar is hidden.
- Font family: `autocomplete.fontFamily`, default `Monaco, Hack, monospace`.
- Scrolling: `react-window` `scrollToItem(selectedIndex, "smart")` whenever the selection changes. "smart" (react-window semantics, **inferred**): no scroll if the row is fully visible; if it is within one viewport, minimally scroll so it sits at the edge; else center it.
- Terminal mapping **(inferred)**:
  - rows = `floor(height / (fontSize×1.5625))`, default 7;
  - columns ≈ `width / 8`, so ~40 cols at the default 320 px. That matches inshellisense's `suggestionWidth = 40`, `descriptionWidth = 30`, `maxSuggestions = 5`, `descriptionHeight = 5` (`ISH/ui/suggestionManager.ts:18-24`).
  - Keep the selected row visible with the edge-scroll policy: the window top moves only when the selection leaves it.
  - Place the menu below the cursor line; flip above when there is not enough room (`isAboveCursor`, `App.tsx:279-296`). When above, align the list's bottom edge to the cursor.

---

## 11. Description box

### 11.1 Content (`AC/components/Description.tsx:91-102`)

```
desc = selected.description || (selected.type in {file, folder} ? selected.type : "")
name = ""
if desc.trim() == "": desc = currentArg.description; name = currentArg.name     // fall back to the arg being completed
```

### 11.2 Placement (`AC/App.tsx:153-169, 376-396`, `Description.tsx:108-177`)

Default (`isDescriptionSeparate=false`): a **bottom bar** under the list, single line, height = `itemSize`, `white-space: nowrap`, horizontally scrollable, italic, `description-text` color, 1 px top border `description-border`.
- Renders `**name**: description` when both exist, or `No description` in `#8c8c8c` when there are suggestions but no text.
- Its horizontal scroll resets to 0 when `currentArg` changes.

`toggleDescription` (Ctrl+K) toggles a **pop-out**: a panel `POPOUT_WIDTH = 200` px wide and max-height = menu height, beside the list. It wraps text (`white-space: normal`) and goes on the right, or the left if clipped on screen (`isValidFrame`, `App.tsx:272-315`). It shows the selected item's description or "No description". Toggling back forces `isAboveCursor=false`.

`autocomplete.alwaysShowDescription=true` forces the pop-out permanently and disables `toggleDescription`. It also hides the keybinding hint.

### 11.3 Argument hint

When the menu is visible with **0 suggestions** but `currentArg` has a name or description, the window shows only the description with 8 px horizontal padding. If both exist it is 2 rows tall: `**name**:` then a line break, then the description. Example: after `git commit -m ` it shows the `message` arg doc. No keys are intercepted in that state.

### 11.4 Hint chip

Shown at the right end of the bottom bar (font = 0.5 × itemSize, italic) when there are suggestions and `shouldShowHint = !isLoading && !alwaysShowDescription`.
- Text = the first settings key whose value is `toggleDescription`, `showDescription` or `hideDescription`, with `autocomplete.keybindings.` stripped. Tokens split on `+` are mapped: `cmd|command|meta`→`⌘`, `control|ctrl`→`⌃`, `shift`→`⇧`, `option|opt`→`⌥`, then joined with no separator.
- Fallback **`⌃k`** (`Description.tsx:29-74`).

---

## 12. Settings (`autocomplete.*` and related)

Keys enum: `Q/packages/api-bindings-wrappers/src/settings.ts:1-51`. Defaults and descriptions: `Q/packages/dashboard-app/src/data/autocomplete.ts`. `getSetting(key, default)` returns `settings[key] ?? default`. Unset keys are `undefined`, so code-level defaults apply as listed in "Effective default (code)".

| key | type | UI default | Effective default (code) | semantics / where read |
|---|---|---|---|---|
| `autocomplete.disable` | bool | false | false | Master off switch ("Enable Autocomplete", inverted in the UI). Read by desktop/doctor (`Q/crates/fig_desktop/src/main.rs:172`, `q_cli/.../doctor/mod.rs:1178`) |
| `autocomplete.insertSpaceAutomatically` | bool | true | `?? true` | Append a space after a subcommand or option that `shouldAddSpace` (§5.1) |
| `autocomplete.immediatelyExecuteAfterSpace` | bool | false | falsy | With an empty search term, prepend `↪ Immediately execute` (§7.2) |
| `autocomplete.scrollWrapAround` | bool | false | falsy | Up/Down wrap around (§3.4) |
| `autocomplete.alwaysSuggestCurrentToken` | bool | false | false | Prepend an auto-execute row with the typed token ("Enter the current argument"); an arg's `suggestCurrentToken` overrides it |
| `autocomplete.fuzzySearch` | bool | false | false | Fuzzy instead of prefix filtering. Overridden by `filterStrategy`, and the user preference is reset on each new arg |
| `autocomplete.preferVerboseSuggestions` | bool | false | falsy | Insert the longest alias of an option or subcommand |
| `autocomplete.onlyShowOnTab` | bool | false | false | Menu only on Tab (§2.3). Also hides auto-execute rows. figterm reads it once at startup (`LazyLock`, `interceptor.rs:23-24`) |
| `autocomplete.disableForCommands` | string[] | [] | [] | Spec names never loaded (spec-load error → reset/hidden) |
| `autocomplete.sortMethod` | `"most recent"`\|`"alphabetical"` | "most recent" | recency unless `"alphabetical"` | §8.2 |
| `autocomplete.scriptTimeout` | number ms | 5000 | 5000 | Script generator timeout floor |
| `autocomplete.immediatelyRunDangerousCommands` | bool | false | falsy | Allow the auto-execute row for `isDangerous` matches |
| `autocomplete.immediatelyRunGitAliases` | bool | true | – | Listed in the UI ("Safe git aliases") but **not read anywhere in the autocomplete code** (probably consumed by the git spec or unused) |
| `autocomplete.firstTokenCompletion` | bool | false | falsy | Complete the command name itself (§6.3) |
| `autocomplete.hideAutoExecuteSuggestion` | bool | false | false | Never show auto-execute rows |
| `autocomplete.navigateToHistory` | bool | – | falsy | Up at index 0 toggles history mode instead of hiding |
| `autocomplete.alwaysShowDescription` | bool | – | falsy | Permanent description pop-out; disables toggleDescription and the hint |
| `autocomplete.theme` | string | "system" (UI list) | **unset → dark** (`setTheme` throws on a missing name and falls back to dark) | §13 |
| `autocomplete.height` | number px | 140 | 140 | Max menu height |
| `autocomplete.width` | number px | 320 | 320 (+50 in history modes) | Menu width |
| `autocomplete.fontFamily` | string | null | Monaco, Hack, monospace | |
| `autocomplete.fontSize` | number px | null | 12.8 | Also scales row height ×1.5625 |
| `autocomplete.keybindings.<key>` | string (action id or `ignore`) | – | – | §3.2 |
| `autocomplete.history.disableLoading` | bool | – | false | Skip loading shell history at startup |
| `autocomplete.developerMode`, `autocomplete.developerModeNPM`, `autocomplete.developerModeNPMInvalidateCache`, `autocomplete.devCompletionsFolder`, `autocomplete.devCompletionsServerPort` | dev | – | – | Load specs from disk or a dev server and disable the spec cache. `developerModeNPM` shows a yellow "Developer mode enabled!" banner (`App.tsx:400-433`) |
| `autocomplete.personalShortcutsToken` | string | – | "+" | Legacy Fig shortcuts (out of scope) |
| `autocomplete.hidePreviewWindow`, `autocomplete.userStyles`, `autocomplete.iconTheme` | – | – | – | Declared or commented out; unused in this package (`iconTheme` is only used by Linux desktop icons) |
| `beta.autocomplete.auto-cache` | bool | – | false | Cache all generators SWR 1 s |
| `beta.history.mode` | `"off"`\|`"history_only"`\|`"show"` | – | effectively off | §6.2 |
| `beta.history.customCommand` | string | – | – | Command whose stdout lines are the history |
| `beta.history.allShells` | bool | – | – | Merge zsh and bash history for bash/zsh |
| `beta.history.ctrl-r` | – | – | – | Declared, unused |

Settings are re-read live. The UI subscribes to `Settings.didChange` and calls `updateSettings` (`AC/fig/hooks.ts:67-89`). The exception is `onlyShowOnTab` inside figterm, which is a one-time `LazyLock`.

---

## 13. Themes

### 13.1 Built-ins (`AC/fig/themes.ts:9-44`)

v1 format:
```jsonc
// dark
{ "version": "1.0", "theme": {
  "backgroundColor": "rgb(48,48,48)", "textColor": "rgb(180,180,180)", "matchBackgroundColor": "rgb(95,89,56)",
  "selection":   { "textColor": "rgb(253,253,253)", "backgroundColor": "rgb(30,90,199)", "matchBackgroundColor": "rgb(106,142,218)" },
  "description": { "textColor": "rgb(180,180,180)", "borderColor": "rgb(65,65,65)" } } }
// light
{ "version": "1.0", "theme": {
  "backgroundColor": "rgb(254,254,254)", "textColor": "rgb(7,7,7)", "matchBackgroundColor": "rgb(255,239,152)",
  "selection":   { "textColor": "rgb(253,253,253)", "backgroundColor": "rgb(41,105,218)", "matchBackgroundColor": "rgb(106,142,218)" },
  "description": { "textColor": "rgb(7,7,7)", "borderColor": "rgb(199,199,199)" } } }
```

### 13.2 Theme file schema (`themes.ts:85-124`)

Optional `author{name, twitter?, github?}`, `version` string, optional `theme{...v1 as above; selection.matchBackgroundColor optional}`, and optional base16-style `shade0..shade7` and `accent0..accent7` strings.

### 13.3 Resolution (`setTheme`, `themes.ts:282-317`)

- Unset or empty → dark, via the error path.
- `"system"` → the built-in matching the OS dark/light preference (`prefers-color-scheme`). For a terminal **(inferred)**: query the background with OSC 11 / `COLORFGBG`, or default to dark.
- `"dark"` or `"light"` → the built-in.
- Else read `<themesDir>/<name>.json`. `themesDir` = `<resources>/themes`: macOS app bundle Resources, Linux `/usr/share/<pkg>/themes`, Windows `<data>/resources/themes` (`Q/crates/fig_util/src/directories.rs:277-279, 391-415`).
  - JSON.parse it and shallow-merge `{...dark, ...parsed}`, then apply it with filler = dark.
  - Any error → dark.
  - Pitfall: a shade-only theme without `version`/`theme` inherits `version:"1.0"` and dark's `theme`, so dark main colors win. Only shade2-4 and accents come from the file.

The UI's theme list also names `cobalt, cobalt2, dracula, github-dark, gruvbox, monokai-dark, nightowl, nord, panda, poimandres, the-unnamed, synthwave-84, solarized-light, solarized-dark` (`dashboard-app/src/data/autocomplete.ts:157-181`). **Those JSON files are not in this repo**; they ship in the resources `themes/` folder. The `q theme` CLI treats only `dark`, `light`, `system` as built in (`Q/crates/q_cli/src/cli/theme.rs:23-25`).

### 13.4 Mapping to the 8 UI colors (`setCSSProperties`, `themes.ts:161-280`)

If `version=="1.0"` && `theme`:
- `main-bg`=backgroundColor, `main-text`=textColor, `matching-bg`=matchBackgroundColor
- `description-text`, `description-border` from `description`
- `selected-bg`, `selected-text` from `selection`
- `selected-matching-bg`=selection.matchBackgroundColor ?? "rgb(106, 142, 218)"
- shade0=main-bg, shade1=desc-border, shade2..4 from the file or the filler, shade5=desc-text, shade6=main-text, shade7=selected-text; accents from the file or the filler.

Else (base16 style):
- shades and accents from the file or the filler;
- main-bg=shade0, matching-bg=shade1, desc-border=shade1, selected-bg=shade2, desc-text=shade5, main-text=shade6, selected-text=shade7, selected-matching-bg=accent6.

Filler (`themes.ts:46-83`):
- dark shades `#181818 #282828 #383838 #585858 #b8b8b8 #d8d8d8 #e8e8e8 #f8f8f8`
- light shades: the same list reversed
- accents (both): `#ab4642 #dc9656 #f7ca88 #a1b56c #86c1b9 #7cafc2 #ba8baf #a16946`

Color parsing (`mapColor`, `themes.ts:130-159`):
- `#rrggbb` or `#rrggbbaa`. Alpha is **premultiplied against black**: `r*a g*a b*a`, with non-integer results possible.
- `rgb(r,g,b)` **without spaces** (regex `/rgb\((\d+),(\d+),(\d+)\)/`).
- Anything else is passed through with a warning. That includes `#rgb`, `rgb()` with spaces (so the default `"rgb(106, 142, 218)"` fallback itself fails to parse) and named colors.
- Recommendation: accept `#rgb`, `#rrggbb`, `#rrggbbaa`, and `rgb()` with optional spaces; render as 24-bit SGR `38;2;r;g;b` / `48;2;r;g;b`, with a 256-color fallback **(inferred)**.

---

## 14. Misc behaviors worth porting

- Aliases are expanded before parsing. From the shell's `alias` output: lines stripped of `alias `, split on `=` (on a space for fish), surrounding single quotes stripped (`AC/state/index.ts:78-96`). Up to one level per alias name, loop-protected (`shell-parser/src/command.ts:158-187`).
- The `clear-cache` event clears the generator cache and the spec index (`AC/fig/hooks.ts:138-148`).
- The app reloads every 24 h (`AC/main.tsx:11-17`). Not needed.
- Telemetry events `autocomplete-insert` and `autocomplete-no-context` are out of scope.

---

## 15. Known bugs and quirks in the reference (decide whether to replicate)

1. `selectSuggestionN` is a no-op (NaN parse) — §3.4.
2. `showDescription` / `hideDescription` actions are not handled.
3. The deletion count escapes only the first whitespace (`/\s/` without `g`). With multiple escaped spaces in the token, too few backspaces are sent. It also ignores quotes the user typed: the token `innerText` drops them, so `'my fi` + insert leaves a stray `'`. **(inferred)**
4. The prefix highlight builds a RegExp from the unescaped query. Queries with `(`, `[`, `+`, `*`… either throw (and fall back to no highlight) or mis-highlight.
5. Recency is written under `tokens[commandIndex]` but read under `tokens[0]`.
6. `priority: 0` is treated as 50 (`s.priority || 50`).
7. The spinner replaces the whole list. The inline spinner is dead code.
8. Shade-only custom themes are mostly overridden by dark (§13.3). `mapColor` rejects spaced `rgb()`.
9. `toggleFuzzySearch` takes effect only on the next parser result and resets per argument.
10. In `HIDDEN_UNTIL_KEYPRESS` suggestions are not recomputed. With `onlyShowOnTab` a stale `suggestions.length` gates Tab interception.
11. The fish first-token generator includes the tab char in names.
12. The two default keybinding tables differ (fig.json vs actions.json) — §3.1.

---

## 16. Minimal Rust model (suggested shape, **inferred**)

```rust
enum Visibility { Visible, HiddenUntilKeypress, HiddenUntilShown, HiddenByInsertion }
struct MenuState {
    visibility: Visibility, suggestions: Vec<Suggestion>, selected: usize, has_changed_index: bool,
    scroll_top: usize, last_inserted: Option<SuggestionKey>, just_inserted: bool,
    history_mode: bool, user_fuzzy: bool, fuzzy: bool, description_popout: bool,
    size_scale: f32,           // increase/decreaseSize (×1.1 steps), session-only
}
// on every buffer change: parse -> on_parser_result(..) (§2.2) -> maybe_recompute(..) (§2.4, §7)
// keys intercepted iff visibility==Visible && !suggestions.is_empty() && !command_running (§4)
// global keys (toggle/show + Tab when only_show_on_tab) iff !suggestions.is_empty()
// Esc: swallow, set HiddenUntilShown, stop intercepting immediately
// insertion: write "\x08"*n + text + "\x1b[D"*k to PTY master with buffer reconciliation (§5.2)
```
Testing anchors: the existing unit tests in `AC/suggestions/tests/*.test.ts` and `AC/fig/tests/theme.test.ts`, plus `interceptor.rs` and `keybindings.rs` tests. Their cases can be ported as Rust `#[test]`s.
