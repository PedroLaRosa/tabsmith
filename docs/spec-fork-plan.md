# Keeping the completion specs current: our own fork of withfig/autocomplete

Status: decided, nothing implemented yet. Research and measurements are from 2026-10-24. Written so a fresh session can
continue without the conversation that produced it.

## Context

tabsmith embeds the compiled `build/` directory of the npm package `@withfig/autocomplete` as `assets/specs.tar.zst`
(SPEC D11, 4.2 MB). The version is in `assets/SPECS_VERSION` (`2.692.3`). The archive is extracted once to
`~/.tabsmith/specs/<ver>/` and the specs run in QuickJS (`src/specs.rs`, `src/js.rs`). `scripts/update-specs.sh [version]`
re-bundles from the npm tarball. `~/.tabsmith/specs/user/` already overrides bundled specs (`specs::user_dir`,
`resolve_in`), so local patches need no engine change. `docs/research/spec-format.md` describes the bundle format.

Problem: upstream is frozen. Its last commit and release are 2025-05-05, and `2.692.3` is still npm `latest`. The repo is
not archived, but CLIs released since then are missing flags and subcommands. Goal: keep the argument data updated,
produced by a static pipeline we control and that tabsmith consumes unchanged.

## Decision

Fork `withfig/autocomplete` as `tabsmith-specs`, build it ourselves and publish the bundle as a release asset. The output
stays in the Fig spec format, so the engine and `src/specs.rs` do not change; only `scripts/update-specs.sh` points
somewhere else. Generated data from other sources may be layered on later as an additive overlay (see "Later"), but is
not part of v1. The licensing checklist below lists what the fork must keep and what new specs may come from.

## Findings

### Why not another source

| Source | State | Verdict |
|---|---|---|
| withfig/autocomplete | MIT, not archived, frozen since 2025-05-05. 716 top-level spec files, 1,484 `.ts` files in `src/`. | Base. |
| microsoft/inshellisense | Active. Depends on `@withfig/autocomplete` pinned at `2.675.0`; no spec fork. | No successor here. |
| carapace-bin | Active, MIT. About 550 completers in `completers/common`, written as Go code. I found no export of a whole command tree. | Skip. |
| jdx/usage | Active, KDL spec format. Tool authors ship their own spec; I found no registry of third-party CLIs. | Skip (both points unverified). |
| sigoden/argc-completions | MIT, 844 scripts. `completions/` last changed **2024-10-29**; the 2026 commits are tooling. 612 of about 616 commits are by one person. | Gap filler only. |
| fish completions | GPL (GitHub reports `NOASSERTION`; GPL-2.0 from memory). | Avoid: clashes with MIT OR Apache-2.0. |
| context7 | Docs for LLMs, not a flag database. | Not a data source. |

Name overlap with Fig's 716 specs: carapace 274, fish 240, argc 366. Union of the three: 463. 253 Fig names are in none.

### argc vs Fig, 11 tools (unique flag names, inherited flags excluded)

| tool | Fig | argc | only in argc | only in Fig |
|---|---|---|---|---|
| brew | 108 | 350 | 249 | 7 |
| cargo | 127 | 127 | 9 | 9 |
| docker | 339 | 407 | 88 | 20 |
| gh | 189 | 328 | 142 | 3 |
| git | 405 | 695 | 316 | 26 |
| kubectl | 221 | 263 | 53 | 11 |
| npm | 79 | 84 | 36 | 31 |
| pnpm | 76 | 120 | 54 | 10 |
| rustup | 35 | 38 | 3 | 0 |
| terraform | 22 | 64 | 42 | 0 |
| uv | 150 | 143 | 39 | 46 |

Fig has real gaps (`gh issue pin`, `docker builder bake`, `git format-patch`, `terraform -backend-config`), but "only in
argc" does not mean newer: Fig was hand-written and is thin for git, gh and terraform. "Only in Fig" is partly an
artefact (aliases such as `cargo b/c/d/r`, values modelled as subcommands such as `gh config set <key>`), and Fig is
ahead for uv, npm and pnpm. Any merge of the two must be an **additive union, never a replacement**.

### How upstream builds and releases (read with `gh api`, nothing executed)

- `pnpm build` runs `npx @withfig/autocomplete-tools compile` (esbuild based, MIT, `2.11.0`, last published 2025-01-07).
  `pnpm test` is `tsc --noEmit`. Node 20, pnpm 9.1.0, `pnpm-lock.yaml` committed, `.npmrc` has `auto-install-peers=true`.
- `build.yml` runs on pushes to `master` that touch `src/**/*.ts`: `pnpm install --frozen-lockfile`, `pnpm build`, zip
  `specs/` plus `icons/` into `specs.zip`, tag `spec-build-number-*`, GitHub release, version bump, npm publish. It needs
  the secrets `FIG_BOT_TOKEN` and `NPM_PUBLISH_TOKEN`. The other workflows are `danger`, `lint`, `typecheck` and `verify-cla`.
- License: `LICENSE` is MIT, "Copyright (c) 2021 Hercules Labs Inc. (Fig)"; `package.json` says ISC (inconsistent).

### Upstream backlog (64 open PRs)

About 50 are mergeable and 13 conflict (mostly 2021 to 2024). Inflow is thin: 16 PRs from other contributors in the 17
months since the freeze, plus a burst of 12 (#2629 to #2640) from one contributor in October 2026. Worth merging after
review:

- New specs: `claude` #2610, `ollama` #2622, `starship` #2621, `mise` #2595 and #2503, `uvicorn` #2632, `pyright` #2633,
  `nox` #2636, `daphne` #2635, `cookiecutter` #2634, `emcc` #2631, `scutil` #2630, `kmutil` #2629, `xattr` #2623,
  `fastlane_cli` #2625, `hstr` #2620, `solana` #2605, `lightdash` #2601.
- Updates: `terraform` apply/init flags #2638, `gh` #2600, `ng` test #2619, `npm diff` #2637, `aws help` #2639,
  `heroku` #2567, `go` vendoring #2603, `rclone` freeze fix #2626.

I have not read any of these PRs. No maintained fork exists: the three forks pushed after the freeze (sample: top 40
forks by stars) are 0 commits ahead.

## What the fork does not give

- **Control, not freshness.** Contributors still PR upstream, so we pull from there (`gh pr list -R withfig/autocomplete`,
  `gh pr checkout <n> -R withfig/autocomplete`). With inflow this thin, the backlog is a one-off windfall; steady
  freshness means authoring updates ourselves or the overlay below.
- **A frozen build tool.** Pin `@tabsmith/autocomplete-tools@2.11.0`; vendor it if it ever breaks (MIT).
- **Free trust.** Spec generators call `executeCommand` on users' machines. Review each merged PR's generators; static-only
  specs are low risk.

## Licensing checklist

Upstream is MIT, which allows forking, renaming, changing and republishing. Its only condition is keeping the copyright
notice and licence text with every copy, the compiled bundle included. Not reviewed by a lawyer.

- **LICENSE.** Keep "Copyright (c) 2021 Hercules Labs Inc. (Fig)" and the MIT text unchanged; add our own copyright line
  below it for our changes.
- **package.json.** Rename `name` to `tabsmith-specs` and set `"license": "MIT"` (upstream says ISC, which contradicts
  `LICENSE`).
- **Name.** MIT grants no trademark rights. "Forked from withfig/autocomplete" is fine; no Fig name or logo in a way that
  suggests endorsement, and no `@withfig` npm scope.
- **New specs, by source.** Hand-written from a tool's `--help` or docs: fine (flag and subcommand names are facts;
  write descriptions in our own words). MIT or Apache-2.0 sources (argc-completions, carapace): fine, keep their
  copyright notice in the NOTICE. GPL sources (fish completions, GNU man page text): never, they would put the bundle
  under GPL terms, which clashes with tabsmith's MIT OR Apache-2.0.
- **Icons.** Upstream's release zips `icons/` too. Brand logos are their owners' trademarks; if any ship, keep the note
  from `assets/icons/README.md`.

## Steps

No Rust changes are expected, so the AGENTS.md skill-loading protocol applies only if that changes. Use one Conventional
Commit per step, in the style of `git log` (`docs/git-convention.md` is referenced by AGENTS.md but does not exist).

1. **Pick the host.** The repo is named `tabsmith-specs`. Open question: which account or org owns it? Forking is
   outward-facing, so confirm with the user first. Keep the fork relationship so upstream PR branches stay fetchable.
   Apply the LICENSE, package.json and name items of the licensing checklist in the first commit.
2. **Prove the build reproduces.** In a new empty directory, clone upstream `master` and run
   `pnpm install --frozen-lockfile --ignore-scripts` (the `prepare` script is `husky install`) and `pnpm build` with node
   20 and pnpm 9.1.0. Compare `build/` with `npm pack @withfig/autocomplete@2.692.3` (file list and hashes). If they
   match, the fork is a drop-in; if not, find out why before going on. Treat the clone as untrusted data: keep scripts
   outside it.
3. **Fork CI.** Delete `verify-cla`, `danger` and the npm publish and bump steps (they need upstream's secrets). Keep
   typecheck and lint. Build on push, on `workflow_dispatch` and on a weekly cron. Publish a release asset
   `specs.tar.zst` made the way `scripts/update-specs.sh` does today (`tar -C build -cf - . | zstd -19`), plus the
   LICENSE and a NOTICE saying it is a modified fork. Pin the build tool in the `build` script instead of a floating `npx`.
4. **Merge the backlog.** Review each PR (generators first, static specs are quick), run `pnpm test`, skip the old
   conflicting ones unless wanted. For new specs, check the source against the licensing checklist.
5. **Switch tabsmith.** `scripts/update-specs.sh` downloads the fork's npm package `@tabsmith/autocomplete-specs`,
   `SPECS_VERSION` becomes its version, and `assets/SPECS_LICENSE` stays. Update the README "Install"/license lines
   and SPEC D11 and its `assets/` row.
   SPEC says THIRD_PARTY notices are not shipped yet; decide whether this is the moment.
6. **Weekly triage.** A short routine: list new upstream PRs, review, merge, release.

## Later (not v1)

Additive overlay built from the CLIs themselves, run in the fork's CI after `pnpm build`. The argc-completions generator
(`scripts/generate.sh`, MIT) scrapes `--help` of a locally installed CLI; `argc --argc-export` (argc 1.24.0, Apache-2.0,
build-time only) turns the script into a JSON command tree. Convert that to a Fig-shaped static module and union it into
the Fig spec: add missing flags and subcommands, never remove or override. Dynamic `choice` shell functions are not
ported (Fig generators keep covering dynamic values). Not tested yet: running the generator on a CI runner, install cost
of the tools and scraping quality. Start with the easily installed tools (git, docker, gh, kubectl, cargo, npm, pnpm, uv,
terraform, brew). The argc-completions snapshot in its repo is older than Fig's, so it only fills gaps.

## Open questions

- Fork owner (step 1).
- Merge the 13 old conflicting PRs, or ignore them?
- Is the overlay worth doing once the fork exists?

## Appendix: how the argc spike was run

The scratchpad is gone after the session, so this is the recipe.

1. Extract the bundled specs into a new directory and add `{"type":"module"}` as its `package.json`.
2. Download `argc` from the GitHub release `sigoden/argc` v1.24.0 (`argc-v1.24.0-aarch64-apple-darwin.tar.gz` on this
   machine) into its own directory.
3. For each tool, fetch `completions/<tool>.sh` with `gh api repos/sigoden/argc-completions/contents/completions/<tool>.sh`
   and run `argc --argc-export <file> > <tool>.json`. This only parses the script's comment tags; nothing downloaded is
   executed.
4. Compare unique flag names. `figNames` walks the `options` and `subcommands` of `import('<fig>/<tool>.js')`; `argcNames`
   walks `flag_options` and `subcommands` of the JSON, skipping entries with `inherited: true`:

```js
const figNames = (s, out = new Set()) => { for (const o of s.options ?? []) for (const n of [].concat(o.name)) out.add(n);
  for (const c of s.subcommands ?? []) figNames(c, out); return out; };
const argcNames = (c, out = new Set()) => { for (const o of c.flag_options ?? []) { if (o.inherited) continue;
  out.add(o.long_name); if (o.short_name) out.add(o.short_name); } for (const s of c.subcommands ?? []) argcNames(s, out); return out; };
```
