# Fig completion-spec bundle (`@withfig/autocomplete@2.692.3`): what tabsmith's embedded JS engine must support

Scope: a static analysis of `ref/specs/package/build` (the compiled npm package). No code from the bundle was executed.
Path abbreviations used below:

- `BUILD/` = `scratchpad/ref/specs/package/build/`
- `PKG/` = `scratchpad/ref/specs/package/`
- `ISH/` = `scratchpad/ref/ish/` (microsoft/inshellisense)

Every bundle file is a single minified line, so citations into `BUILD/` give a **byte offset** (`@1234`) instead of a line number.
Counts come from a small lexer that blanks string, template and regex literal contents before matching, so text in
descriptions does not count. Section 16 describes the method and its limits. Anything marked **(inferred)** is a
conclusion or recommendation, not something read directly from code.

---

## 0. Summary

| Question | Answer |
|---|---|
| Module format | ES modules (`"type":"module"`). Every spec file ends with one `export{local as default[, x as y...]};` clause. |
| `import` between spec files | **None.** No static imports, no `import.meta`, no `require`. The only `import()` is in `dynamic/index.js`, a lazy-loader map that tabsmith does not need. |
| Shared chunks | **None.** Every file is self-contained. Helper libraries are inlined (duplicated) into 94 files as esbuild CommonJS-interop wrappers. |
| Exports other than `default` | `versions` (10 diff-versioned files), `getVersionCommand` (6 diff-versioned `index.js` files), and about 30 named helper exports nobody loads. |
| Loader name → file | `name` → `BUILD/<name>.js`. Fall back to `BUILD/<name>/index.js` for the 6 `diffVersionedCompletions` names. `loadSpec:"aws/s3"` → `BUILD/aws/s3.js`. |
| Host APIs the specs need | `executeCommand` (passed in as an argument, never a global), `console.{log,warn,error}`, `setTimeout` (1 spec), and optionally `Intl.NumberFormat` (1 spec). No `fetch`, `process`, `Buffer`, `window`, `URL`, `TextDecoder` or `crypto`. |
| `executeShellCommand` call form | Object `{command, args[, cwd, env]}` → `Promise<{stdout, stderr, status}>`. There are 297 direct object-form calls in spec code. The legacy string form occurs only 4 times, in 2 files (`aws/ecs.js`, `magento.js`). |
| `generatorContext` fields used | Spec code uses `environmentVariables` (17 files) and `currentWorkingDirectory` (2 files). The bundled `filepaths()` helper (76 files) also uses `searchTerm`, `isDangerous`, `currentWorkingDirectory` and `environmentVariables.HOME`. `currentProcess` and `sshPrefix` are never read. |
| Language features | ES2020 and later: async/await, generators, `?.`, `??`, `??=`, classes, optional catch binding, BigInt, Symbol, Map/Set, `matchAll`, `flatMap`, `.at()`, `\u{...}`, regex flags `gimsuy`, one named capture group, no lookbehind. QuickJS handles all of these. |
| Size | 76.7 MB raw. Solid `tar.zst -19` is 4.21 MB, and `-22 --long` is 3.91 MB. Per-file `zstd -19` totals 7.02 MB. aws, az and gcloud make up 82% of the raw bytes. |
| Recommendation | **Embed** in the binary, compressed with zstd. Allow an optional download/override directory for updates. See section 13. |
| License | `PKG/LICENSE` is **MIT**, "Copyright (c) 2021 Hercules Labs Inc. (Fig)". `PKG/package.json` says `"license": "ISC"`. Both are permissive: ship the MIT text plus notices for the inlined third-party code (section 1). |

---

## 1. Package facts

From `PKG/package.json`:

- `"name": "@withfig/autocomplete"`, `"version": "2.692.3"`, `"schemaVersion": "v7"`, `"type": "module"`.
- `"main": "./build/index"`, `"types": "./build/index.d.ts"`.
- `"exports": { ".": "./build/index.js", "./dynamic": "./build/dynamic/index.js" }`.
- `"files"`: `build/**/*.js|css|json|d.ts`, plus `icons/*.png|svg`.
- Runtime `dependencies`, which are inlined into the build: `@fig/autocomplete-generators ^2.4.0`, `@fig/autocomplete-helpers ^1.0.7`, `semver ^7.6.3`, `strip-json-comments ^5.0.1`, `yaml ^2.7.0`.
- `"repository": "https://github.com/withfig/autocomplete"`.

**License.** `PKG/LICENSE` is the MIT license text, "Copyright (c) 2021 Hercules Labs Inc. (Fig)". `package.json` declares `ISC`, which is inconsistent. The build contains **no** `/*!`/`@license` comments; only `wp.js` has the word "Copyright", and it is inside a description string. That means the inlined third-party code carries no license headers.

(inferred) Redistribute with:

- the MIT LICENSE text above, and
- a THIRD-PARTY notice for the inlined libraries:
  - semver (ISC), inlined in the 6 diff-versioned `index.js` files;
  - yaml (ISC), in `copilot.js`, `pre-commit.js`, `serverless.js`, `sls.js`;
  - strip-json-comments (MIT), in `deno.js`, `deno/generators.js`, `rush.js`;
  - `@fig/autocomplete-helpers` / `@fig/autocomplete-generators` (MIT, Fig), in 88 files.

`PKG/icons/` contains 49 PNG/SVG files: alert, android, apple, asterisk, aws, azure, box, carrot, characters, command, commandkey, commit, cpu, database, default-file-icon, default-folder-icon, discord, docker, file(.png/.svg), firebase, flag, folder(.png/.svg), gcloud, gear, git, github, gitlab, gradle, heroku, invite, kubernetes, netlify, node, npm, okteto, option, package, q, red-flag, slack, statusbar, string, symlink, template, twitter, vercel, yarn. These back `icon:"fig://icon?type=<name>"`.

Icon usage in specs:

- `fig://icon?type=` with these types: string 130, aws 47, asterisk 42, npm 32, git 24, option 21, yarn 20, docker 20, box 20, package 19, node 17, command 17, commandkey 14, kubernetes 12, alert 10, and others.
- `fig://template?color=..&badge=..`: 230.
- emoji literals such as `"\u{1F4E6}"`: about 12k.
- `http(s)` URLs: 1,388.

(inferred) A terminal overlay can map `type=` to a one-cell glyph and ignore URLs.

---

## 2. Directory layout and index files

`BUILD/` has 739 top-level entries: 716 `*.js`, 21 directories and 2 other files (`index.json`, `index.d.ts`). Recursively there are 1,489 files: 1,486 `.js`, `index.json`, `index.d.ts` and `dynamic/index.d.ts`.

| Subfolder | .js files | Top-level `<name>.js` exists? | Meaning |
|---|---|---|---|
| `aws/` | 418 | yes, `aws.js` | Subcommand specs loaded via `loadSpec:"aws/<svc>"`. `aws/regions.js` is a data module whose `default` is an **array**. |
| `az/` | 230 | **no** | Diff-versioned. `az/index.js` is the version resolver, `az/2.53.0.js` is the base spec with `versions`, and `az/2.53.0/*.js` holds 228 sub-specs (`loadSpec:"az/2.53.0/account"`). |
| `gcloud/` | 68 | yes | Sub-specs. `gcloud.js` references `gcloud/alpha` @523 and `gcloud/beta`, **which do not exist**. |
| `dotnet/` | 20 | yes | `dotnet/dotnet-add` and similar. |
| `deno/` | 3 | yes | `deno/generators.js` exports 8 named generators and **no default**. `deno/config_schema.d.js` and `deno/deno_doc.d.js` are **0-byte** files. |
| `php/` | 3 | yes | `artisan`, `bin-console`, `please`. |
| `python/` | 1 | yes | `http.server`. The `python` spec reaches it through `isModule:"python/"` (`python -m http.server`). |
| `task/` | 2 | yes | `go-task`, `taskwarrior`. |
| `fig/` | 4 | **no** | Diff-versioned: `index`, `1.0.0`, `2.0.0`, and `shared`, which is a helper module with `default = {}`. |
| `heroku/` | 4 | **no** | Diff-versioned: `index`, `8.0.0`, `8.6.0`, `shared` (`default = {}`). |
| `infracost/` | 3 | **no** | Diff-versioned: `index`, `0.9.0`, `0.10.0`. |
| `shopify/` | 3 | **no** | Diff-versioned: `index`, `2.0.0`, `3.0.0`. |
| `@usermn/sdc/` | 2 | n/a | Diff-versioned: `index`, `0.0.0`. |
| `@capgo/cli`, `@fig/publish-spec`, `@magnolia/cli`, `@preset/cli`, `@withfig/autocomplete-tools`, `@wordpress/create-block` | 1 each | n/a | npm-scoped CLIs, reached via `npx @scope/name` (an `isCommand` arg). |
| `example/` | 2 | n/a | `git_push`, `trigger`: documentation examples. |
| `dynamic/` | 1 | n/a | `dynamic/index.js` (69 KB): `export default {"<name>": () => import("../<name>.js"), ...}`. |

### `index.json`, `index.js`, `index.d.ts`

- `BUILD/index.json` (21 KB) is `{"completions": string[1484], "diffVersionedCompletions": string[6]}`.
- `BUILD/index.js` is the same data as ESM: `var e=[...];diffVersionedCompletions=[...];export{e as default,diffVersionedCompletions};`.
- `BUILD/index.d.ts`: `declare const completions: string[]; declare const diffVersionedCompletions: string[]; export { completions as default, diffVersionedCompletions }`.
- `diffVersionedCompletions = ["@usermn/sdc","az","fig","heroku","infracost","shopify"]`.
- `completions` holds **every** `.js` path without the extension (1,484 entries), with three adjustments:
  - the 6 diff-versioned names appear **without** `/index`, e.g. `"az"` rather than `"az/index"`;
  - `index` and `dynamic/index` are excluded;
  - everything else is included, even modules that are not specs (`fig/shared`, `heroku/shared`, `deno/generators`, the two empty `deno/*.d`, `aws/regions`, `example/*`).

  This was verified by comparing it with the file listing: `files − completions` = the 6 `*/index` + `index` + `dynamic/index`, and `completions − files` = the 6 diff-versioned names.
- Of the 1,484 entries, 727 are "root-loadable" (contain no `/`, or are `@scope/name`). The other 757 are sub-paths.
- `dynamic/index.d.ts`: `declare const completions: {[key: string]: () => Promise<{default: any}>}`. Its 1,484 keys equal `completions`, except that the 6 diff-versioned names appear as `"<name>/index"`. Every key `k` maps to `import("../k.js")`.

What a loader needs from the index:

- "Is there a public spec called `name`?" is `completions.has(name)`.
- "Is `name` diff-versioned?" is `diffVersionedCompletions.has(name)`.

---

## 3. Module format: exactly what the engine must parse

1. **Every non-empty file is an ES module whose only export statement is the final `export{...};`.** All 1,486 files were checked: 1,484 match `export\s*\{([^}]*)\}\s*;?\s*$`. The 2 exceptions are the 0-byte `deno/*.d.js`. Examples:
   - `var t=e;export{t as default};` (`-.js`)
   - `var n=e;export{n as default,a as versions};` (`az/2.53.0.js`)
   - `export{Lo as default,No as getVersionCommand};` (`az/index.js`, tail)
2. **No `import` statements, no `import.meta`, no `require(`, no `typeof require`** anywhere in code (strings excluded). The only `import(` is in `dynamic/index.js`. **A module resolver/loader is therefore unnecessary**: compile each file as a stand-alone module, or rewrite the export clause and run it as a script (see 3.6).
3. **No shared chunks.** Shared data is duplicated per file. For example, every `aws/*.js` starts with its own copy of the regions array (`aws/s3.js` begins `var x=["af-south-1",...`).
4. **esbuild CommonJS interop** appears in 94 files: the `__commonJS` helper `var R=(e,t)=>()=>(t||e((t={exports:{}}).exports,t),t.exports)` and `__toESM` built on `Object.create/defineProperty/getOwnPropertyDescriptor/getOwnPropertyNames/getPrototypeOf`. There are also TypeScript `__awaiter` helpers driven by `function*` generators (622 `function*` and 1,312 `yield` occurrences, almost all in this inlined library code). Example: `BUILD/git.js` @0..~6500.
5. **Top-level code has side effects but no top-level `await`.** None was found.
   - Factories run at load time: the JetBrains specs (`clion.js`, `goland.js`, `idea.js`, `phpstorm.js`, `pycharm.js`, `rubymine.js`, `rustrover.js`, `webstorm.js`) do `var e=(i,t)=>({...}); var n=e("clion","CLion"); export{n as default}`, and `cargo.js` exports `je()`.
   - Module-level mutable state exists: `man.js` keeps `A=new Map, Z=0, D=!1` with a 1-hour TTL via `Date.now()` (@~9300). The inlined keyValue helper keeps `var X=new Map` as a value cache.
   - **(inferred) Keep one long-lived JS context and cache module namespaces.** Re-evaluating per keystroke would lose these caches and cost a re-parse.
6. **Engine requirement (inferred).** Compile with `rquickjs::Module::declare(ctx, name, src)?.eval()?` and read `module.namespace()?.get("default")` (plus `versions` / `getVersionCommand`). This API is confirmed via the rquickjs docs (context7): `Module::declare`, `eval()` → `(Module, Promise)`, `namespace()`.

   Because every file has exactly one trailing export clause and no imports, an equally valid zero-loader option is a regex rewrite before evaluation:

   - `export{A as default,B as versions};` becomes `return {default:A, versions:B};`
   - the file body is wrapped in `(function(){ ... })()` and evaluated as a script.

---

## 4. What files export

| Export name | Files | Notes |
|---|---|---|
| `default` | 1,483 | Missing in `deno/generators.js` and the 2 empty files. |
| `versions` | 10 | `fig/1.0.0`, `fig/2.0.0`, `heroku/8.0.0`, `heroku/8.6.0`, `infracost/0.9.0`, `infracost/0.10.0`, `shopify/2.0.0`, `shopify/3.0.0`, `@usermn/sdc/0.0.0`, `az/2.53.0`. |
| `getVersionCommand` | 6 | The 6 diff-versioned `*/index.js`. |
| Other named exports (not loaded by any loader) | about 12 files | `themesGenerator` (×3), `stdOption` (×2), `dependenciesGenerator` (×2), `awsProfileGenerator`, `searchGenerator`, `existingUsersandGroups`, clang `std*Suggestions`/`clangBase`, deno `generate*` (8), fig/shared `*Generator(s)` (about 15), heroku/shared `getAppGenerator`. |

Export-set histogram: `{default}` 1,444; `{default, versions}` 10; `{default, getVersionCommand}` 6; the rest are small helper sets.

**What `default` evaluates to:**

- an object literal (`Fig.Subcommand`): 1,466 files;
- an object produced by a factory call: 9 files (8 JetBrains specs plus `cargo.js`);
- an **async function**: 6 files (the diff-versioned `*/index.js`);
- an **array**: 2 files (`index.js`, `aws/regions.js`);
- `{}`: 2 files (`fig/shared`, `heroku/shared`).

In TypeScript terms, a loaded module has one of two shapes:

```ts
type SpecModule =
  | { default: Fig.Spec; getVersionCommand?: Fig.GetVersionCommand }
  | { default: Fig.Subcommand; versions: Fig.VersionDiffMap };
```

Here `Fig.Spec` is either a `Subcommand` or `(version?: string) => Promise<Subcommand | {versionedSpecPath, version}>`.

---

## 5. Resolving names and `loadSpec` strings to files

### 5.1 Resolution algorithm (Fig conventions)

- A spec file is looked up as `${path}${name}.js` and, **if reading fails**, as `${path}${name}/index.js`.
- If `name` is diff-versioned, load `${name}/index` and resolve its result to a `Subcommand` (section 6).
- If the name is not in `index.json`, fall back to a local `~/.fig/autocomplete/build/<name>.js`.
- The first token, or an `isCommand`/`isScript` arg, maps to a spec location as follows:
  - `"?"` → local `_shortcuts` found by walking up to `.fig/autocomplete/build/_shortcuts.js`.
  - The personal-shortcuts token (default `"+"`) → `~/` local `+`.
  - First token (`isScript === undefined`):
    - `bin/console` or `*/bin/console` → global `php/bin-console`;
    - a name with no `/` → **global** spec `name`;
    - anything containing `/` → a **local** script spec `<dir>/.fig/autocomplete/build/<basename>.js`.
  - `isCommand` arg (`isScript === false`): a path not starting with `/`, `./` or `~/` → global, so `npx @capgo/cli` → `@capgo/cli`.
  - `isScript` arg → local.
- `isModule:"python/"` with token `http.server` → global `python/http.server`.
- Cache loaded specs by `source,path,name` and wrap loading in a **5000 ms** timeout.

### 5.2 inshellisense's algorithm (simpler)

- It builds a nested name tree from `index.js`. A versioned name maps to `<path>/<name>/index.js`, everything else to `<path>/<name>.js` (`ISH/src/runtime/runtime.ts:30-50`).
- It **drops `gcloud`, `az`, `aws`** and all of their sub-specs (`runtime.ts:26-28`).
- String `loadSpec` → `path.join(specResourcesPath, key + ".js")` (`runtime.ts:76-81`).
- ISH does **not** really support diff-versioned specs. `getSubcommand` calls `spec()` with no version and checks for a `name` property on the returned Promise, so it returns `undefined` (`runtime.ts:176-186`).

### 5.3 What the bundle contains

- `loadSpec` occurs 782 times in spec code across 37 files.
  - **763 are string literals.** By prefix: `aws/…` 409, `az/…` 228, `gcloud/…` 70, `dotnet/…` 19, `php/…` 3. The rest are top-level names: `git-flow` 5, `docker-compose` 3, `django-admin`, `serverless`, `git`, `simctl`, `php` (2 each), and `grep`, `flutter`, `rg`, `ssh`, `npm`, `docker`, `xcodebuild`, `composer`, `speedtest-cli`, `bunx`, `create-react-native-app`, `react-native`, `tailwindcss`, `next`, `gltfjsx`, `prisma` (1 each).
  - Every string target resolves to `BUILD/<value>.js` **except** `gcloud/alpha` and `gcloud/beta` (`gcloud.js` @523), which are missing upstream. tabsmith must treat a missing `loadSpec` target as "no extra spec", not as an error.
  - 4 are inline object literals (a `Subcommand`, e.g. `tmuxinator.js`, `fig/*`).
  - 5 are functions plus 1 via a reference (`bun.js`, `pnpm.js`, `yarn.js`, `rushx.js`). Example: `loadSpec:async e=>({name:"create-"+e,type:"global"})` returns a **SpecLocation** `{name, type:"global"|"local", path?}`.
  - The rest are references or calls returning strings.
  - Generators can also put `loadSpec` on *suggestion objects*, e.g. `{name, loadSpec: i==="rw"?"redwood":i}` in `bun.js`/`pnpm.js`.
- A function `loadSpec` is called as `await loadSpec(token, exec)`. The result can be a `SpecLocation[]`, which triggers a recursive parse, or a `Subcommand`, which is merged with the parent `parserDirectives`.

**Rust resolution rule for tabsmith (inferred from the above):**

```text
resolve(name):
  if name in diffVersioned           -> load "<name>/index.js"  (default is async fn; see section 6)
  elif exists "<name>.js"            -> load it
  elif exists "<name>/index.js"      -> load it                 (fallback)
  else                               -> None (silently)
first token -> name only if it contains no '/', except the "bin/console" special case
```

---

## 6. Diff-versioned specs (6 names)

### 6.1 Resolver files (`<name>/index.js`)

All six share the same inlined code: `semver` plus the Fig spec-diff library. `default = pt(name, versionsList)`, which is `(cliVersion?) => Promise<{versionedSpecPath: "<name>/<chosen>", version: cliVersion}>` (`BUILD/az/index.js` @~27400-27700).

| name | versions list | `getVersionCommand(exec)` (exact) |
|---|---|---|
| `az` | `["2.53.0"]` (@27689) | `JSON.parse((await exec({command:"az",args:["version","-o","json"]})).stdout)["azure-cli"]` |
| `fig` | `["1.0.0","2.0.0"]` | `{stdout} = await exec({command:"fig",args:["--version"]})`; return `stdout.slice(stdout.indexOf(" ")+1)` |
| `heroku` | `["8.0.0","8.6.0"]` | `exec({command:"heroku",args:["--version"]})`, then regex `/heroku\/([0-9]+\.[0-9]+\.[0.9]+)/` (sic: `[0.9]`); return the match or `"8.0.0"` |
| `infracost` | `["0.9.0","0.10.0"]` | `exec({command:"infracost",args:["--version"]})` → `semver.clean(stdout.slice(stdout.indexOf(" ")+1))` |
| `shopify` | `["2.0.0","3.0.0"]` | `exec({command:"shopify",args:["version"]})` → `stdout.match(/\d+\.\d+\.\d+/)?.[0] ?? ""` |
| `@usermn/sdc` | `["0.0.0"]` | `(await exec({command:"npx",args:["@usermn/sdc","--version"]})).stdout`. (inferred) This may make `npx` download a package; consider skipping it or putting it on a denylist. |

**Version choice, `Si(sorted, v)`** (`az/index.js` @27400):

```js
versions.sort(semver.compare);          // sorts in place, ascending
if (!v) idx = last;
else idx = (scan from last to 0, first i with semver.compare(versions[i], v) <= 0), else last;
return { versionedSpecPath: `${name}/${versions[idx]}`, version: v };
```

**Version detection (inferred):**

- It only runs when `typeof default === "function"`.
- The result is cached per name.
- With `getVersionCommand`, it uses `await getVersionCommand(executeCommand)`.
- Otherwise it uses `semver.clean((await executeCommand({command:name,args:["--version"]})).stdout)`.
- Errors are swallowed and the version becomes `undefined`.

### 6.2 Versioned files (`<name>/<x.y.z>.js`)

These export `default` (a full base `Subcommand`) and `versions`, a map of version string → **diff** object. Examples:

- `fig/2.0.0.js`: `h["2.15.0"]={subcommands:[{name:"restart",...}]}; h["2.16.0"]={...}`.
- `heroku/8.0.0.js`: diffs with `{name:"run:logs",remove:!0}`.
- `az/2.53.0.js`: `{"2.53.0":{}}`.

Applying the diffs needs a helper such as `getVersionFromVersionedSpec(default, versions, version).spec` from `@fig/autocomplete-helpers`, which is **not on disk**. (inferred, from the merge code inlined in every `*/index.js` @26692-27400 and from the shape of `versions`) It sorts the `versions` keys by semver and applies `subcommandMerge(spec, versions[k])` for each key `<= version`, in ascending order.

**Merge semantics** (read from the inlined code, `az/index.js` @~26700):

- `S(x)` = `x ? (Array.isArray(x) ? x : [x]) : []`.
- `namesOverlap(a, b)` = some name in `S(b.name)` is in the set `S(a.name)`.
- `subcommandMerge(base, diff)`:
  - if `diff === null`, return `base`;
  - otherwise `out = {...base, ...diff}`;
  - for each of the keys `subcommands`, `options`, `args`: if `diff[key]` is truthy, set `out[key] = merger[key](base[key], diff[key])`.
- `listMerge(baseList, diffList, itemMerge)` (used for `subcommands` with `subcommandMerge`, and for `options` with `optionMerge`):
  1. For each `d` in `diffList`, find the first **unused** base index `o` whose names overlap.
     - If none: push `d` (a new item).
     - Otherwise mark `o` used. If `"remove" in d`, drop it. Else push `itemMerge(base[o], d)`.
  2. Append all unused base items **after** the diff items. **The resulting order is: diff items first, then the remaining base items.**
- `optionMerge` is the same object merge as `subcommandMerge`, but only `args` is merged recursively.
- `argsMerge(base, diff)` = `S(diff).filter(d => !d.remove).map((d, i) => ({...S(base)[i], ...d}))`. Note the quirk: the index `i` is taken *after* filtering.

---

## 7. Runtime APIs and language features used by the bundle

Each row gives "all code" (including inlined libraries) and "spec code" (inlined CJS libraries blanked; yaml is ESM-inlined, so it stays counted in its 4 files). The format is files / occurrences.

| API / global | All code | Spec code | Notes for tabsmith |
|---|---|---|---|
| `fetch(` | 0 | 0 | Not needed. Network access goes through `executeCommand` with `curl`, which appears 27 times. |
| `require(` / `import` | 0 | 0 | — |
| `process.*` | 6 / 18 | 0 | Only `typeof process=="object"&&process.env&&process.env.NODE_DEBUG` inside semver (`fig/index.js` @911). It is guarded, so **do not define `process`**. |
| `Buffer` | 0 | 0 | yaml checks `typeof Buffer`/`typeof atob` (`copilot.js` @32844). |
| `atob` / `btoa` | 4 / 16 | 4 | Guarded (yaml binary tags only). |
| `setTimeout` | 1 / 1 | 1 | `man.js` @9609: `await new Promise(i=>setTimeout(()=>i(),4e3))` while another call is filling its cache. **Must be provided**, because QuickJS core has none (rquickjs ships no timers; inferred, verify for your version). |
| `setInterval` / `clearTimeout` / `queueMicrotask` | 0 | 0 | Provide `clearTimeout` cheaply for safety. |
| `TextDecoder` / `TextEncoder` / `URL` / `URLSearchParams` / `crypto` / `structuredClone` / `performance` / `navigator` / `window` / `self` / `globalThis` / `fig.*` / `Fig.*` | 0 | 0 | Not needed. QuickJS has `globalThis` anyway. |
| `console.*` | 132 / 226 | 47 / 132 | Methods: `log` 66, `error` 62, `warn` 4 (spec code). **Must exist.** Route the output to a log file, never to the TTY (section 15). |
| `JSON.parse` | 170 / 485 | 96 / 309 | Heavily used on `stdout`. |
| `JSON.stringify` | 94 / 179 | 7 / 91 | — |
| `Date` | 10 / 18 | 10 | `new Date` 9, `Date.now` 1 (`man.js`), `Date.UTC` 4 (yaml). |
| `Intl.*` | 1 / 1 | 1 | `cargo.js` @10811: `new Intl.NumberFormat(void 0,{notation:"compact",compactDisplay:"short",maximumSignificantDigits:3})`. **QuickJS has no Intl**, so this custom generator would throw and fall back to `[]`. Provide a 10-line polyfill or accept the loss. |
| `.toLocale*()` | 5 / 5 | 5 | `toLocaleUpperCase`/`toLocaleString`. QuickJS implements these (non-localized). |
| `.localeCompare(` | 103 / 193 | 16 / 17 | Mostly the filepaths sorter. QuickJS gives a code-point compare, so sort order can differ slightly from V8 ICU. |
| `encodeURIComponent` / `decodeURIComponent` | 2 / 3 and 4 / 4 | ✓ | Built in. |
| `Map` / `Set` / `Promise` / `Symbol` / `BigInt` / `WeakMap` / `Proxy` / `Reflect` | 105, 115, 108, 13, 4, 0, 0, 0 files | ✓ | Built in. |
| `Object.entries` / `Object.assign` / `Object.freeze` / `Object.defineProperty` / `Array.from` / `.flatMap` / `.flat` / `.at(` / `.matchAll(` | ✓ | ✓ | `.at(`: 8 files; `.matchAll`: 10 files. Not used: `Object.fromEntries`, `.replaceAll`, `.findLast`, `.toSorted`, `structuredClone`. |
| `new RegExp(` | 14 / 28 | 8 / 16 | Some patterns are built from strings. |
| `eval` / `Function(` / `with` | 0 | 0 | — |

**Language features:**

| Feature | Count |
|---|---|
| `async` | 112 files, 349 uses |
| `await` | 101 files, 362 uses |
| `function*` / `yield` | 94 files (inlined `__awaiter`) |
| `?.` | 114 files, 730 uses |
| `??` | 111 files, 314 uses |
| `??=` | 1 file (`twilio.js` @544) |
| `class` | 10 files (semver, yaml) |
| optional catch binding `catch{` | 121 files |
| spread / rest | used |
| template literals | used everywhere; esbuild turns `"\n"` into a backtick literal containing a real newline |
| `\u{1F4E6}` code-point escapes | in strings |
| getters, `#private`, `static{}`, `**`, numeric separators, BigInt literals, `async function*`, `for await`, top-level await | 0 |

**Regex literals (792 total):**

- flags: `g` 445, `m` 28, `i` 22, `s` 21, `y` 12, `u` 5;
- 1 named group (`fnm.js` @190 `(?<...>)`);
- **0 lookbehind**, 0 `\p{}`, 0 `d`/`v` flags.

**Verdict.** Everything is within QuickJS's ES2020+ support. The host must add only:

1. `console`
2. `setTimeout` / `clearTimeout`
3. optionally `Intl.NumberFormat`
4. the `executeCommand` function passed as an argument (never a global)
5. a Promise job loop

For item 5, rquickjs `AsyncRuntime::execute_pending_job()` / `idle()` (confirmed in rquickjs docs).

---

## 8. Callback signatures the host must call (counts in spec code; inlined libraries in parentheses)

"Spec code" means the `@fig/*` CJS wrappers are blanked. Counts are object-literal keys plus method shorthand.

| Key | Spec-code occurrences: value kinds | Function arity seen | Host call |
|---|---|---|---|
| `generators` | 4,677 in 312 files: object, array, or result of `filepaths()`/`keyValue*()`/`valueList()`/`ai()` | — | Accepts one generator or an array. |
| `script` | 695 in 219 files: **array 615**, function 56 + 8 method + 3 ref, object `{command,args[,env]}` 4, call 6+1 | 0–1 `(tokens)` | Array → `{command:a[0], args:a.slice(1), cwd}`. Object → `{cwd, ...obj}`. Function → called with `tokens` and may return an array, an object, or **`undefined`, meaning skip**. **No legacy string `script` exists** (0 occurrences). |
| `postProcess` | 668 in 213 files: function 568 + 15 method + 70 ref, call 14 | 1: 591, 2: 60 | Sync `(stdout, tokens) → (string \| Suggestion)[]`. Drop falsy items and items without a name. |
| `splitOn` | 30 in 21 files, all string (`"\n"` as a template literal) | — | `stdout.trim()===""?[]:stdout.trim().split(splitOn)`. ISH checks `postProcess` first. |
| `custom` | 213 in 82 files: function 198, factory call 14, object 1 (bundle-wide 653 including libraries) | 0: 6, 1: 24, 2: 146, 3: 36 | `await custom(tokens, executeCommand, generatorContext)` (`ISH/src/runtime/generator.ts:41-43`). May be sync or async, and **may return `undefined`** (e.g. `aws/ecs.js` `if(s<0)return;`), so treat that as `[]`. Items get `type:"arg"` if `type` is missing. |
| `trigger` | 89 in 63 files: function 60, string 27, object 1 (`z.js`: `{on:"change"}`) | 2: 39, 1: 12, 0: 9 | Called as `trigger(searchTerm, previousSearchTerm)` → bool. A string `s` means `a.lastIndexOf(s)!==b.lastIndexOf(s)`. Objects: `on:"change"` (`a!==b`), `"threshold"` (`length`), `"match"` (`string`). |
| `getQueryTerm` | 57 in 46 files: string 33, function 23 | 1 | Function → `getQueryTerm(token)`. String `s` → `token.slice(token.lastIndexOf(s)+s.length)`, or the whole token if `s` is not found (`ISH/src/runtime/generator.ts:57-62`). |
| `template` | 3,621 in 515 files: string 3,143, array 476 | — | Values: `"filepaths"` 2,068, `"folders"` 974, `"help"` 92, `"history"` 7, plus arrays of those. |
| `filterTemplateSuggestions` | 7 in 6 files | 1 | `(TemplateSuggestion[]) → TemplateSuggestion[]`. |
| `cache` | 213 in 69 files: object 192, ref 10, boolean 11 (`valueList({cache:true})`) | — | Keys: `ttl` 139 (ms; literal values seen: `1e3`×40, `3e4`×40, `100`×7, `6e4`×4, `60`×4, `1e4`×3, `36e5`×2, `18e5`, `86400`, `72e5`, `1728e5`, `6048e5`; the rest are variables), `strategy` 81 (`"stale-while-revalidate"` 79, `"max-age"` 3), `cacheByDirectory` 24, `cacheKey` 5. |
| `scriptTimeout` | 13 in 11 files (all values `15e3`, `1e4`, `12e3`, `2e4`; 94 more inside the `ai()` library) | — | ISH uses `scriptTimeout ?? 5000` and SIGKILL on timeout (`ISH/src/runtime/utils.ts:96-97`). |
| `generateSpec` | 44 in 36 files: function 38, ref 6 (all **async**) | **2** | `await generateSpec(tokens, executeCommand)` → `Subcommand`, merged into the current node. Optional cache key `generateSpecCacheKey` (string or `({tokens})=>string`). |
| `loadSpec` | see 5.3 | 1 | `await loadSpec(token, exec)`. |
| `parserDirectives.alias` | 10 functions (git, hub, gh, bun, pnpm, yarn, rushx, …) | 2 | `await alias(token, exec)` → string, which replaces the token. Example (`git.js`): `{stdout,status} = await exec({command:"git",args:["config","--get",`alias.${e}`]}); if(status!==0) throw …; return stdout`. |
| other `parserDirectives` keys | `optionsMustPrecedeArguments` 129, `flagsArePosixNoncompliant` 55, `optionArgSeparators` 2 | — | Parser-only. |
| `debounce` | 102 in 29 files (boolean) | — | A debounced arg delays generator runs until typing pauses. |
| `isDangerous` | 351 booleans in 87 files | — | Not a function. |

Version-related signatures:

- `getVersionCommand(executeCommand) → Promise<string>`: 6 files.
- Diff-versioned `default(version?: string) → Promise<{versionedSpecPath, version}>`: 6 files.

---

## 9. The `executeCommand` contract

### 9.1 Input

The input is **an object**: `Fig.ExecuteCommandInput = {command: string, args: string[], cwd?: string, env?: Record<string,string>, timeout?: number}` (ISH `utils.ts:90-96`).

- Direct object-literal calls in spec code: `custom` 242 + `generateSpec` 55 = **297**. A further 83 helper functions receive the executor and were resolved to object-form calls as well (88 pass-throughs, 79 + 4 resolved).
- Keys seen inside `fn({command:…})` calls in spec code: `command` 294, `args` 291. Spec code never passes `cwd`, `env` or `timeout` directly.
  - `cwd` is passed by the inlined `filepaths()` helper (`{command:"ls",args:["-1ApL"],cwd:dir}`, `git.js` @3122).
  - `env` appears only in `vr.js`'s `script:{command:"vr",args:[],env:{NO_COLOR:"1"}}`.
  - The host must fill in `cwd = currentWorkingDirectory` when it is absent (ISH `generator.ts:30-31`).
  - **Merge `env` over the inherited environment.** ISH does `{...process.env, ...env, ISTERM:"1"}` (`utils.ts:96`). (inferred) tabsmith could likewise set a marker env var, e.g. `TABSMITH=1`.

### 9.2 Output

The output is `Promise<{stdout: string, stderr: string, status: number}>`.

- Spec code reads `{stdout:x}=…` 209 times, `.stdout` 73, `.status` 19, `{status:x}` 7, and `.stderr` **0**. **`status` must be the numeric exit code**: `git.js`/`hub.js` aliases do `if(o!==0)throw…`, and `bun.js`/`pnpm.js` do `if(t.status!==0)…`.
- (inferred) Normalize **both** `stdout` and `stderr` with a `cleanOutput` function:

  ```js
  out.replace(/\r\n/g, "\n").replace(/\x1b\[\?25h/g, "").replace(/^\n+/, "").replace(/\n+$/, "")
  ```

  ISH returns raw stdout; its script path then `.trim()`s before `postProcess` (`generator.ts:33`). CRLF normalization matters on Windows.
- On failure (spawn error or timeout), the executor can **throw**, and the generator's try/catch returns `[]`. ISH resolves `status:code??0`. Since specs wrap their own calls in try/catch, either works; tabsmith rejects on spawn failure (section 14).
- Timeouts (inferred): default 5000 ms and **20000 ms on Windows**, taking `max(timeout, input.timeout)` when the input carries one. Specs never pass `timeout`, so the default applies to them.

### 9.3 Legacy string form (old Fig API)

There are exactly **4 call sites in 2 files**:

- `aws/ecs.js` @3385, @3653 and @4175: `await a("aws ecs list-tasks …")` and `` await a(`aws deploy list-deployment-groups --application-name ${i}`) ``, followed by **`JSON.parse(r)`**. The result is used as the stdout *string*.
- `magento.js` @22: helper `var c=async t=>{let i=await t(`php -r 'print(json_encode(require "app/etc/env.php"));'`);return JSON.parse(i)}`, called from `generateSpec` next to object-form calls.

An executor that accepts only the object form makes these throw (`input.args` is undefined, so a `[command, ...args]` spread fails), and the generator yields nothing.

(inferred) Cheap compatibility in the JS shim: `typeof input === "string"` → run `sh -c input`, or the configured POSIX shell on Windows, and resolve to **`stdout` as a string**. That was legacy Fig's contract.

### 9.4 Which programs specs run

There are 156 distinct executables (`script:[…]` heads plus `command:"…"`). Top ones:

- `fig` 215, mostly the inlined `ai()` library: `fig settings --format json autocomplete.ai.enabled` and `fig _ request --route /ai/chat`.
- `aws` 146, `bash` 128, `ls` 106, `git` 98, `cat` 74, `npm` 28, `curl` 27, `yarn` 24, `docker` 20, `podman` 11, `task` 11, `ffmpeg` 9, `networksetup` 8, `cargo` 7, `gh` 6, `brew` 6, `xcrun` 5, `pbpaste` 4, `sysctl` 4, ….

Portability:

- **`bash -c …` occurs 138 times in 72 files**, plus `sh -c` 2, `zsh -c` 1 and `fish -c` 1.
- **`ls -1ApL` occurs 100 times**: the inlined `filepaths()` helper in 88 files, plus about 12 direct uses such as `aws/s3.js` script functions (one has a typo, `"-1ApL "`).
- macOS-only tools appear: `defaults`, `mdfind`, `launchctl`, `system_profiler`, `networksetup`, `dscl`, `pbpaste`, `xcrun`, `shortcuts`.

(inferred) For Windows parity:

1. **Intercept** `{command:"ls", args:["-1ApL"], cwd}` and answer natively from Rust. Output is one entry per line, directories suffixed `/`, symlinks followed (`-L`), dotfiles included except `.`/`..` (`-A`).
2. Run `bash` through Git Bash when it is present, as ISH does: `getExecutionShell` → `gitBashPath()` on win32, with special-character escaping (`ISH/src/runtime/utils.ts:17-39`).
3. Intercept `command:"fig"`, returning `{status:1, stdout:"", stderr:""}`, so the 5 specs using `ai()` (`git.js`, `hub.js`, `pre-commit.js`, `turbo.js`, `vsce.js`) degrade quietly instead of running a foreign binary.

---

## 10. `generatorContext` (third argument of `custom`)

The `generatorContext` has this shape:

```ts
{ currentWorkingDirectory: cwd || "",
  currentProcess: foregroundProcess || "",
  sshPrefix: "",
  searchTerm,                       // parser's current search term (raw text of the token being completed)
  environmentVariables,             // shell's env (Record<string,string>)
  isDangerous: Boolean(currentArg?.isDangerous) }
```

ISH passes the same keys, but `currentProcess:""`, `searchTerm:""` (TODO), `isDangerous:false`, and `environmentVariables` is the host's `process.env` (`ISH/src/runtime/generator.ts:8-17`).

Usage in spec code:

- `environmentVariables`: 26 occurrences in **17 files** (asdf, bun, echo, env, goto, j, …).
- `currentWorkingDirectory`: 3 occurrences in 2 files (`cargo.js`, `z.js`).
- Nothing else. **`currentProcess` and `sshPrefix` are never read anywhere.**

Usage in the inlined `filepaths()`/`folders()` helper (88 files contain it; 76 files call it 159 times):

- It destructures `{isDangerous, currentWorkingDirectory, searchTerm}` and uses `environmentVariables.HOME` and `$VAR` expansion (`git.js` @1142-3542).
- **So `searchTerm` must be the real current token.** Otherwise `filepaths()` always lists the cwd: this is exactly ISH's bug with `searchTerm:""`.

The helper's algorithm, for a native re-implementation or verification:

- `shellExpand(s, ctx)`:
  1. If `s` starts with `~` and (`len==1` or `s[1]=='/'`), replace the first `~` with `ctx.environmentVariables?.HOME ?? "~"`.
  2. Then replace `/\$([A-Za-z0-9_]+)/g` with the env value if defined, else keep it as is.
  3. Then replace `/\$\{([A-Za-z0-9_]+)(?::-([^}]+))?\}/g` with the env value, else the default, else as is.
- `getCurrentInsertedDirectory(cwd, searchTerm, ctx)`:
  1. If `cwd===null`, return `"/"`.
  2. `o = shellExpand(searchTerm)`, then `a = o.slice(0, o.lastIndexOf("/")+1)`.
  3. Return `a===""` ? `cwd` with a trailing `/` : `a.startsWith("/")` ? `a` : `cwd + "/" + a`.
- `filepaths({extensions=[], equals=[], matches, filterFolders=false, editFileSuggestions, editFolderSuggestions, rootDirectory, showFolders="always"})` returns:
  - `trigger(new, old)`: true if the `lastIndexOf("/")` values differ; false if both are -1; otherwise compare the prefixes before the last `/`.
  - `getQueryTerm: t => t.slice(t.lastIndexOf("/")+1)`.
  - `custom(tokens, exec, ctx)`:
    1. Run `exec({command:"ls", args:["-1ApL"], cwd: getCurrentInsertedDirectory(rootDirectory ?? cwd, searchTerm, ctx)})`.
    2. Split on `\n`, drop `.DS_Store`, then sort: non-dot names ascending by `localeCompare`, then dot names, then append `"../"`.
    3. A line ending in `/` → `{type:"folder"}`, otherwise `{type:"file"}`, each `{name, insertValue:name, isDangerous, context:{templateType:"filepaths"|"folders"}}`.
    4. Respect `showFolders` (`"only"` / `"never"` / `"always"`), apply the `extensions` / `equals` / `matches` filters, then merge the `edit*Suggestions`.
    5. Return `[]` on any error.
  - `folders = filepaths({showFolders:"only"})`. Both are exported as `Object.assign(fn, Object.freeze(fn()))`, so they work as a generator object **or** a factory.
- `ai()` prompt functions are called with `{tokens, executeCommand, generatorContext}`, an object argument (`apt.js` @6889-7902).

---

## 11. Inlined helper libraries (per-file copies, not shared)

| Library (marker) | Files | Host impact |
|---|---|---|
| `@fig/autocomplete-helpers`: `filepaths`, `folders`, `shellExpand`, `getCurrentInsertedDirectory`, `sortFilesAlphabetically` | 88 | Needs `exec` with `cwd`, the context fields above, and `localeCompare`. |
| `@fig/autocomplete-generators`: `valueList`, `keyValue`, `keyValueList`, `ai` | 88 | Pure JS. `ai()` needs `fig` (section 9.4). Calls in specs: `filepaths()` 159 in 76 files, `valueList` 10 in 9, `keyValueList` 13 in 4, `keyValue` 8 in 4, `ai` 5 in 5. |
| `semver` + spec diff/merge | 6 | Classes, regexes, the guarded `process`. |
| `yaml` | 4 | Classes, Symbol, BigInt, guarded `atob`/`btoa`, `Uint8Array`. |
| `strip-json-comments` | 3 | Pure. |

---

## 12. Lifecycle requirements (inferred from the code above)

- **Long-lived context.** Evaluate each module once and cache its namespace by path. Module-level caches (`man.js`, `keyValue` `Map`) depend on this.
- **Keep JS functions alive.** Specs mix data with functions, and functions cannot be serialized to JSON. Either:
  - keep `Persistent<Function>` handles, or
  - walk the spec once in JS, replacing each function with `{"$fn": id}` and storing it in a JS-side table, so Rust gets a serde-friendly tree plus callable ids.
- **Async.** 546 of 638 `custom` functions (bundle-wide) are async, and all `generateSpec`/`getVersionCommand`/`alias`/`loadSpec` functions are async. `postProcess`, `trigger` and `getQueryTerm` are always sync. Always `await` whatever `custom` returns, since some are sync. Pump jobs until the promise settles. Enforce per-call timeouts on the Rust side (loading 5000 ms; scripts per section 8).
- **Safety.** Specs are third-party code run on every keystroke. Set a QuickJS memory limit and an interrupt handler (rquickjs `Runtime::set_memory_limit` / `set_interrupt_handler`; inferred, check the API for your version).
- **Parse cost.** The largest files are:

  | File | Size |
  |---|---|
  | `gcloud/compute.js` | 4.07 MB |
  | `aws/ec2.js` | 1.49 MB |
  | `az/2.53.0/network.js` | 1.47 MB |
  | `aws/sagemaker.js` | 0.80 MB |
  | `aws/rds.js` | 0.80 MB |

  The median `.js` is 13.5 KB and p90 is 136 KB. 1,133 of 1,486 files (64.2 MB) contain **no function at all**: they are pure data, including 405 of the 418 aws sub-specs, 229 of the 230 az files and all 68 gcloud files. (inferred) An optional build-time optimization is to pre-evaluate pure-data specs to JSON/bincode, or to precompile to QuickJS bytecode. Only the 353 function-bearing files (12.5 MB) then need the engine at runtime.

---

## 13. Size, and embed vs download

| Measure | Bytes |
|---|---|
| Raw (all 1,489 files) | **76.67 MB** (tar 77.8 MB) |
| aws (419 files incl. `aws.js`) | 36.99 MB |
| az (230) | 12.55 MB |
| gcloud (69) | 14.10 MB |
| everything else (771) | 13.03 MB |
| npm tarball `withfig-autocomplete-2.692.3.tgz` | 7.8 MB |
| solid tar.gz -9 / -6 | 7.88 MB / 7.95 MB |
| solid tar.zst -3 / **-19** / -22 --long=27 | 6.26 MB / **4.21 MB** / 3.91 MB |
| solid tar.xz -9e / brotli -q 11 | 3.86 MB / 4.01 MB |
| per-file gzip -9 (sum) | 8.58 MB |
| per-file **zstd -19** (sum, random access) | **7.02 MB** (aws 3.03, az 0.89, gcloud 0.44, other 2.66) |
| per-file zstd -19 + trained 110 KB dictionary | 5.77 MB |
| "other" only, solid zstd -19 | 1.22 MB |
| aws + az + gcloud only, solid zstd -19 | 2.96 MB |
| decompression of the whole 76.6 MB zstd stream (Python `compression.zstd`) | 61 ms |

**Recommendation (inferred): embed.**

- Embedding adds 4–7 MB to the binary. You get offline operation, a pinned spec version that matches the tested JS shim, and no network dependency for specs.
- The simplest layout with random access and no disk writes is a `build.rs` that zstd-19 compresses each file and emits a sorted `&[(&str /*path*/, &[u8] /*zstd*/)]` table (~7.0 MB), decompressed on first use and cached in memory.
- If binary size matters more, use one solid `specs.tar.zst` (4.2 MB) extracted once to `<cache_dir>/tabsmith/specs/<version>/` (77 MB on disk), or use the trained dictionary (5.8 MB).
- **Optional updates.** Add a command that downloads the npm tarball (`https://registry.npmjs.org/@withfig/autocomplete/-/autocomplete-<ver>.tgz`; standard npm URL pattern, inferred) into a user specs dir that takes precedence over the embedded set. Resolution order: user dir → downloaded → embedded.
- **Optional build flag** to leave out aws, az and gcloud, saving about 63 MB raw and about 4.4 MB of per-file zstd. ISH excludes them unconditionally (`runtime.ts:26-28`).

---

## 14. Minimal host shim (inferred; derived from sections 7–10)

This JS prelude is evaluated once per context. `__exec`, `__log` and `__sleep` are Rust functions; the async ones return Promises via rquickjs futures.

```js
globalThis.console = { log: (...a) => __log("log", a), info: (...a) => __log("info", a), debug: (...a) => __log("debug", a),
                       warn: (...a) => __log("warn", a), error: (...a) => __log("error", a) };
let __tid = 0; const __live = new Set();
globalThis.setTimeout = (fn, ms = 0, ...args) => { const id = ++__tid; __live.add(id);
  __sleep(ms).then(() => { if (__live.delete(id)) fn(...args); }); return id; };
globalThis.clearTimeout = (id) => { __live.delete(id); };
globalThis.Intl ??= { NumberFormat: class { constructor(_l, o = {}) { this.o = o; }
  format(n) { if (this.o.notation !== "compact") return String(n);
    const u = [[1e12,"T"],[1e9,"B"],[1e6,"M"],[1e3,"K"]].find(([v]) => Math.abs(n) >= v);
    return u ? +(n / u[0]).toPrecision(this.o.maximumSignificantDigits ?? 3) + u[1] : String(n); } } };
// executeCommand passed to custom/generateSpec/loadSpec/alias/getVersionCommand:
globalThis.__makeExec = (cwd) => (input) => typeof input === "string"
  ? __exec("sh", ["-c", input], cwd, null, 0).then((r) => r.stdout)        // legacy Fig (4 call sites)
  : __exec(input.command, input.args ?? [], input.cwd ?? cwd, input.env ?? null, input.timeout ?? 0);
```

The Rust `__exec` must:

- spawn `command` with `args` **without** a shell, in `cwd`, with env = inherited + `env`;
- enforce a timeout (default 5 s, 20 s on Windows; kill on expiry);
- normalize output with `cleanOutput`;
- resolve `{stdout, stderr, status}`;
- reject on spawn failure;
- short-circuit `ls -1ApL` (native) and `fig` (status 1).

---

## 15. Edge-case checklist

- **0-byte modules.** `deno/config_schema.d.js` and `deno/deno_doc.d.js` are empty but listed in `index.json`. Loading them must give "no default", not crash.
- **No `default` export.** `deno/generators.js`.
- **Non-object `default`.** Arrays in `aws/regions.js` and `index.js`; `{}` in `fig/shared` and `heroku/shared`. Do not treat these as root CLIs. Only names reachable as a first token or via `loadSpec`/`isCommand` should matter.
- **Missing `loadSpec` targets.** `gcloud/alpha` and `gcloud/beta` must be ignored silently.
- **Diff-versioned names.** These have no `<name>.js`; they must go through `<name>/index.js`, then `getVersionCommand`, `versionedSpecPath`, the versioned file and the diff merge. If version detection fails, the version is `undefined` and the newest file is chosen.
- **`script` function may return `undefined`.** Skip the run in that case.
- **`custom` may return `undefined`.** Treat it as `[]`.
- **`postProcess` items may be strings, `null`, or objects without a name.** Filter them.
- **`splitOn` + empty stdout.** The result must be `[]`, not `[""]`.
- **Generators fail silently.** Every generator error, including a JS exception, a timeout, `JSON.parse` of empty stdout, or the `Intl` ReferenceError, must yield `[]` and log, never abort the menu.
- **`console` capture.** Calls should go to a log file, never to the user's TTY: they would corrupt the overlay.
- **`trigger` may throw.** Treat that as `shouldTrigger = true`.
- **Name collisions.** Spec names like `-` (zsh pre-command modifier, `hidden:!0`, `args:{isCommand:!0}`), `aliases`, `figterm` exist as real top-level specs.

---

## 16. Method (for reproducibility)

- Analysis scripts (Python, run with `-I` from the scratchpad, reading the bundle as data only):
  - `scratchpad/scripts/jsstrip.py`: a tolerant JS lexer that blanks string, template and regex literal contents and keeps `${}` expressions. It was validated by checking bracket balance after stripping: all 1,486 files balanced.
  - `scratchpad/scripts/analyze.py`, `query.py`, `query2.py`, `query3.py`.
- "Spec code" = full code with every esbuild `__commonJS` wrapper body (`var X=R(…)`, about 798 KB in total) blanked. The yaml library is inlined as ESM, not CJS, so its features stay counted for `copilot.js`, `pre-commit.js`, `serverless.js` and `sls.js`.
- Limits:
  - Key counts use `(?<=[{,])key:`, so they include destructuring patterns.
  - Value kinds resolve identifiers by their first definition in the same file. `ref?` means unresolved, e.g. member access like `o.listCertificates`.
  - Executor-call tracing follows the second parameter up to 3 helper hops within a file. 9 + 5 pass-throughs were unresolved; manual sampling found only object-form calls among them.
- Sizes were measured with `tar --format=ustar`, `gzip`, the `zstd` CLI, `xz`, `brotli`, and Python 3.14 `zlib`/`compression.zstd`.
