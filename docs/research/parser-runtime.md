# Completion engine: tokenizer, spec loading, argv parser, suggestions, generators, executeCommand

This is the spec for tabsmith's completion engine. It is reconstructed from Amazon Q / Kiro's TypeScript engine (the reference) and from Microsoft inshellisense's runtime (a second, simpler implementation).

## 0. Conventions and sources

Path prefixes used in citations (all read-only, under `.../scratchpad/ref/`):

| Prefix | Real path |
|---|---|
| `SP/` | `qac/packages/shell-parser/src/` |
| `AP/` | `qac/packages/autocomplete-parser/src/` |
| `AC/` | `qac/packages/autocomplete/src/` (byte-identical to `qac/packages/autocomplete-app/src/` for every file cited here) |
| `SH/` | `qac/packages/shared/src/` |
| `ABW/` | `qac/packages/api-bindings-wrappers/src/` |
| `AB/` | `qac/packages/api-bindings/src/` |
| `FZ/` | `qac/packages/fuzzysort/fuzzysort.js` |
| `CR/` | `qac/crates/` |
| `ISH/` | `ish/src/runtime/` |
| `SPECS/` | `specs/package/build/` (compiled `@withfig/autocomplete@2.692.3`, minified, one line per file, so there are no line numbers) |

Markers:
- **(inferred)** means a behavior that is not on disk and is reconstructed from usage plus knowledge of the upstream package.
- **(rec)** means a recommendation for tabsmith, not a description of either reference.

Three npm packages the qac engine depends on are **not on disk**:
- `@fig/autocomplete-shared`, which provides `convertSubcommand` and `initializeDefault`.
- `@fig/autocomplete-helpers`, which provides `getVersionFromVersionedSpec`.
- `@withfig/autocomplete-types`, which declares the `Fig.*` types.

Two partial substitutes exist:
- `@fig/autocomplete-generators` (`filepaths`, `folders`, `keyValue*`) is bundled verbatim into many compiled specs, for example `SPECS/apt.js`.
- The diff/merge half of `autocomplete-helpers` is bundled into every diff-versioned `index.js`, for example `SPECS/heroku/index.js`.

Both are decoded below.

High-level pipeline in qac (per keystroke):

```
edit buffer + cursor
  -> slice buffer to cursor; abort if char under cursor is non-space         (AC/state/index.ts:486-491)
  -> getCommand(bufferSliced, aliases)  [shell-parser]                        (SP/command.ts:189-202)
  -> parseArguments(command, shellContext)  [autocomplete-parser]             (AP/parseArguments.ts:1088-1144)
       loads specs (CDN / local), walks tokens through a state machine,
       returns { completionObj, currentArg, passedOptions, searchTerm,
                 suggestionFlags, annotations, commandIndex }
  -> triggerGenerators(parserResult)  (re-run per trigger rules)              (AC/state/generators.ts:118-198)
  -> getAllSuggestions + history merge + priority sort + filterSuggestions    (AC/state/index.ts:98-187)
```

---

## 1. Tokenizing the command line (qac `shell-parser`)

### 1.1 Grammar (`SP/parser.ts:1-23`)

```
terminator     = ";" | "&" | "&;"
literal        = string | ansi_c_string | raw_string | expansion | simple_expansion | word
concatenation  = literal literal
command        = (concatenation | literal)+
assignment     = (word | subscript)("=" | "+=")literal
assignment_list= assignment+ command?
statement      = "{" (statement terminator)+ "}" | "(" statements ")"
               | statement "||" statement | statement "&&" statement
               | statement "|" statement  | statement "|&" statement
               | command | assignment_list
statements     = (statement terminator)* statement terminator?
program        = statements
```

### 1.2 Node model (`SP/parser.ts:25-107`)

`NodeType` string values:
- Structural: `program`, `assignment_list`, `assignment`, `variable_name`, `subscript`, `compound_statement`, `subshell`, `command`, `pipeline`, `list`.
- Unused: `process_substitution`.
- Literals: `concatenation`, `word`, `string`, `expansion`, `command_substitution`, `raw_string`, `ansi_c_string`, `simple_expansion`, `special_expansion`, `arithmetic_expansion`.

Every node has the following fields:

```ts
{ text: string /* str.slice(startIndex,endIndex) */, innerText: string /* unquoted/unescaped */,
  startIndex, endIndex /* exclusive */, complete: boolean, type, children: Node[] }
```

The node types add these fields:
- `AssignmentList`: `hasCommand`.
- `Assignment`: `operator: "=" | "+="` and `name: VariableName | Subscript`.
- `List`: an `operator` field is declared but never set.

`createNode(str, partial)` (`SP/parser.ts:159-176`) works as follows:
1. Default to `{startIndex:0,type:word,endIndex:str.length,complete:true,children:[]}`.
2. Set `text = str.slice(start,end)`.
3. Set `innerText = getInnerText(node)`.

### 1.3 `innerText` (unquoting) (`SP/parser.ts:124-157`)

- `concatenation` is the concatenation of the children's `innerText`.
- Delimiters to strip: `string` `"`…`"`, `raw_string` `'`…`'`, `ansi_c_string` `$'`…`'`. All other types strip nothing.
- The end delimiter is stripped **only if `complete`**, so for an unterminated `"foo` the innerText is `foo`.
- Escapes, applied while walking the remaining chars:
  - In a `word`, every `\` is dropped and the next char is taken literally. So `a\ b` gives `a b`, and a trailing lone `\` is dropped.
  - In a `string`, `\` is dropped only if the next char is one of `` $ ` " \ `` or newline.
  - `raw_string`, `ansi_c_string` (no ANSI-C decoding is done), and expansions/substitutions keep their text verbatim.

### 1.4 Operators (`SP/parser.ts:109-122`)

`parseOperator(str,i)`: if `str[i]` is one of `& ; |`, take the 2-char slice. If it is one of `; & &; | |& && ||`, return it; otherwise return the single char. Otherwise return `null`.

These chars **always** terminate a word inside a command, even without surrounding spaces. `a&&b` is two commands.

### 1.5 Literal parsing

`childAtIndex(str,i,inString,terminators)` (`SP/parser.ts:277-311`) dispatches on the char at `i`. It uses up to 3 chars of lookahead.

| lookahead | node | parser |
|---|---|---|
| `$((` | `arithmetic_expansion` | `parseLiteral("$((", "))")`, with no children |
| `$(` | `command_substitution` | `parseCommandSubstitution(str,i,")")` |
| `${` | `expansion` | `parseLiteral("${","}")`, which can have children |
| `$'` (only when not inside a `"` string) | `ansi_c_string` | `parseLiteral("$'","'")` |
| `$` otherwise | `simple_expansion` / `special_expansion` / `null` | `parseSimpleExpansion` |
| `` ` `` | `command_substitution` | `parseCommandSubstitution(str,i,"`")` |
| `'` (not in string) | `raw_string` | `parseLiteral("'","'")` |
| `"` (not in string) | `string` | `parseLiteral('"','"')`, which can have children |

`parseLiteral(type,start,end)` (`SP/parser.ts:313-348`) scans from `startIndex+start.length`:
- If the type can have children (`string`, `expansion`), try `childAtIndex(…,[end])` first and jump past any child found.
- Else, if the char is `\` and the type is not `raw_string`, skip the next char.
- Else, if `str.slice(i,i+end.length)===end`, finish with `complete=true`.
- On EOF, return the node with `complete:false` and `endIndex = str.length`.

`parseSimpleExpansion` (`SP/parser.ts:194-230`):
- If the next char is one of `* @ ? - $ 0 _`, return a `special_expansion` of length 2.
- Otherwise read until one of `\t`, space, `\n`, `$`, `\`, or the caller's terminators.
- If zero chars were read (a lone `$`), return `null`, so the `$` becomes part of a word.

`parseCommandSubstitution` (`SP/parser.ts:233-253`):
- It recursively calls `parseStatements(str, i+1|i+2, terminalChar)`.
- `complete = terminated && children.length>0`.
- `endIndex = terminator+1`, or `str.length` if unterminated.

### 1.6 Words and arguments (`SP/parser.ts:404-477` `parseConcatenationOrLiteralNode(str,start,terminalChar)`)

Loop over chars from `start`:
1. If `parseOperator(str,i)!==null` or `c===terminalChar`, **stop** (do not consume).
2. `child = childAtIndex(str,i,false,["&","|",";","\n","'",'"',"`",terminalChar?])`. If there is a child, end any current word, push the child into the current argument, and jump `i` to `child.endIndex-1`.
3. Else, if `c` is space or tab, `endArgument(i)`.
4. Else, if `c==="\\"`, set `i+=1` (this is how `\ ` keeps a space inside a word). Start a word at `i` if none is started.

At the end, call `endArgument(i)`.

`endArgument`:
- If the argument has more than one part, wrap the parts in a `concatenation` whose `complete` is the last part's `complete`.
- A single part is pushed as is.

The function returns `{children, endIndex:i}`. So `--foo="a b"c` is **one** argument: a concatenation of word `--foo=`, string `"a b"`, and word `c`.

Newline is **not** a word separator here, only a terminator for `$var`.

### 1.7 Commands and assignments

`parseCommand(str, idx, terminalChar)` (`SP/parser.ts:479-499`):
- `startIndex = max(nextNonSpace(idx), idx)`.
- Parse the children as in §1.6.
- `complete = children.length>0`.
- `endIndex = children.length>0 ? endIndex : str.length`.

The command's `text` **includes trailing whitespace** up to the operator or EOF. For example, the command in `"git "` has `text "git "`.

`parseAssignments` (`SP/parser.ts:556-570`):
- Repeat while the remaining string (from the next non-space position) matches `/^[\w[\]]+\+?=.*/`.

`parseAssignmentNode` (`SP/parser.ts:501-554`):
- The operator is `+=` if the char before `=` is `+`.
- The name is a `variable_name`, or a `subscript` if a `[` occurs before the operator. For a subscript, the index word lies between `[` and the char before `=`.
- The value is `parseConcatenationOrLiteralNode(str, eq+1, " ")`, so a space terminates it while quotes and substitutions are honoured.
- `complete = lastChild.complete`.
- **Quirk:** an empty value (`FOO= cmd`) has zero children, so this line throws a `TypeError`. The engine then has no command and shows nothing. (rec: treat it as complete.)

`parseAssignmentListNodeOrCommandNode` (`SP/parser.ts:572-602`):
- If there are assignments and the next non-space char after the last assignment is not an operator, the last assignment is complete, and it is not at EOF, then parse a command after it.
- The result is an `assignment_list{hasCommand, children:[...assignments, command?]}`.

Consequences:
- `FOO=bar git ` is a command `git ` (tokens `git`, `""`).
- `FOO=bar` alone has no command, so there are no suggestions.
- `FOO=bar ` gives an empty command with tokens `[""]`, which gets first-token completion.

### 1.8 Statements (`SP/parser.ts:350-402, 604-705`)

`parseStatement(str,i,terminalChar)`:
1. Skip whitespace.
2. If the char is `{` or `(`, call `parseStatements(str,i+1,"}"|")", mustTerminate=isCompound)`. Build a `compound_statement` or `subshell` with `complete = terminated && hasChildren`. Unterminated nodes extend to the last child's end, or to EOF.
   - Because `mustTerminate` is set for `{`, inner commands do not stop at `}`. `{a; b}` therefore makes `b}` a word, while `{ a; b; }` terminates correctly (bash semantics).
3. Else, call `parseAssignmentListNodeOrCommandNode`.
4. Look at the next operator:
   - If there is none, or it is `;`, `&`, `&;`, or the terminalChar, return the statement.
   - Else, parse the right-hand statement recursively.
     - `&&` / `||` reduce to `list`, flattened: if the right-hand side is already a list, its children are spliced in.
     - `|` / `|&` reduce to `pipeline`. If the right-hand side is a `list`, the pipeline binds tighter: `Pipeline(lhs, rhs.children[0])` replaces the list's first child.

`parseStatements` loops `parseStatement`:
- If the next non-space char is the terminalChar (and the call is not `mustTerminate`), return `terminatorIndex`.
- At EOF, push and stop.
- On a terminator op (`; & &;`), consume it. If the next non-space char is the terminalChar, return.
- If there is no op but tokens remain (for example `(a) b`), mark the statement `complete=false` (assignment lists keep their own flag) and continue from there.

`parse(str)` returns a `program` whose children are the top-level statements (`SP/parser.ts:725-730`).

### 1.9 Selecting the current command and building tokens (`SP/command.ts`)

`getCommand(buffer, aliases, cursorIndex?)` (`SP/command.ts:189-202`):
1. `index = cursorIndex ?? buffer.length`.
2. Parse.
3. `descendantAtIndex(tree, index, "command")` (`SP/command.ts:21-36`) finds the **deepest** node of type `command` with `startIndex <= index <= endIndex` (both inclusive), checking children in order, first hit wins.
   - This handles commands inside `$(…)`, backticks, `"…$(…)"`, subshells, pipelines, and lists.
4. If none is found, return `null`.
5. Otherwise call `convertCommandNodeToCommand`, then `expandCommand` (aliases).

`AC/state/index.ts:486-499` passes `buffer.slice(0,cursor)` and the cursor. If the char *at* the cursor is non-whitespace (the cursor is mid-word), the engine resets to the empty state and shows nothing.

`convertCommandNodeToCommand(tree)` (`SP/command.ts:67-91`):
- It throws if the node is not a `command`.
- `tokens = children.map(c => ({ originalNode:c, node:c, text:c.innerText }))`. `token.text` is the **unquoted** value.
- **Trailing empty token:** if `(children.length===0 ? 1 : lastChild.endIndex) < tree.endIndex && tree.text.endsWith(" ")`, push `createTextToken(cmd, endIndex, "")`.
  - This is how "user typed a space, now completing a new empty argument" is represented. `git ` gives `["git",""]`; `git` gives `["git"]`.
  - Quirk: a buffer of exactly one space gives `[]` (no suggestions). Two spaces give `[""]`.
- `Command = { tokens, tree, originalTree }`.

Worked examples:

| buffer (cursor at end) | tokens |
|---|---|
| `git co ` | `git`, `co`, `""` (`SP/../tests/command.test.ts:14`) |
| `ls \| gr` | `gr` |
| `ls \| ` | `""` |
| `ls \|` | `[]`, so no suggestions |
| `echo $(git ch` | `git`, `ch` |
| `cd my\ di` | `cd`, `my di` |
| `git commit -m "fix bu` | `git`,`commit`,`-m`,`fix bu` (unterminated string) |
| `FOO=1 BAR=2 npm ` | `npm`, `""` |
| `(cd x && git ` | `git`, `""` |

`getTopLevelCommands` / `getAllCommandsWithAlias` (`SP/command.ts:204-236`) return all top-level commands. They are used only for history indexing.

### 1.10 Alias expansion (`SP/command.ts:100-187`, `AC/state/index.ts:78-96`)

Alias map source: the shell integration reports the `alias` command output string (`shellContext.alias`). The parsing works as follows:
1. Strip `^alias\s` (multiline).
2. Split on `\n`.
3. For each line, `[key,...value] = line.split(sep)`, where `sep` is `" "` if the shell path contains `fish` and `"="` otherwise.
4. `value = value.join(sep)` with one leading `'` and one trailing `'` stripped.

`expandCommand(command, _, aliases)`:
- It loops while `tokens.length > 1` (that is, the user has typed past the first word), `aliases[first.text]` exists, and that alias has not been used yet.
- Each iteration:
  1. Compute `aliasValue = alias.replace(/^'(.*)'$/,"$1")`.
  2. Try `substituteAlias`; errors are swallowed.
  3. Mark the alias as used, then re-read the first token.
- Chains such as `g`→`git` expand. Self-referencing aliases (`ls='ls -G'`) expand once.
- Tests are in `qac/packages/shell-parser/tests/command.test.ts:24-33`:
  - `woman` stays `woman`, because there is a single token.
  - `woman ` becomes `man ""`.
  - `man woman ` is unchanged, because only the first token is expanded.
  - `quote ` (alias `'q'`) becomes `q ""`.

`substituteAlias(command, token, alias)`:
1. Rebuild the buffer as `textBefore(token) + alias + textAfter(token)` (relative to `tree`).
2. Re-parse and shift every index by `tree.startIndex`.
3. Require exactly one top-level statement that is a `command`. Aliases containing `|`, `&&`, or `;` therefore fail (they become a pipeline or list) and are **not** expanded.
4. Re-map `originalNode`: new tokens that fall inside the alias span all map to the alias token's original node. Later tokens map 1:1, offset by the number of extra alias tokens.
5. If the count check fails, throw.

The same function is reused by the argv parser for `parserDirectives.alias` (§3.9).

### 1.11 Second implementation: inshellisense lexer (`ISH/parser.ts`)

This lexer is much simpler and should not be copied blindly.

1. `parseCommand(cmd, shell)`: `cmd.split(/(\|\|)|(&&)|(;)|(\|)/).at(-1).trimStart()`. This is **not quote-aware**: `echo "a|b` splits inside the quotes.
2. `lex` (`ISH/parser.ts:59-154`) is a char state machine with the states quoted, quote-continued, flag, and cmd.
   - A token starting with `'`, `"`, or `` ` `` is quoted. If the closing quote is immediately followed by a non-space, it becomes "quote-continued" (`"a"b`).
   - A token starting with `-` is a flag.
   - Flags end at whitespace **or at `=`**. `--opt=val` gives tokens `--opt` (an option, complete) and `val`.
     - Quirk: this `char === "="` branch also fires while reading a plain word, which creates a duplicate token for `FOO=bar`.
   - A word ends at whitespace not preceded by the shell's escape char.
   - `complete` is true when the token is followed by whitespace.
   - Each token carries `tokenLength`, a display width (wcwidth), so the UI can backspace it.
3. Post-processing:
   - `unescapeSpaceTokens` turns `\ ` into space (the escape char is `` ` `` for pwsh and `^` for cmd, per `ISH/utils.ts:71-85`).
   - `unwrapQuotedTokens` strips the quotes from quote-continued tokens.
4. Aliases (`ISH/alias.ts`):
   - Loaded once at start via `bash -i -c alias` (with git-bash on Windows) or `zsh -i -c alias`. Values are parsed into tokens.
   - `aliasExpand` replaces the first token if it is complete (no recursion).
5. There is no subshell, `$( )`, or env-assignment awareness.

(rec) tabsmith should port qac's `shell-parser` 1:1. It is about 900 lines of TypeScript, self-contained, and well defined, and it gives correct current-command selection for pipes, lists, subshells, substitutions, and env prefixes. For PowerShell, `cmd`, and nushell, tabsmith will need a separate simplified lexer; ish's escape-char table is a good reference.

---

## 2. Spec loading

### 2.1 Spec name to location (`AP/loadSpec.ts:61-106`, `getSpecPath(name, cwd, isScript?)`)

`SpecLocation = {type:"global", name} | {type:"local", name, path?}`. qac adds `diffVersionedFile?: string` (`SH/internal.ts:4-6`).

Rules, applied in order:

1. `name === "?"`: `{type:local, name:"_shortcuts", path: searchFigFolder(cwd)}`. This runs `bash -c 'until [[ -f .fig/autocomplete/build/_shortcuts.js ]] || [[ $PWD = $HOME ]] || [[ $PWD = "/" ]]; do cd ..; done; echo $PWD'` in cwd (`AP/loadSpec.ts:35-52`). (rec: drop it.)
2. `name === (setting autocomplete.personalShortcutsToken || "+")`: `{local, name:"+", path:"~/"}`.
3. `[path, basename] = splitPath(name)` (split after the last `/`; the dirname keeps its trailing `/`; `SH/utils.ts:178-181`).
4. If `!isScript`:
   - If `isScript === undefined` (this is the **first token** of a command):
     - If `name==="bin/console"` or it ends with `/bin/console`, return `{global,"php/bin-console"}`.
     - If the path contains no `/`, return `{global, name}`.
   - Else (`isScript === false`, from an `isCommand` arg): if the path does **not** start with `/`, `./`, or `~/`, return `{global, name}`. So `@withfig/autocomplete-tools` stays global.
5. Otherwise the location is local:
   - If the path starts with `/` or `~/`, return `{local, name:basename, path}`.
   - Otherwise return `{local, name:basename, path:`${cwd}/${path minus leading "./"}`}`.

Expected results from the tests (`qac/packages/autocomplete-parser/tests/loadSpec.test.ts:48-123`, `cwd="test_cwd"`):

| input | result |
|---|---|
| `git` | global `git` |
| `@withfig/autocomplete-tools` (isScript=false) | global, full name |
| `@withfig/autocomplete-tools` (first token) | local `autocomplete-tools` at `test_cwd/@withfig/` |
| `./test` | local `test` at `test_cwd/` |
| `~/dir/test` | local `test` at `~/dir/` |
| `/dir/test` | local `test` at `/dir/` |

`serializeSpecLocation` (`AP/loadSpec.ts:54-59`) produces `global://name=<n>` or `local://path=<p>&name=<n>`. It is used in cache keys.

### 2.2 Loading a location (`AP/loadSpec.ts:112-197` `importSpecFromLocation`)

The first source that works wins:

1. **Dev server.** Only if `isInDevMode()` and `autocomplete.devCompletionsServerPort` is a string or number. Import `http://localhost:<port>/<name>[/<diffVersionedFile>].js` with a 20 s timeout. Errors are *not* caught.
   - `isInDevMode()` means `autocomplete.developerMode` or `autocomplete.developerModeNPM` is truthy (`ABW/settings.ts:66-71`).
2. **Dev folder.** Only if in dev mode and `autocomplete.devCompletionsFolder` is set. Try `importSpecFromFile(name or name/diffFile, devPath)`. Errors are swallowed.
   - `importSpecFromFile(name, dir)` (`AP/loadHelpers.ts:55-78`) reads `${dir}/${name}.js`, falling back to `${dir}/${name}/index.js`. It evaluates the text as an ES module via a Blob URL and caches modules by source text (`importString`, `AP/loadHelpers.ts:36-49`).
3. **Local location.** `[dir, base] = splitPath((path || "~/") + name)`, then `importSpecFromFile(base, dir + ".fig/autocomplete/build/")`. Errors propagate.
4. **Global location:**
   - If `publicSpecExists(name)` (the name is in CDN `index.json` `completions`), import from the CDN as `<name>[/<diffVersionedFile>]` (§2.3).
   - Else try `importSpecFromFile(name, "~/.fig/autocomplete/build/")`. Errors are swallowed. This "private/user spec" folder applies to globals too.
5. If nothing loaded, throw `MissingSpecError("No spec found")`.

The `ResolvedSpecLocation` type `{type:"private", namespace, name}` is declared but never produced.

"Private specs" in practice means one of two things:
- **(a)** A second CDN reachable only with Amazon Midway auth (`CR/fig_desktop/src/protocol/spec.rs:58-73`), whose `index.json` is merged with the public one.
- **(b)** Local files in `.fig/autocomplete/build/`.

### 2.3 CDN format and versions

- Base URL: `https://specs.q.us-east-1.amazonaws.com/`. Spec URL: `${base}${name}.js`. Index: `${base}index.json` (`AP/loadHelpers.ts:27-34`).
- When running inside the desktop webview (protocol is not `https:`), the import goes through `spec://localhost/<name>.js` and `spec://localhost/index.json` instead (`AP/loadHelpers.ts:83-97,119-122`). The Rust handler (`CR/fig_desktop/src/protocol/spec.rs:196-248`):
  - Merges the indexes of all CDNs: union of the sets, then sorted (lines 167-193).
  - For `<name>.js`, it picks the first non-default CDN whose `completions` contains the name (binary search), else the public CDN. It proxies the response with `Content-Type` `application/javascript` by default.
- Direct HTTPS mode uses `exponentialBackoff({attemptTimeout:1000, baseDelay:500, maxRetries:5, jitter:100})` (`AP/loadHelpers.ts:101-116`; algorithm at `SH/utils.ts:224-246`):
  - Each attempt is `withTimeout(1000)`.
  - On failure: `delay = delay*2 + floor(rand*jitter)`, sleep, and retry, up to 5 attempts in total.
  - After that, throw `SpecCDNError`.
- `index.json` schema (validated with zod, `AP/loadHelpers.ts:204-223`): `{ completions: string[], diffVersionedCompletions: string[] }`. It is fetched once and memoized as a promise. On any failure it resolves to empty sets and is reset, so the next call retries.
  - The `clear-cache` event resets it via `clearSpecIndex()` (`AC/fig/hooks.ts:138-148`).
- The local equivalent shipped in the npm package is `SPECS/index.js`: `export default ["-","@capgo/cli",…,"zoxide"]` (1484 entries incl. `aws/s3`, `az`, `dotslash`, `php/bin-console`, `python/http.server`), `export const diffVersionedCompletions = ["@usermn/sdc","az","fig","heroku","infracost","shopify"]` (`SPECS/index.d.ts`).
  - `SPECS/dynamic/index.js` exports a map `{ "<name>": () => import("../<name>.js"), … }`. This is the "dynamic" (lazy-import) form of the same list; it is exposed as the package export `./dynamic`.
- `MOST_USED_SPECS` are preloaded (`AP/constants.ts:1-33`, `AP/loadHelpers.ts:235-246`): `aws q cd git rm ls cat mv ssh cp chmod source curl make mkdir man ln grep kill code nano vi vim nvim node npm npx yarn brew open docker python`. For a diff-versioned name, `<name>/index` is preloaded.

(rec) tabsmith should ship `SPECS/` embedded or unpacked under its data dir and resolve `name` to `<specs>/<name>.js`, using `index.js` as the existence index. It should then layer the user overrides in this order:
1. The dev folder (setting `autocomplete.devCompletionsFolder`).
2. `~/.tabsmith/autocomplete/build/` (equivalent of `~/.fig/autocomplete/build/`).
3. Per-project `.fig/autocomplete/build/` for local/script specs.

### 2.4 Per-spec caching, disabling, and timeouts (`AP/loadSpec.ts:199-245`)

`loadSubcommandCached(location)`:
1. If `location.name` is in the setting `autocomplete.disableForCommands` (string[]), throw `DisabledSpecError`.
2. `key = [type, path||"", name].join(",")`.
3. If `autocomplete.developerModeNPMInvalidateCache` is set, clear the cache and reset the setting to false. Else, if not `developerModeNPM` and the key is cached, return the cached value.
4. `withTimeout(5000, loadFigSubcommand(location))`, then `convertSubcommand(spec, initializeDefault)` (§2.7), then cache.

`loadFigSubcommand`:
- If the name is diff-versioned (per the index), load with `diffVersionedFile:"index"`, i.e. `<name>/index.js`.
- Call `tryResolveSpecToSubcommand(specFile, originalLocation)`.

Caches are `Map`s registered in a global list (`AP/caches.ts`). `resetCaches()` clears all of them: `specCache`, `generateSpecCache`, and the parse caches.

### 2.5 Versioned specs (`AP/tryResolveSpecToSubcommand.ts:8-43`, `AP/loadHelpers.ts:149-190`)

A spec module is either `{default: Subcommand}` or `{default: (version?) => …, getVersionCommand?}`.

If `default` is a function:
1. Compute the version with `getVersionFromFullFile(spec, name)`:
   - The memo key is `cliVersion-<name>`, an in-memory map. It is bypassed in dev mode.
   - If `getVersionCommand` is exported, `version = await getVersionCommand(executeCommand)`. Example: heroku runs `heroku --version` and applies `/heroku\/([0-9]+\.[0-9]+\.[0.9]+)/` (sic), defaulting to `"8.0.0"`. az parses `az version -o json` and reads `["azure-cli"]`.
   - Else, `semver.clean((await executeCommand({command:name, args:["--version"]})).stdout)`.
   - Any error gives `undefined`.
2. `r = await default(version)`.
   - If `r` has no `versionedSpecPath`, it is the Subcommand (a "simple versioned" spec).
   - If `r = {versionedSpecPath, version}` (a **diff-versioned** spec):
     1. `[dir, base] = splitPath(versionedSpecPath)`, e.g. `"heroku/8.6.0"` → `["heroku/","8.6.0"]`.
     2. Load `{…location, name: dir minus trailing "/", diffVersionedFile: base}`, i.e. `heroku/8.6.0.js`.
     3. That module must export `default` (the base Subcommand) and `versions` (a `VersionDiffMap`). Return `getVersionFromVersionedSpec(default, versions, version).spec`.
     4. Otherwise throw `WrongDiffVersionedSpecError`.

`createVersionedSpec(specName, versionFiles)` is decoded from the minified `SPECS/heroku/index.js` / `SPECS/az/index.js` (`pt`/`Si`):

```js
const pickIndex = (sorted, target) => {            // Si
  if (!target) return sorted.length - 1;
  for (let i = sorted.length - 1; i >= 0; i--) if (semver.compare(sorted[i], target) <= 0) return i;
  return sorted.length - 1;                          // nothing <= target -> newest
};
createVersionedSpec = (specName, versionFiles) => async (version) => {
  const sorted = versionFiles.sort(semver.compare);
  return { versionedSpecPath: `${specName}/${sorted[pickIndex(sorted, version)]}`, version };
};
```

The version file format is `export {default: <base Subcommand>, versions: {"8.11.1": <diff>, …}}`, for example `SPECS/heroku/8.0.0.js`. Many version files have an empty `versions` (`{}`). A diff is a partial Subcommand where list items are matched **by name overlap**. `{name, remove:true}` deletes the matched item.

Merge semantics, decoded from the bundled helper (`lt.merge`, `at.merge`, `ct`, `Li` in `SPECS/heroku/index.js`):

```
mergeSubcommand(base, diff):  if diff == null -> base
   out = {...base, ...diff}                                  // shallow for plain keys
   if diff.subcommands: out.subcommands = mergeNamed(base.subcommands, diff.subcommands, mergeSubcommand)
   if diff.options:     out.options     = mergeNamed(base.options,     diff.options,     mergeOption)
   if diff.args:        out.args        = mergeArgs(base.args, diff.args)
mergeOption(base, diff): same, with only `args` special.
mergeNamed(baseList, diffList, merge):   // both makeArray'd
   used = {}; out = []
   for d in diffList:
     j = first index in baseList not in used whose names (makeArray) intersect d's names
     if j == -1: out.push(d)                                  // new item
     else: used.add(j); if !("remove" in d): out.push(merge(baseList[j], d))   // removed if remove key present
   for each base item not used: out.push(it)                  // untouched base items appended AFTER
   return out
mergeArgs(base, diff) = diff.filter(d => !d.remove).map((d,i) => ({...base[i], ...d}))   // positional
```

`getVersionFromVersionedSpec(base, versions, target)` lives in `@fig/autocomplete-helpers` and is **not on disk**. Its behavior (inferred):
1. Sort the keys of `versions` with `semver.compare`.
2. Apply `mergeSubcommand` cumulatively for every key `<= target` (all keys if `target` is undefined).
3. Return `{version: last applied key, spec}`.

(rec) Implement the version/diff logic in Rust after `default(version)` returns. Alternatively, run the bundled JS merge helper in the JS engine, since every diff-versioned `index.js` already contains it. Note, however, that the helper bundled in `index.js` is not exported.

### 2.6 What inshellisense does instead (`ISH/runtime.ts`)

- It loads the bundled npm specs only. The module-level `specSet` is a trie built from `index.js` paths (`aws/s3` becomes `specSet.aws.s3`). Diff-versioned names map to `<name>/index.js` (`ISH/runtime.ts:30-52`).
- `ignoredSpecs = ["gcloud","az","aws"]` (and their subpaths) are excluded (lines 26-28).
- Extra local spec roots come from the config `specs.path[]`; each root's `index.js` is merged in (lines 88-104).
- `loadSpec(tokens)` imports `specSet[root]` and caches by root name forever (lines 56-73).
- `getSubcommand(spec)`: if `spec` is a function, it calls `spec()` with **no version**. It returns the result only if that has `name`. **Diff-versioned specs therefore do not work**, because `default()` returns `{versionedSpecPath}` (lines 176-186).
- `lazyLoadSpec(key)` imports `<specsRoot>/<key>.js` for string `loadSpec` (lines 76-81). Function `loadSpec` results that are `SpecLocation`s are **TODO / unimplemented** (`lazyLoadSpecLocation` returns `undefined`, lines 84-86).

### 2.7 Spec normalisation: `convertSubcommand(spec, initializeDefault)` (inferred; `@fig/autocomplete-shared`)

Everything downstream in qac uses the normalised "Internal" shape (`SH/internal.ts:28-34`):

```ts
Internal.Subcommand = Omit<Fig.Subcommand,…> & {
  name: string[];                                   // makeArray(name)
  subcommands: Record<string, Internal.Subcommand>; // one key PER NAME ALIAS -> same object
  options: Record<string, Internal.Option>;         // keyed by every option name
  persistentOptions: Record<string, Internal.Option>;
  args: Internal.Arg[];                             // makeArray(args) or []
  loadSpec?: SpecLocation[] | Internal.Subcommand | ((token, exec) => Promise<SpecLocation[] | Internal.Subcommand>);
  parserDirectives?: {...}; generateSpec?; generateSpecCacheKey?; additionalSuggestions?; filterStrategy?; requiresSubcommand?; ...
}
Internal.Option = Fig.Option & { name: string[]; args: Internal.Arg[] }
Internal.Arg    = Fig.Arg & { generators: Fig.Generator[] }   // Metadata.ArgMeta
```

The evidence that this conversion exists:
- Subcommands are looked up as `spec.subcommands[token]` (`AP/parseArguments.ts:255`).
- Options are looked up as `spec.options[token] || spec.persistentOptions[token]` (`:244`).
- `Object.values(...)` is deduplicated with `new Set` because aliases map to the same object (`AC/suggestions/index.ts:105-107,137-140`).
- `arg.generators` is always iterated as an array (`AP/parseArguments.ts:107`).
- `loadSpec` is treated as either an array (locations) or a subcommand object (`AP/parseArguments.ts:863-898`).

Assumed `initializeDefault` details (inferred; consistent with the tests in `AP/../tests/parseArguments.test.ts:66-135`):
- `arg.generators = arg.template ? [{template: arg.template}, …makeArray(arg.generators ?? [])] : makeArray(arg.generators ?? [])`. For generators whose template includes `filepaths` or `folders`, it defaults `trigger:"/"` and `getQueryTerm:"/"`.
- `options` holds non-persistent options and `persistentOptions` holds options with `isPersistent:true`. (If both maps held persistent options, the `Set` dedup would fail because `convertOption` creates distinct objects.)
- `loadSpec` normalisation:
  - A string `s` becomes `[{name:s, type:"global"}]`.
  - A function `f` becomes `(t,exec)=>f(t,exec).then(r => Array.isArray(r) ? r : ("type" in r ? [r] : convertSubcommand(r)))`.
  - An object is passed through `convertSubcommand`.
  - The same applies to `arg.loadSpec`.
- Name map collisions: the later item wins.

How often each loadSpec shape appears in the 2.692.3 specs:
- `loadSpec:"<path>"`: 763 occurrences, e.g. `"docker-compose"`, `"aws/s3"`, `"az/2.53.0/account"`.
- `loadSpec:async e=>({name:"create-"+e,type:"global"})`: 5 (a single location object, which needs the normalisation above).
- `loadSpec:{…}`: 4.

### 2.8 `generateSpec` (`AP/parseArguments.ts:605-694`)

`generateSpec(tokens: string[], executeCommand) => Promise<Fig.Subcommand>` is used by 44 specs (e.g. `git`, `cargo`, `npm`-family, `dotnet`).

When it runs: in the token loop, **before** consuming token `i` (for i ≥ 1, including the iteration that hits the final token and breaks), if the current `completionObj.generateSpec` exists (`:907-916`). The tokens passed are **all** token texts, including the trailing partial token.

Cache key: if the subcommand defines `generateSpecCacheKey`, it is either a string or `({tokens}) => string`. The key becomes `${tokens[0]}:${cacheKey}`. There is no key in dev mode. The cache is `generateSpecCache` (global Map, no TTL).

The result is converted and **merged** into the current completionObj:
- `subcommands`, `options`, and `persistentOptions` are object-merged (generated entries win).
- `args`: the existing args are kept if `completionObj.args.length>0`; otherwise the generated args are used and the subcommand arg state is reset from them.

Errors are logged and the state is left unchanged. While parsing history, `exec` is a stub that throws `ParsingHistoryError` (`:596-603`).

A state that used generateSpec is stored in `parseArgumentsGenerateSpecCache`. For the `fig` spec only, the key is added to the set cleared by `clearFigCaches` (`:757-765,1016-1021`).

### 2.9 Mixins

Neither qac nor ish implements spec mixins (`grep mixin` only hits icon code). `"mixin"` is only a `SuggestionType` with its own icon (ish `🏝️`, `ISH/suggestion.ts:22`). (rec) skip.

---

## 3. Parsing argv against the spec (`AP/parseArguments.ts`)

### 3.1 Data structures (`:36-100`)

```ts
ArgArrayState = { args: Arg[] | null, index: number, variadicCount?: number }
TokenType = "none" | "subcommand" | "option" | "option_arg" | "subcommand_arg" | "composite"
Annotation = { text, type, tokenName? }                              // basic
           | { text, type:"subcommand", spec, specLocation }         // root of a (loaded) spec
           | { text, type:"composite", subtokens: BasicAnnotation[] } // -abc, --opt=val
ParserState = { completionObj, optionArgState, subcommandArgState, annotations,
                passedOptions: Option[], commandIndex, haveEnteredSubcommandArgs, isEndOfOptions }
ParserResult = { completionObj, currentArg: Arg|null, passedOptions, searchTerm, commandIndex,
                 suggestionFlags /* bitmask Subcommands=1, Options=2, Args=4, Any=7 (SH/utils.ts:12-18) */,
                 annotations }
```

`createArgState(args?)` (`:102-134`):
1. Start from `{args: args.length ? args : null, index:0}`.
2. Rewrite each arg's generators: any generator whose `template` includes `"filepaths"` is **replaced** by the bundled `filepaths` generator. Otherwise, if it includes `"folders"`, it is replaced by `folders` (§4.3).
   - "filepaths" wins over "folders".
   - Other template entries in the same generator (`history`, `help`) are **dropped**.
3. If the original had `filterTemplateSuggestions`, it is copied onto the shared generator object. (Quirk: this mutates a global.)
4. Generators are deduplicated with a Set.

Arg-state helpers:
- `updateArgState` (`:163-176`):
  - If the current arg is variadic, stay on it and increment `variadicCount`.
  - Else, if it is not the last arg, `index+1`.
  - Else, `{args:null,index:0}`.
- `getCurrentArg = args?.[index] ?? null`.
- `isMandatoryOrVariadic(a) = !!a && (a.isVariadic || !a.isOptional)`.
- `preferOptionArg(s) = isMandatoryOrVariadic(curOptArg) || !curSubcommandArg`. **An optional option-arg only takes a token when no positional slot is open.**
- `getArgState(s) = preferOptionArg(s) ? optionArgState : subcommandArgState`.

### 3.2 Entry point: `parseArguments(command, ctx)` (`:1088-1144`)

1. If there are no tokens, throw `ParseArgumentsError`.
2. **Single token** (typing the command name):
   - The spec is `firstTokenSpec` (`:1026-1086`). With setting `autocomplete.firstTokenCompletion` falsy, its `args` are `[]`.
   - If the token contains `/`: `bin/console` uses the global `php/bin-console`; anything else uses the global `dotslash` (`SPECS/dotslash.js` = `{name:"dotslash",args:{template:["filepaths","folders"]}}`).
   - Return `getResultFromState(initialState(spec, token, location))`.
3. Otherwise: `state = parseArgumentsCached(command, ctx)` (all tokens except the last).
4. Then `updateState(state, lastToken, isFinalToken=true)`. On exception, append `{type:"none", text:lastToken}`.
5. Return `getResultFromState(state)`.

`firstTokenSpec`'s single arg has one custom generator (cache `{strategy:"stale-while-revalidate", ttl:10000}`). It runs a login shell (`executeLoginShell`, §5.3) with `executable = ctx.currentProcess`:
- fish: `complete -C ""`, lines `name\tdescription`. Quirk: the name slice keeps the tab.
- bash: `compgen -c`.
- zsh: `for key in ${(k)commands}; do echo $key; done && alias +r`.
- Results are deduplicated by name, with type `"subcommand"`.

ish equivalent (`ISH/runtime.ts:437-465`) for an incomplete first token:
- Prefix-match the sorted spec names (binary search) and alias names.
- Aliases get `type:"shortcut"`, priority 100. Specs get `type:"subcommand"`, priority 40.

### 3.3 Token loop: `parseArgumentsCached(command, ctx, locations?, isParsingHistory, startIndex)` (`:780-1024`)

1. `tokens = command.tokens.slice(startIndex)`.
2. `locations = locations ?? [getSpecPath(tokens[0].text, cwd)]`.
3. **Cache:** `key = [tokens[0..-1].join(" "), serializeSpecLocation(loc), cwd, currentProcess].join(",")`. The last token is excluded, so typing within the last token is a cache hit.
   - Checked for each location, except in dev mode.
   - The states are stored in `parseArgumentsCache` / `parseArgumentsGenerateSpecCache` with no TTL, cleared by `resetCaches()`.
4. Load the first location whose spec loads (skip local ones while parsing history), using `withTimeout(5000, loadSubcommandCached(...))`. If none loads, throw.
5. `state = initialState(spec, tokens[0].text, specLocation)`, which pushes the root annotation `{text, type:subcommand, spec, specLocation}`.
6. `updateStateForLoadSpec(spec.loadSpec, 0)` handles a root that delegates (§3.8).
7. `for i in 1..tokens.length-1`:
   1. If `completionObj.generateSpec`, merge the generated spec (§2.8).
   2. If `i == tokens.length-1`, **break**. The final token is handled in `parseArguments` and is never used to load specs.
   3. Record `lastArgObject = currentArg(getArgState(state))`, `lastArgType = preferOptionArg ? option_arg : subcommand_arg`, and `lastState = state`.
   4. `state = updateState(state, tokens[i].text)` (§3.4). This throws if the token fits nowhere, which aborts the whole parse; the UI shows nothing.
   5. `lastType` is the type of the last annotation, or of the last subtoken if it is composite.
   6. **Arg alias directive** (§3.9).
   7. **Nested specs** (§3.8): `loadSpec = lastType==subcommand ? completionObj.loadSpec : undefined`. If the token was consumed as an arg (`lastType==lastArgType && lastArgObject`), it is overridden by:
      - `arg.loadSpec`, or
      - `isCommand`/`isScript`: `[getSpecPath(token, cwd, Boolean(isScript))]`, or
      - `isModule`: `[{global, name: isModule + token}]` (e.g. `python -m http.server` → `python/http.server`).

      If `updateStateForLoadSpec(loadSpec, i, token)` recursed, return its state.
8. Store the state in the cache and return it.

### 3.4 The state machine: `updateState(state, token, isFinal)` (`:543-573`)

Each candidate below is tried in order. A candidate that throws falls through to the next.

1. **Subcommand**, if `canConsumeSubcommands = !isMandatoryOrVariadic(curOptArg) && !haveEnteredSubcommandArgs` (`:538-540`).
2. **Option / option chain**, if `canConsumeOptions` (§3.5).
3. **Option arg**, if `preferOptionArg`.
4. **Subcommand arg**, otherwise. This throws `"Cannot consume subcommand arg."` if no positional slot is left.

`updateStateForSubcommand` (`:262-307`):
1. Throw if `haveEnteredSubcommandArgs` (after a positional, no more subcommands).
2. Look up `completionObj.subcommands[token]` (exact, case-sensitive, any alias).
3. Push the annotation `subcommand`. If final, stop there: the state does not descend.
4. Otherwise, inherit downwards:
   - The child gets the parent's `parserDirectives` if it has none.
   - `Object.assign(child.persistentOptions, parent.persistentOptions)`, so parent entries override same-named child ones. (qac mutates the cached spec object here; (rec) compute this without mutation.)
5. Reset the state: `completionObj=child`, `passedOptions=[]` (exclusivity and repeat counts restart per subcommand), empty option-arg state, `subcommandArgState = createArgState(child.args)`.

`updateStateForOption(state, flag)` (`:309-344`):
1. Look up `option = options[flag] || persistentOptions[flag]`, throwing if missing.
2. `isRepeatable`: `false` means 1. A number `n` throws once the count of passed options whose names overlap reaches `n`. `true` and `undefined` mean unlimited in the parser. (The suggestion list still hides already-used options by default, §4.7.)
3. Push the annotation `option`. If final, only annotate.
4. Else, `passedOptions += option` and `optionArgState = createArgState(option.args)`. This replaces any pending optional arg of a previous option.

`updateStateForOptionArg` / `updateStateForSubcommandArg` (`:346-396`):
- Both throw if there is no current arg of that kind, and both annotate.
- If not final, call `updateArgState(...)`. A subcommand arg also sets `haveEnteredSubcommandArgs=true`.

### 3.5 When options can be consumed: `canConsumeOptions` (`:191-238`)

Return false in each of these cases:
- `haveEnteredSubcommandArgs && completionObj.parserDirectives.optionsMustPrecedeArguments === true`.
- `isEndOfOptions` (after `--`).
- The current option arg is mandatory or variadic. **Exception:** if it is variadic, at least one value has been consumed (`variadicCount>0`), and `optionsCanBreakVariadicArg !== false`, return true.
- The current subcommand arg is variadic, has `variadicCount>0`, and has `optionsCanBreakVariadicArg === false`.

Otherwise return true.

### 3.6 Option tokens: `updateStateForChainedOptionToken` (`:398-536`)

1. If final and the token is `-` or `--`, throw. (A lone dash being typed is not an option; it falls through to the arg rules.)
2. If the token is `--` (not final), set `isEndOfOptions=true`, annotate `option`, and reset the option-arg state.
3. **Long-style:** `flagsArePosixNoncompliant || token.startsWith("--") || !token.startsWith("-")`. This also covers dashless "options".
   - `seps = new Set(parserDirectives.optionArgSeparators || "=")`. A string gives a set of its chars; an array gives a set of the strings.
   - Find the first char of the token that is in `seps`.
     - **With a separator:** split at it into `flag` and `arg` (the rest, rejoined).
       1. `updateStateForOption(flag)`, not final.
       2. Throw if the option has more than one arg.
       3. `updateStateForOptionArg(arg, isFinal)`. This throws for options without args, so the whole token falls back to being a positional.
       4. Annotate `composite[{option, text: flag+sep, tokenName: flag}, {option_arg, text: arg}]`.
       - This applies to **every** option, whether or not it has `requiresSeparator`.
     - **Without a separator:** `updateStateForOption(token, isFinal)`. If the option has `requiresEquals` or `requiresSeparator`, clear the option-arg state, so the next space-separated token is *not* its value.
4. **POSIX short chain** (`-abc`): `for i in 1..len-1`:
   1. Let `flag = "-"+token[i]`.
   2. Try `updateStateForOption(flag)`. On failure: if `i>1`, set `optionArg = token.slice(i)` and break; if `i==1`, rethrow.
   3. Push the subtoken `{option, text: i==1 ? "-a" : "b", tokenName: "-b"}`.
   4. If that option's current arg is mandatory or variadic, set `optionArg = token.slice(i+1)` and break.

   After the loop, if `optionArg` is non-empty, throw if the option has more than one arg, then `updateStateForOptionArg(optionArg, isFinal)` and push the subtoken `option_arg`.

   The result is a `composite` annotation. `passedOptions` excludes the last letter when final (`:534`), so it remains suggestable while being typed.

Expected behaviors, from the qac test table in `qac/packages/autocomplete-parser/tests/parseArguments.test.ts:291-530`. These tests are marked `describe.todo`, so they document intended behavior. The spec is `mocks/spec.ts`:
- `-m` has a mandatory arg, `-o` an optional arg, `-v` a variadic arg, and `-n` no args.
- `--req-eq` has `requiresEquals`.
- `--req-sep` has `requiresSeparator:":"`.
- `man` has two mandatory positionals.
- `oas` has `optionArgSeparators:["=",":"]`.

| buffer | flags | currentArg | searchTerm |
|---|---|---|---|
| `cmd man -oarg` | Args | `-o`.arg | `arg` |
| `cmd man -o=arg` | Args | `-o`.arg | `=arg` |
| `cmd man -omarg` | Args | `-m`.arg | `arg` |
| `cmd man -moarg` | Args | `-m`.arg | `oarg` |
| `cmd man -nb` (no `-b`) | Any | man.args[0] | `-nb` |
| `cmd man --opt=` | Args | `--opt`.arg | `` |
| `cmd man --none=arg ` | Any−Subcommands | man.args[1] | `` (consumed as positional) |
| `cmd man --req-eq ` | Any | man.args[0] | `` |
| `cmd oas --req-sep:arg ` | Any | oas.args[0] | `` |
| `cmd pnf -pnfo=arg` (posix-noncompliant) | Args | `-pnfo`.arg | `arg` |
| `cmd man -- -o ` | Args | man.args[1] | `` |
| `cmd -o arg ` (cmd has no positionals) | Any | null | `` (optional opt-arg consumed) |
| `cmd ompa argument ` (optionsMustPrecedeArguments) | Args | null | `` |

### 3.7 Result: `getResultFromState` (`:696-744`)

1. `searchTerm = lastAnnotation.text` (the unquoted final token, `""` after a trailing space).
2. `argState = getArgState(state)`.
3. If the last annotation is composite, use `optionArgState`. If its last subtoken is `option_arg`, set `searchTerm = subtoken.text` and `onlySuggestArgs=true`.
4. `currentArg = getCurrentArg(argState)`.
5. Compute the flags. Args are always on. Unless `onlySuggestArgs` or `isEndOfOptions`:
   - add Subcommands if `canConsumeSubcommands`, and
   - add Options if `canConsumeOptions`.

Consequences:
- Typing a partial subcommand still yields the parent's subcommand list filtered by `searchTerm`. The final token is not committed.
- An exactly-typed subcommand or option is annotated but not entered, so its siblings stay visible.

`initialParserState` is the result for an empty spec (`:746-755`).

### 3.8 Nested specs: `updateStateForLoadSpec(loadSpec, index, token?)` (`:858-901`)

1. `result = typeof loadSpec === "function" ? (token !== undefined ? await loadSpec(token, exec) : undefined) : loadSpec`. At index 0 there is no token, so function loadSpecs are skipped at the root.
2. **Array of SpecLocations:**
   - `state = parseArgumentsCached(currentCommand, ctx, result, isParsingHistory, startIndex + index)`. Parsing restarts with the token at `index` as the new root, e.g. `aws s3 ls` → spec `aws/s3` with tokens `s3 ls …`, and `sudo git …` → spec `git`.
   - Then `commandIndex += index` (cumulative across nesting).
   - Return the nested state.
3. **Subcommand object:** replace `completionObj` with `{…result, parserDirectives: {...parent.pd, ...result.pd}}`. Reset the option-arg state and passedOptions. `subcommandArgState = createArgState(result.args)` and `haveEnteredSubcommandArgs=false`.

`commandIndex` is what the generator context uses: `annotations.slice(commandIndex)` and `tokens.slice(commandIndex)` (`AC/state/generators.ts:26-37`). Generators of `git` under `sudo git` therefore see `["git", …]`.

Tests (`parseArguments.test.ts:620-675`): `cmd sudo sudo sudo sudo ` loads the spec 4 times and ends with 2 annotations, `[sudo-root, ""]`.

ish (`ISH/runtime.ts:188-250, 301-376`):
- `genSubcommand` resolves the loadSpec when entering a subcommand: a function result object is merged, a string triggers `lazyLoadSpec`, and an object is merged. It **overwrites** the parent's subcommand entry (memoization by mutation).
- `isCommand` args call `loadSpec(remainingTokens)` and recurse.
- `isScript` and `isModule` are not handled.

### 3.9 `parserDirectives.alias` on args (`:942-969`)

This is used by the git spec (`SPECS/git.js`):

```js
parserDirectives:{alias:async(token,exec)=>{const{stdout,status}=await exec({command:"git",args:["config","--get",`alias.${token}`]}); if(status!==0) throw …; return stdout}}
```

The procedure, after a token was consumed as the arg kind that owns the directive (`lastType==lastArgType`) and the token is not already in `substitutedAliases`:

1. `value = typeof alias==="string" ? alias : await alias(token, exec)`.
2. `currentCommand = substituteAlias(command /*original*/, tokens[i], value)` (§1.10).
3. Add the token to `substitutedAliases`, re-slice the tokens, set `state = lastState`, then `i -= 1; continue`. This re-parses the same position with the expanded words.
4. On failure:
   - If nothing has been substituted yet, the error propagates and the whole parse fails. For git, an unknown non-subcommand word gives no suggestions.
   - Otherwise it is remembered and thrown at the end of the iteration, unless a loadSpec consumed it.
5. `substitutedAliases` is cleared at the end of each normal iteration. The `continue` skips the clear, which prevents loops.

### 3.10 Fields that the parser ignores

- `isRequired` (17k uses in specs), `hidden`, `deprecated`, `default`, `isDangerous`, `displayName`, and `description` are not used by the parser.
- `exclusiveOn` and `dependsOn` are used only by suggestion building (§4.7).
- `isPersistent` takes effect through the `persistentOptions` maps.
- `requiresSubcommand` affects only space insertion (§4.7).

Field usage counts in the 2.692.3 specs:

| field | count |
|---|---|
| `isRequired` | 17402 |
| `icon` | 15638 |
| `isOptional` | 3720 |
| `default` | 3407 |
| `requiresSeparator` | 2793 |
| `hidden` | 2547 |
| `priority` | 2209 |
| `isVariadic` | 2115 |
| `insertValue` | 1628 |
| `displayName` | 1616 |
| `exclusiveOn` | 1328 |
| `isRepeatable` | 982 |
| `loadSpec` | 782 |
| `isPersistent` | 562 |
| `isDangerous` | 527 |
| `filterStrategy:"fuzzy"` | 409 |
| `dependsOn` | 257 |
| `debounce` | 102 |
| `isCommand` | 94 |
| `suggestCurrentToken` | 81 |
| `generateSpec` | 44 |
| `requiresEquals` | 27 |
| `isScript` | 18 |
| `requiresSubcommand` | 17 |
| `additionalSuggestions` | 16 |
| `optionsCanBreakVariadicArg` | 14 |
| `isModule` | 1 |
| `generateSpecCacheKey` | 0 |

`parserDirectives` combinations seen:

| combination | count |
|---|---|
| `{optionsMustPrecedeArguments:true}` | 124 |
| `{flagsArePosixNoncompliant:true}` | 49 |
| both | 5 |
| `{optionArgSeparators:["=",":"]}` | 1 |
| `{flagsArePosixNoncompliant:true, optionArgSeparators:["=","."]}` | 1 |
| arg-level `{alias}` | 4 |

**Name correction:** the directive is `flagsArePosixNoncompliant`, not "…Noncompound".

---

## 4. Suggestion generation

### 4.1 Generator dispatch (`AC/state/generators.ts:83-116`)

For each generator of `currentArg.generators`:

| condition | handler |
|---|---|
| `generator.template` | `getTemplateSuggestions` (§4.4) |
| else `generator.script` | `getScriptSuggestions` with default timeout = setting `autocomplete.scriptTimeout` (default **5000** ms) (§4.5) |
| else | `getCustomSuggestions` (§4.6) |

For the last case, if `filterTemplateSuggestions` is set and the first result has a `context`, `filterTemplateSuggestions(results)` is applied. This is the path for `filepaths`/`folders` with filters.

Results are tagged with `suggestion.generator = generator`, which `getQueryTerm` lookup uses later.

Context given to every generator (`AC/state/generators.ts:17-37`, `AC/generators/helpers.ts:10-15`):

```ts
{ currentWorkingDirectory: figState.cwd || "", currentProcess: processUserIsIn || "",
  environmentVariables: Record<string,string> /* from shell integration; pairs with empty key/value dropped (AC/state/index.ts:63-76) */,
  sshPrefix: "", annotations: annotations.slice(commandIndex),
  tokenArray: tokens.slice(commandIndex).map(t => t.text),   // includes the current partial token
  isDangerous: Boolean(currentArg?.isDangerous), searchTerm }
```

If `currentWorkingDirectory` is empty, script and custom generators return `[]` (`haveContextForGenerator`, `AC/generators/helpers.ts:25-26`).

### 4.2 When generators (re)run: triggers and debounce (`AC/state/generators.ts:118-198`)

This runs on every new parser result. For each generator index:

1. Trigger if there is no previous state at this index, **or** `currentArg !== previousArg` (object identity; the spec cache keeps identities stable while you type in the same slot).
2. Else, if `trigger === undefined`: do not re-run (results are filtered client-side), unless `currentArg.debounce`, in which case always re-run.
3. Else, compare the new search term `a` with the previous one `b`:

   | trigger | re-run when |
   |---|---|
   | string `t` | `a.lastIndexOf(t) !== b.lastIndexOf(t)`. The usual choice for `/` (path segments), `,` (lists), `:` and `@`. |
   | function | `trigger(a, b)` |
   | `{on:"change"}` | `a !== b` |
   | `{on:"threshold", length:n}` | `a.length > n && !(b.length > n)`. This fires **only when crossing upward**; ish fires both ways (`ISH/generatorCache.ts:84`). |
   | `{on:"match", string: s\|s[]}` | `strings.findIndex(x=>x===a) !== strings.findIndex(x=>x===b)` |

   An exception thrown in the trigger counts as a trigger.
4. When triggering, the new state is `{generator, context, result: previous.result || [], loading:true}`, so old results are kept while loading.
   - If `currentArg.debounce` is set: `sleep(typeof debounce==="number" && debounce>0 ? debounce : 200)`, then run, unless the state became stale (identity check in `updateGenerator`, `:43-68`).
   - Else run immediately.
5. When the request resolves, store `{loading:false, result}` if still current.

The trigger counts across specs are: `"@"`×7, `"/"`×6, `":"`×4, `","`×4, `"."`×2, `"$"`, `"+"`, `".."`, `" "`, `{on:"change"}`×1, plus about 300 function triggers (mostly the bundled `filepaths` trigger and `keyValue` triggers).

While a generator is loading, its stale results are shown **only if it has no `getQueryTerm`** (`AC/suggestions/index.ts:216-223`).

ish (`ISH/generatorCache.ts:44-192`) works differently:
- It is stateless per call and keyed by `JSON.stringify([cacheKey ?? generatorId, cacheKey ? [] : tokensExcludingActive, cacheByDirectory||!cache ? cwd : ""])`. The LRU holds 256 entries (`:9,53-68`).
- The trigger is evaluated against the last observed token for that key. String triggers accept an array too.
- `debounce:true` waits 100 ms (`:90-104`).
- An identical in-flight request is shared.
- Failed runs are not cached.
- `clearImplicitGeneratorState()` drops the entries without an explicit `cache` when the command ends (`:198-202`).

### 4.3 `filepaths` / `folders` templates

This is the exact algorithm from the bundled `@fig/autocomplete-generators`, decoded from `SPECS/apt.js`. qac swaps the template for this custom generator (§3.1), so it is the effective qac behavior.

```js
filepaths(options = {}) → Generator, where options = { extensions=[], equals=[], matches /*RegExp*/, filterFolders=false,
                                         editFileSuggestions, editFolderSuggestions, rootDirectory, showFolders="always"|"only"|"never" }
folders  = filepaths({showFolders:"only"})          // both are also callable with options
Generator = {
  trigger: (newT, oldT) => { const n = newT.lastIndexOf("/"), o = oldT.lastIndexOf("/");
                             if (n !== o) return true; if (n === -1 && o === -1) return false;
                             return newT.slice(0, n) !== oldT.slice(0, o); },
  getQueryTerm: t => t.slice(t.lastIndexOf("/") + 1),
  custom: async (tokens, exec, ctx) => {
    const dir = getCurrentInsertedDirectory(rootDirectory ?? ctx.currentWorkingDirectory, ctx.searchTerm, ctx) ?? "/";
    try {
      const out = await exec({ command: "ls", args: ["-1ApL"], cwd: dir });   // -1 one/line, -A no ./.., -p dirs get "/", -L follow links
      const names = sortFilesAlphabetically(out.stdout.split("\n"), [".DS_Store"]);
      const res = [];
      for (const name of names) if (name) {
        const tt = name.endsWith("/") ? "folders" : "filepaths";
        if ((tt === "filepaths" && showFolders !== "only") || (tt === "folders" && showFolders !== "never"))
          res.push({ type: tt === "filepaths" ? "file" : "folder", name, insertValue: name,
                     isDangerous: ctx.isDangerous, context: { templateType: tt } });
      }
      return applyEdits(applyFilter(res));
    } catch { return []; }
  }
}
sortFilesAlphabetically(list, skip) = [...nonDot.sort(localeCompare), ...dot.sort(localeCompare), "../"]   // skip is case-insensitive; "../" ALWAYS appended
getCurrentInsertedDirectory(cwd, searchTerm, ctx):
   if cwd === null -> "/"
   s = shellExpand(searchTerm, ctx); dir = s.slice(0, s.lastIndexOf("/") + 1)
   dir === "" ? ensureTrailingSlash(cwd) : dir.startsWith("/") ? dir : ensureTrailingSlash(cwd) + dir
shellExpand(s, ctx): 1) "~" or "~/..." prefix -> ctx.environmentVariables.HOME ?? "~"
                     2) $NAME -> env[NAME] ?? "$NAME"   3) ${NAME} / ${NAME:-default} -> env[NAME] ?? default ?? match
applyFilter (only when extensions/equals/matches given):
   keep if (type==="folder" && !filterFolders) || equals.has(name) || (matches && name.match(matches))
   else split name on "."; try each suffix "c", "b.c", "a.b.c"... against extensions set
applyEdits: merge editFileSuggestions into files / editFolderSuggestions into folders (object spread)
```

Spec usage: `template:"filepaths"`×2068, `"folders"`×974, `["filepaths"]`×217, `["folders"]`×111, `["filepaths","folders"]`×104, `"help"`×92, `["folders","filepaths"]`×26, plus about 25 template combinations that include history. Script args often use `generators: filepaths({extensions:["py"], editFileSuggestions:{priority:76}})`.

(rec) Implement this in Rust with `std::fs::read_dir`, not `ls`, so it works on Windows. Expose it to JS so the bundled `filepaths()` calls `exec({command:"ls",args:["-1ApL"],cwd})`. tabsmith's `executeCommand` should special-case exactly that argv by emulating it natively when `ls` is not on PATH (Windows). The output is one entry per line, `/` appended to dirs (symlinks followed), excluding `.` and `..`.

ish's own templates (`ISH/template.ts:7-59`, `ISH/templateCache.ts`) work as follows:
- `fs.readdir(cwd, {withFileTypes:true})` with a 100 ms TTL cache (16 entries).
- Entries: `{name, priority:55, context:{templateType}, type:"file"|"folder"}`.
- There is no `../`.
- The `history` and `help` templates return `[]` (TODO).
- The cwd is resolved from the partial token (`resolveCwd`, `ISH/utils.ts:119-152`):
  - `~` resolves to home.
  - A token containing the path separator uses its dirname, which must be readable, with a fallback to the parent.
  - The query becomes the basename (`ISH/suggestion.ts:375-377`).

### 4.4 Other templates (`AC/generators/templateSuggestionsGenerator.ts:10-87`)

- **`history`:** `getHistoryArgSuggestions(annotations, currentProcess)` (`AC/history/index.ts:319-401`).
  - Shell history (zsh `fc -R; fc -ln 1`, bash `fc -ln 1`, fish `history search`, run via login shells; or the custom command from setting `beta.history.customCommand`) is parsed with the same argv parser into a per-spec mirror tree of arg values (`:64-121, 252-305`).
  - The values recorded at the same position (subcommand-arg, then option-arg) are returned as `{name:value, type:"arg", context:{templateType:"history", …}}`.
  - Setting `beta.history.allShells` merges bash and zsh.
  - The fallback is Fig's own history.
  - (rec) This is optional for v1.
- **`help`:** this produces the subcommand names of the spec level *above* the subcommand that triggered the generator, excluding the triggering subcommand's own aliases. Typical spec: `help <command>`.
  - Algorithm: walk the annotations from the root spec, descending through each subcommand annotation except the last subcommand/option annotation.
  - Each entry is `{type:"special", name, insertValue:name, isDangerous:false, context:{templateType:"help"}}`.
- **`filterTemplateSuggestions(suggestions)`** is applied last if present.

### 4.5 Script generators (`AC/generators/scriptSuggestionsGenerator.ts:10-92`)

1. `cmd = typeof script === "function" ? script(tokenArray) : script`. Falsy means `[]`; this lets specs return `undefined` to skip.
2. `input = Array.isArray(cmd) ? {command:cmd[0], args:cmd.slice(1), cwd} : {cwd, ...cmd}`. An `ExecuteCommandInput` object may override `cwd`.
3. `timeout = max(defaultTimeout(5000 or setting), generator.scriptTimeout ?? 0, input.timeout ?? 0)`.
4. Run via the cache (§4.8) with `cacheKey = generator.cache?.cacheKey ?? JSON.stringify(input)`, calling `executeCommandTimeout(input, timeout)`.
5. Interpret `stdout`. (`stdout` is already cleaned: `\r\n`→`\n`, `ESC[?25h` removed, leading and trailing newlines stripped; §5.2.)
   - If `splitOn`: `stdout.trim()==="" ? [] : stdout.trim().split(splitOn)`.
   - Else, if `postProcess`: `postProcess(stdout, tokenArray)`, then filter out falsy items and objects without `name`.
   - **Neither:** `[]`. (ish: also `[]`.)
   - Note: `splitOn` takes precedence over `postProcess` in qac. ish does the opposite (`ISH/generator.ts:33-38`).
6. Map strings to `{type:"arg", name:s, insertValue:s, isDangerous}`. Objects get `type: item.type || "arg"`.
7. Any exception gives `[]`.

Script shapes in specs: arrays ×210, functions ×34 (returning an array or `undefined`), `{command,args,env?}` objects ×4. There are **no** string scripts, so the legacy string form is gone from 2.692.3.

### 4.6 Custom generators (`AC/generators/customSuggestionsGenerator.ts:11-57`)

```ts
custom(tokenArray: string[], executeCommand: ExecuteCommandFunction,
       generatorContext: { currentWorkingDirectory, currentProcess, sshPrefix: "", searchTerm,
                           environmentVariables, isDangerous }) => Promise<Fig.Suggestion[]>
```

The call goes through the cache with key `generator.cache?.cacheKey` (or the tokens, §4.8). Each result gets `type: s.type || "arg"`. An exception gives `[]`.

The `executeCommand` handed to `custom` uses the default timeout: **5000 ms**, or 20000 ms on Windows (`ABW/executeCommand.ts:19-22`, `ABW/executeCommandWrappers.ts:101-102`).

ish (`ISH/generator.ts:8-55`):
- The context is `{environmentVariables: process.env, currentWorkingDirectory: cwd, currentProcess:"", sshPrefix:"", isDangerous:false, searchTerm:""}`. **`searchTerm` is not filled in.** (rec: fill it in; the bundled `filepaths` relies on it.)
- `script`, `custom`, and `template` are all executed if present and concatenated.

### 4.7 Static suggestions and assembly (`AC/suggestions/index.ts`)

`getStaticSuggestions(completionObj, passedOptions)` (`:96-191`, memoized):

- **Subcommands:**
  - Built from the unique `Object.values(subcommands)`.
  - Each gets `shouldAddSpace` and `type:"subcommand"`.
  - Sorted with `localeCompare` on the first name.
- **additionalSuggestions:**
  - Strings become `{name, isDangerous: completionObj.isDangerous}`.
  - Entries without `type` or `icon` get the icon `fig://template?color=628dad&badge=➡️`.
  - Sorted alphabetically.
- **Options:**
  - Source: the unique values of `options` ∪ `persistentOptions`.
  - Drop an option if any of its names is in ∪ `passedOption.exclusiveOn`.
  - Drop it if `countEqualOptions(option, passed) >= (isRepeatable || 1)`, unless `isRepeatable === true`. **By default an option disappears once used.**
  - If any name is in ∪ `passedOption.dependsOn` and not yet passed, set `priority = 75`.
  - Add `shouldAddSpace`.
  - Compute `separatorToAdd`:
    - `undefined` if `args[0].isOptional`.
    - Else, if `requiresSeparator`, the string value itself, or `true` ⇒ the first of `parserDirectives.optionArgSeparators || "="`.
    - Else, if `requiresEquals`, `"="`.
  - Add `type:"option"`.
  - Sort alphabetically.

`shouldAddSpaceToItem` (`:45-68`):
- First arg exists and is not optional: true.
- Else, if `requiresSubcommand` is a boolean, use it.
- Else, true iff the item has a non-empty `subcommands` map.

Arg suggestions (`:87-93`): strings in `currentArg.suggestions` become `{type:"arg", name, isDangerous: currentArg.isDangerous}`. Objects keep their type, defaulting to `"arg"`.

`getAllSuggestions(currentArg, completionObj, passedOptions, flags, generatorStates, annotations)` (`:193-290`) builds the list in this order:
1. Subcommands, if the Subcommands flag is set.
2. Arg static suggestions, then each generator's results, if the Args flag is set. (Results are skipped while loading if the generator has `getQueryTerm`.)
3. additionalSuggestions, always.
4. Options, if the Options flag is set.

**Option chain** (last annotation is composite and its last subtoken is an option, e.g. typing `-la`):
1. Keep only `option` and `arg` suggestions.
2. If the Options flag is off (the option needs a mandatory arg), still append the option whose short name equals `-<last char>`.
3. `arg` suggestions get `getQueryTerm: () => ""`.
4. The option whose 2-char short name is `-<lastChar>` is renamed to the whole typed chain, so it exact-matches.
5. Every other option with a short name `-x` becomes `{name: chain+"x", insertValue: chain+"x"}`.

### 4.8 Generator result cache (`AC/generators/helpers.ts:30-58`, `AC/generators/cache.ts`)

1. `cache = generator.cache`. If there is none and setting `beta.autocomplete.auto-cache` is true, use `{strategy:"stale-while-revalidate", ttl:1000}`. If there is still no cache, just run.
2. `dir = generator.template ? getCWDForFilesAndFolders(cwd, searchTerm) : cwd`. Note that the swapped `filepaths` object has no `template`, so `dir` is the cwd.
3. `key = [cache.cacheByDirectory ? dir : undefined, cacheKey || tokenArray.join(" ")].toString()`, e.g. `",git checkout "`.

`CacheEntry.entry(run, cache)` (`AC/generators/cache.ts:45-56`):
- If not initialized, `fetchAsync`: await any in-flight promise, else run and store the value.
- `strategy` defaults to `"stale-while-revalidate"`:
  - **max-age:** `now > lastFetch + ttl` means a blocking refetch; otherwise return the cached value.
  - **stale-while-revalidate:** if a fetch is in flight, return the stale value; else, if `now > lastFetch + (ttl ?? 0)`, do a **blocking** refetch (`return this.fetchAsync(run)` is awaited by the caller); otherwise return the cached value. So qac's "SWR" only serves stale data while another request is in flight.
  - Any other strategy: always refetch.
- Quirk: if `run` rejects, `promise` is never cleared, so the entry is poisoned until `clear-cache`. (rec: do not replicate.)

`cache` usage in specs:

| cache value | count |
|---|---|
| `{ttl:30000}` | 40 |
| `{strategy:"stale-while-revalidate"}` | 30 |
| `{ttl:<var>}` | 34 |
| `{strategy:"stale-while-revalidate", cacheByDirectory:true}` | 15 |
| `{ttl: 2 days}` | 13 |
| `{swr, ttl:1h}` | 12 |
| others | rest |

ish semantics, which are the better model (rec):
- `age <= ttl` returns cached results.
- Past the TTL, max-age blocks and refetches; any other strategy returns stale data and refreshes in the background.
- A missing `cache` means results are memoized per key until trigger, cwd, or token-context change, and are cleared after each command.

### 4.9 Query term, filtering, ranking

`getQueryTermForSuggestion(s, searchTerm)` (`AC/suggestions/helpers.ts:102-128`):
1. Take `g = s.getQueryTerm || s.generator?.getQueryTerm`.
   - A string `g` gives `searchTerm.slice(searchTerm.lastIndexOf(g) + 1)`. This uses `+1`, not `+g.length` (qac quirk); ish uses `+length` and returns the whole token when `g` is not found (`ISH/generator.ts:57-62`).
   - A function `g` gives `g(searchTerm)`.
2. For `type:"shortcut"` with a leading `?`, strip the `?`.
3. Otherwise return `searchTerm`.

Common `getQueryTerm` values in specs: `t=>t.slice(t.lastIndexOf("/")+1)` (filepaths), `t=>t.slice(t.lastIndexOf(sep)+sep.length)` (keyValue), `"@"`×13.

`filterStrategy` selection (`AC/state/index.ts:363-382`):
- Read `currentArg ? currentArg.filterStrategy : completionObj.filterStrategy`.
- `"prefix"` means prefix matching and `"fuzzy"` means fuzzy matching.
- `"default"` or `undefined` means the user setting `autocomplete.fuzzySearch` (default false), re-read whenever the arg changes.
- In specs, `"fuzzy"` is used 409 times and `"prefix"` once.

`filterSuggestions(list, searchTerm, fuzzy, suggestCurrentToken, settings)` (`AC/suggestions/index.ts:393-519`):
- **Empty searchTerm:** return all suggestions with `!hidden`, in input (priority) order. If setting `autocomplete.immediatelyExecuteAfterSpace` is on, the list is non-empty, and auto-execute is not hidden, prepend `{name:"↪", type:"auto-execute", insertValue:"\n", description:"Immediately execute"}`.
- **Non-empty searchTerm:** for each suggestion:
  1. `q = queryTerm(s)`, and `qx = (type==="folder") ? q+"/" : q`.
  2. **Exact name match:** some name with `name.toLowerCase() === qx.toLowerCase()` (hidden suggestions are included). The suggestion goes into the exact group (non-folders first, then folders).
     - It is preceded once, by the first such match only, by an auto-execute clone `{...s, type:"auto-execute", icon: fig://icon?type=carrot, name: folder ? match minus "/" : name, description: folder ? "folder" : desc, insertValue:"\n"}`.
     - The clone is added only if all of these hold: (no name, no insertValue, or `names[0]===insertValue`); (not `isDangerous`, or setting `autocomplete.immediatelyRunDangerousCommands`); (no `args[0]`, or it is optional); and auto-execute is not hidden (settings `autocomplete.hideAutoExecuteSuggestion` or `autocomplete.onlyShowOnTab`).
  3. Else, if not hidden, try a partial match over its names.
  4. If it has a `displayName`, also try a partial match on `[displayName]`. This happens regardless of the above, so duplicates are possible.
- Partial match (`getPartialMatch`, `:357-391`):
  - Prefix scores come from `getPrefixNameMatches` (`helpers.ts:40-53`): exact = 0, case-insensitive exact = −1, case-sensitive prefix = −2, case-insensitive prefix = −3, else no match. The best score is kept.
  - With fuzzy matching enabled, the score is fuzzysort's (≤0, 0 = exact; `FZ` lines 119-199). If the suggestion also prefix-matches, the score becomes `prefixScore/10`, which keeps prefix matches on top.
  - With fuzzy matching disabled, the prefix score is used.
- Order the output as: exact non-folder matches, then exact folder matches, then partial matches sorted by score desc, then priority desc (stable).
- If the output is non-empty, no auto-execute was added, and auto-execute is not hidden:
  - If `searchTerm==="."`, or `searchTerm` ends in `/` and a suggestion has `context.templateType==="folders"`, prepend `{name: "." or "↪", type:"auto-execute", originalType:"folder", insertValue:"\n", description:"Enter the current directory"}`.
  - Else, if `suggestCurrentToken` (`currentArg.suggestCurrentToken ?? setting autocomplete.alwaysSuggestCurrentToken (false)`), prepend `{name: searchTerm, type:"auto-execute", insertValue:"\n", description:"Enter the current argument"}`.

fuzzysort algorithm (`FZ` lines 42-49, 119-252) for `single(query, target)`:
1. Compare lowercase char codes.
2. **Simple pass:** find the query chars in order. If any is missing, return null.
3. **Strict pass:** match each query char only at "beginning" positions (an uppercase after a non-uppercase, the first alphanumeric after a non-alphanumeric, or any non-alphanumeric char) or at the position right after the previous match. On failure, backtrack the previous char to its next beginning.
4. Score with `matches = strict ? strictMatches : simpleMatches`:
   - Start at 0. For each match index `t`, if it is not consecutive with the previous one, subtract `t`.
   - If the strict pass failed, multiply by 1000.
   - Finally subtract `targetLen − queryLen`.
   - An exact match scores 0.

Overall pipeline (`AC/state/index.ts:98-187`):
1. `getAllSuggestions`.
2. History handling:
   - If setting `beta.history.mode === "show"`, append the full-line history suggestions whose names are not already present, then boost by history priority.
   - If it is `"history_only"`, or the history mode toggle is on, use only history.
3. `updatePriorities` (recency boost; see the UI report §8) and a stable sort by priority desc.
4. `filterSuggestions`.
5. If 0 < n ≤ 50, deduplicate by fields `name, insertValue, displayName, args`.

ish pipeline (`ISH/suggestion.ts:358-457`):
1. Collect subcommands and options (unless already inside args), then generator results (default priority 60), static suggestions, and template results.
2. `filter`: prefix (`startsWith`, case-insensitive) or fuzzy, which in ish is **substring** `includes`. For array names, use the first matching name as the display name.
3. Priorities: options are always 45 (`getSuggestionPriority`); otherwise `priority ?? 50`.
4. Sort by priority. Within a priority band, file/folder entries are re-ranked by relevance (exact 3, prefix 2, substring 1), then shorter name, then `localeCompare`.
5. Adjust path insert values.
6. Remove:
   - suggestions whose names were already typed (`removeAcceptedSuggestions`),
   - `~` folders once the line contains `~`,
   - hidden entries, unless their name equals the typed token,
   - empty names,
   - duplicate names.
7. Options are additionally filtered by `exclusiveOn` against the typed option tokens.

### 4.10 Suggestion object fields (`Fig.Suggestion` plus qac internal additions, `SH/internal.ts:9-26`)

| field | qac behavior |
|---|---|
| `name: string \| string[]` | Used for matching (any element). The first element is used for sorting, priority/recency keys, and insertion (or the longest one with `autocomplete.preferVerboseSuggestions` for options and subcommands). Generator strings become `{name}`. |
| `displayName` | Shown in the UI. Also matched (partial only). |
| `insertValue` | Inserted verbatim for non-file/folder types (supports `{cursor}`; `\n` means execute). Files and folders are always built from the matched name, shell-escaped. Details: autocomplete-ui.md §5 / `AC/state/insertion.ts:151-224`. |
| `replaceValue` | Not used by qac. |
| `description` | UI only. |
| `icon` | UI only. `fig://icon?type=<name>`, `fig://template?...`, an emoji, or a URL (ish maps `fig://icon?type=x` to Nerd Font glyphs, `ISH/suggestion.ts:63-95`). |
| `priority` (0-100, default 50) | Clamped to [0,100] except for auto-execute. Recency boost applies. Ties and partial-match ordering use it. |
| `hidden` | Excluded unless exactly matched (qac) or typed exactly (ish). |
| `isDangerous` | Suppresses the auto-execute clone unless the setting allows it. |
| `type` | `"subcommand" \| "option" \| "arg" \| "file" \| "folder" \| "special" \| "mixin" \| "shortcut"`, plus internal `"history" \| "auto-execute"`. Affects matching (folders), auto-execute, chain logic, and insertion. |
| `previewComponent` | UI only (ignored). |
| `deprecated` | Ignored by the engine. |
| `context.templateType` | Marks template results (`filepaths`/`folders`/`history`/`help`). Used by `isTemplateSuggestion` and the folder auto-execute. |
| internal: `shouldAddSpace`, `separatorToAdd`, `args`, `generator`, `getQueryTerm`, `fuzzyMatchData`, `originalType` | Set by the engine. |

---

## 5. `executeCommand` / `executeShellCommand` API

### 5.1 Types (`@withfig/autocomplete-types` ≥1.31; not on disk, but every call site matches)

```ts
type ExecuteCommandInput  = { command: string; args: string[]; cwd?: string;
                              env?: Record<string, string | undefined>; timeout?: number /* ms */ };
type ExecuteCommandOutput = { stdout: string; stderr: string; status: number };
type ExecuteCommandFunction = (input: ExecuteCommandInput) => Promise<ExecuteCommandOutput>;
```

Where it is passed:
- As the 2nd parameter of `custom(tokens, exec, ctx)`, `generateSpec(tokens, exec)`, `loadSpec(token, exec)`, `parserDirectives.alias(token, exec)`, and `getVersionCommand(exec)`.
- Spec call sites always use object form: 268 `await x({command:…})` calls, 88 with `cwd:`, 1 with `env:` (`{command:"vr",args:[],env:{NO_COLOR:"1"}}`).
- A non-zero `status` does **not** reject. Callers check `status` themselves (e.g. the git alias directive).

### 5.2 qac implementation chain

`executeCommandTimeout(input, timeout = os==="windows" ? 20000 : 5000)` (`ABW/executeCommand.ts:19-61`):
1. Call `withTimeout(max(timeout, input.timeout ?? 0), Process.run({executable: input.command, args: input.args, environment: input.env, workingDirectory: input.cwd, terminalSessionId, timeout: input.timeout}))`. A timeout **rejects** with `TimeoutError`.
2. Clean `stdout` and `stderr` with `cleanOutput` (`:11-17`): `\r\n`→`\n`, remove `\x1b[?25h`, strip leading `\n+` and trailing `\n+`. Inner whitespace and trailing spaces are kept.
3. Return `{status: exitCode, stdout, stderr}`. A non-zero exit only logs a warning.

`Process.run` (`AB/process.ts:8-39`) sends a protobuf `RunProcessRequest {executable, arguments, env: [{key, value?}], workingDirectory, terminalSessionId, timeout: Duration}`. An env entry whose value is `undefined` means **unset that variable**.

Execution happens in the PTY wrapper (figterm), inside the user's shell session (`CR/figterm/src/message.rs:61-150, 459-515`):
- **Executable:** an absolute path is used as is. Otherwise it is resolved with `which_in(exe, PATH from the shell's env, cwd)`, falling back to the bare name. There is **no shell**: argv is executed directly.
- **cwd:**
  1. The request cwd, if it canonicalizes to an existing dir.
  2. Else the shell's current cwd (from the shell integration).
  3. Else the process cwd.
  4. Else `/` (or `C:\`).
- **Environment:** if the captured shell env (`SHELL_ENVIRONMENT_VARIABLES`, reported by the shell hooks) is non-empty, use exactly that env (`env_clear` + set). Then:
  - remove `LS_COLORS`, `CLICOLOR_FORCE`, `CLICOLOR`, `COLORTERM`;
  - set `PROCESS_LAUNCHED_BY_Q=1`, `HISTFILE=""`, `HISTCONTROL=ignoreboth`, `TERM=xterm-256color`, `NO_COLOR=1`;
  - then apply the request env (set, or remove on `None`).
- **Process setup:** on Unix, `setsid()` runs in `pre_exec` (no controlling TTY). On Windows the process is created with `DETACHED_PROCESS`. `kill_on_drop(true)`.
- **Timeout:** `request.timeout` or **60 s** by default (`:477`). On timeout or spawn error the response is an error, and the JS side rejects.
- **Output:** stdout and stderr are decoded as lossy UTF-8. `exit_code = status.code().unwrap_or(0)`, so a signal-killed process reports 0.
- The desktop-local fallback (no figterm session) is in `CR/fig_desktop/src/request/process.rs:83-121`. It differs as follows:
  - same idea, with the desktop's own env plus `Q_TERM=<version>`, `PROCESS_LAUNCHED_BY_Q=1`, `HISTFILE=""`, `HISTCONTROL=ignoreboth`, `TERM=xterm-256color`;
  - no `NO_COLOR`;
  - the outer wait is `max(request.timeout, 10s)` (`:65-69`).

`executeLoginShell({command, executable|shell, timeout})` (`ABW/executeCommandWrappers.ts:39-99`):
- Runs `[exe, "-lc"|"-lic" (linux: -lc), command]` with a timeout of `timeout ?? 5000` ms.
- The output is cleaned, then everything up to and including the **last** `ESC]697;DoneSourcing BEL` is dropped. That marker is printed by the shell integration after the rc files are sourced.
- If a shell is given by name, it is resolved through `/etc/shells` or `which`.

### 5.3 The legacy string script path

In the qac engine there is **no** working string path. `getScriptSuggestions` does `{cwd, ...commandToRun}`, and spreading a string yields `{0:"l",1:"s",…}` with no `command`. ish has the same bug (`ISH/generator.ts:29-31`). Fig's original engine ran a string `script` as `bash -c "<script>"` in the session cwd (inferred, historical); old specs relied on this. The 2.692.3 specs contain zero string scripts.

(rec) For tolerance, if `script` (or the function's return) is a string, run `["sh","-c",s]` on Unix or `["cmd","/C",s]` on Windows (or git-bash if found), in cwd. Likewise, if a `custom` generator calls `exec("some string")`, treat it the same way.

### 5.4 ish implementation (`ISH/utils.ts:90-115`)

`buildExecuteShellCommand(timeout, signal)` returns `({command,args,cwd,env}) =>` and does the following:
1. On Windows, find a git-bash and use it as the `spawn` shell (escaping args that contain `& | < >` or whitespace with `"…"`). Otherwise use no shell.
2. Spawn with env `{...process.env, ...env, ISTERM:"1"}`.
3. Collect stdout and stderr, then resolve `{status: code ?? 0, stdout, stderr}` on close.
4. At the timeout, send SIGKILL; the promise still resolves with partial output. There is **no output cleaning**.

The timeout is 5000 ms by default, or the generator's `scriptTimeout` for its script and custom calls (`ISH/generator.ts:24`).

### 5.5 Recommended Rust shape (rec)

```rust
struct ExecInput { command: String, args: Vec<String>, cwd: Option<PathBuf>, env: Vec<(String, Option<String>)>, timeout_ms: Option<u64> }
struct ExecOutput { stdout: String, stderr: String, status: i32 }
```

Semantics to implement:
- Spawn directly, without a shell. Resolve PATH using the user's shell env; on Windows also try `PATHEXT`.
- `cwd` defaults to the session cwd.
- Strip color variables and set `NO_COLOR=1` and `TERM=xterm-256color`.
- Detach from the TTY.
- Timeout = `max(5000 or setting, scriptTimeout, input.timeout)`. On timeout, kill the process and return an error to JS, so the generator yields `[]`.
- Apply qac's `cleanOutput` to stdout and stderr.
- Never reject on a non-zero status.

Expose this to rquickjs as an async host function returning a Promise. The spec bundles also use `console.*`, `setTimeout` (man.js), `URL` (git, hub, turbo, webpack, vsce, pre-commit, emr-serverless), `atob`/`btoa` (copilot, serverless/sls, pre-commit), `Intl.NumberFormat` (cargo), `String.prototype.localeCompare`, and `typeof process` guards. Polyfill whatever QuickJS lacks.

---

## 6. Divergences between qac and ish

| topic | qac (Fig/Q/Kiro) | ish |
|---|---|---|
| current command | Full AST: pipes, `&&`, `\|\|`, `;`, `&`, subshells, `$( )`, backticks, env prefixes | Regex split on `\|\| && ; \|`, not quote-aware |
| aliases | From the shell integration, recursive, first token, only after a space | Loaded once via `bash/zsh -i -c alias`, first token only |
| versioned specs | Simple and diff-versioned | Neither works |
| loadSpec function returning locations | Supported | TODO |
| short option chaining `-abc`, `-ovalue` | Yes | No (exact option names only) |
| `--opt=value` | Via `optionArgSeparators` | Lexer splits at `=` |
| `isRepeatable`, `exclusiveOn`, `dependsOn`, `optionsMustPrecedeArguments`, `optionsCanBreakVariadicArg`, `flagsArePosixNoncompliant`, `--` | Implemented | Only `exclusiveOn`, plus "remove already typed" |
| generator cache | Odd SWR (blocking), poisoned on error | Proper TTL/SWR with background refresh, LRU 256 |
| `filepaths` | Spec-bundled custom generator using `ls -1ApL`, with `../` | `readdir`, no `../`, path-aware cwd |
| fuzzy | fuzzysort + prefix bonus | Substring `includes` |

(rec) Follow qac semantics, with ish's cache and Windows ideas, and the noted fixes:
- treat an empty assignment value as complete;
- use `getQueryTerm` with `+len`;
- implement threshold triggers in both directions only if desired;
- do not poison the cache;
- run filepaths through native readdir.

---

## 7. Settings read by the engine (`ABW/settings.ts:1-51`)

| setting | default | used at |
|---|---|---|
| `autocomplete.developerMode`, `autocomplete.developerModeNPM` | false | Dev mode: no parse, generateSpec, or version cache; load dev specs |
| `autocomplete.developerModeNPMInvalidateCache` | — | Clears the spec cache once |
| `autocomplete.devCompletionsFolder` | — | §2.2 step 2 |
| `autocomplete.devCompletionsServerPort` | — | §2.2 step 1 |
| `autocomplete.disableForCommands` | [] | §2.4 |
| `autocomplete.personalShortcutsToken` | `"+"` | §2.1 |
| `autocomplete.firstTokenCompletion` | falsy | §3.2 |
| `autocomplete.scriptTimeout` | 5000 | §4.5 |
| `beta.autocomplete.auto-cache` | false | §4.8 |
| `autocomplete.fuzzySearch` | false | §4.9 |
| `autocomplete.alwaysSuggestCurrentToken` | false | §4.9 |
| `autocomplete.immediatelyExecuteAfterSpace` | false | §4.9 |
| `autocomplete.immediatelyRunDangerousCommands` | false | §4.9 |
| `autocomplete.hideAutoExecuteSuggestion`, `autocomplete.onlyShowOnTab` | false | §4.9 |
| `autocomplete.sortMethod` | `"recency"` (anything ≠ `"alphabetical"`) | Priorities |
| `autocomplete.preferVerboseSuggestions` | — | Insertion |
| `autocomplete.insertSpaceAutomatically` | true | Insertion |
| `beta.history.mode` (`off`/`history_only`/`show`), `beta.history.customCommand`, `beta.history.allShells` | — | History |
