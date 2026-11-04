# Native install + auto-update for tabsmith (Claude Code style)

## Context

Today tabsmith installs only from source (`cargo install --path .`, README "Install"). The goal is Claude Code's model:
one-liners for each OS that fetch a prebuilt binary, release binaries for the same OS/arch set Claude Code ships, built
and published by GitHub Actions, and native installs that update themselves in the background.

Claude Code's model (code.claude.com/docs/en/setup):
- `curl -fsSL …/install.sh | bash`, `irm …/install.ps1 | iex`,
  `curl -fsSL …/install.cmd -o install.cmd && install.cmd && del install.cmd`. Optional version arg: `bash -s 2.1.89`,
  `& ([scriptblock]::Create((irm …/install.ps1))) 2.1.89`.
- Platforms: darwin-arm64, darwin-x64, linux-x64, linux-arm64, linux-x64-musl, linux-arm64-musl, win32-x64, win32-arm64.
- Binary at `~/.local/bin/claude` (Windows `%USERPROFILE%\.local\bin\claude.exe`), SHA256 checksums for every release,
  background auto-update, a `claude update` command, and a `DISABLE_AUTOUPDATER` opt-out.

Already settled:
- `PedroLaRosa/tabsmith` is **public**, so release assets download without a token.
- `main` is **protected**: every change lands through a PR, the three `test (<os>)` CI checks must pass, conversations
  must be resolved, there are no force pushes, and the rules apply to admins too. Branch protection doesn't apply to
  tags, so the release workflow still runs on a pushed `v*` tag.
- v1 includes **background auto-update**.

This revision maps the plan onto the hexagonal layout (Phases 1–11, `docs/hexagonal-refactor-plan.md`). The first
draft named flat files (`src/update.rs`, `src/pty.rs`, `src/settings.rs`) that no longer exist.

## Key design decisions

1. **Hosting = GitHub Releases only.** Each release carries 6 binaries, `SHA256SUMS`, `VERSION`, `install.sh`,
   `install.ps1` and `install.cmd`. The public URLs are
   `https://github.com/PedroLaRosa/tabsmith/releases/latest/download/<file>`, so there is no Pages site, bucket or
   domain to run. `VERSION` (plain text such as `0.2.0`) works like Claude Code's `latest` channel file: installers and
   the updater read it, then download the pinned `releases/download/v<ver>/<file>` so every file comes from the same
   release.
2. **6 artifacts cover all 8 of Claude Code's platform slots.** Linux ships one static **musl** binary per arch. It runs
   on glibc distros (Ubuntu, Debian, **WSL**) and on Alpine, so there's no separate gnu build and no glibc-version
   floor. WSL needs nothing special: `install.sh` sees `uname -s` = `Linux`. Names follow Claude Code:
   `tabsmith-darwin-arm64`, `-darwin-x64`, `-linux-x64`, `-linux-arm64`, `-win32-x64.exe`, `-win32-arm64.exe`. These
   are raw binaries, not archives, the same as Claude Code. rquickjs-sys 0.14 ships prebuilt bindings for all 6 targets.
3. **One fixed install path, no `versions/` folder (a deliberate difference from Claude Code).** `tabsmith setup` writes
   the absolute running-binary path into the rc scripts (`RenderInitScriptUseCase` → `ShellEnvironment::current_executable`
   → `shared::infrastructure::os::process_info::exe_path`, SPEC §5.5). On Linux `current_exe` resolves symlinks, so with
   a symlink into `versions/` every rc would point at a versioned file, and deleting an old version would break shell
   startup. The binary therefore stays at `~/.local/bin/tabsmith` / `%USERPROFILE%\.local\bin\tabsmith.exe` and is
   replaced with an atomic rename:
   - Unix: `rename(tmp, exe)`. Running sessions keep the old inode. The rename also gives the file a new inode, which
     matters on macOS: overwriting a signed binary in place gets it SIGKILLed.
   - Windows: rename the running `tabsmith.exe` to `tabsmith.exe.<pid>.old` (Windows allows renaming a running image
     but not deleting it), move the new file into place, and delete `*.old` on a best-effort basis on later runs.
4. **Auto-update starts from the composition root, beside `_ pty`.** `tabsmith _ pty` starts with every terminal tab,
   the equivalent of Claude Code's startup check. `bootstrap::cli::run` starts the background update just before
   `run_pty_command`. It can't live inside `terminal_session`, because the layer rules forbid one module calling
   another module's use case. The modules hold `Rc` (not `Send`), so the thread builds its own `SelfUpdateModule` from
   `SharedDependencies::from_env()`. The thread body is wrapped in `catch_unwind`, so a panic is logged and never
   printed, even before `run_pty_session` installs its panic hook. The use case checks at most once every 24h, using
   the marker file `~/.tabsmith/update-check`. It records the check before checking, and does no JSON writes, so it
   can't race the engine's `state.json` writes. It does nothing unless the running binary is the native install
   (`~/.local/bin/tabsmith[.exe]`), so cargo installs, `target/` dev builds and package managers are left alone. The
   opt-out is `app.disableAutoupdates`. A new version takes effect in the next tab. Failures go to `pty.log`; the PTY
   logger never writes to the terminal.
5. **Never install a broken binary.** Every shell start execs this file, so a truncated download would break every new
   terminal. Before the rename, the updater and both installers check SHA256 against `SHA256SUMS`, and the updater also
   runs `tmp --version` and expects `tabsmith <ver>`. `install.sh` wraps its body in `main "$@"` so a cut-off
   `curl | sh` runs nothing.
6. **SHA256 comes from `ring`, behind a port.** ring is already compiled in through ureq → rustls (`Cargo.lock`:
   0.17.14). It becomes a direct dependency (`ring = "0.17"`, matching the Cargo.toml style; `--locked` pins it), so no
   new crate is built. The architecture test forbids external crates in domain and application
   (`ALLOWED_EXTERNAL_CRATES = ["unicode_width"]`), so hashing goes through an application port, `Sha256Digest`, with a
   `RingSha256Digest` adapter.
7. **Versions reuse `specs::domain::value_objects::semantic_version::SemanticVersion`.** It's a domain value object, so
   the module-boundary rule allows importing it from another module. "Newer" means `latest.compare(current) ==
   Greater`. A version that won't parse never updates, so the updater never downgrades or installs garbage.
8. **No code signing in v1.** Files that `curl` and `Invoke-WebRequest` download carry no quarantine or Mark-of-the-Web
   tag, so Gatekeeper and SmartScreen don't block them. On Apple Silicon the linker already applies an ad-hoc
   signature, and the release smoke test (`--version` on macos-latest arm64) catches a signature broken by `strip`.

## New module: `self_update`

A new vertical slice, registered in `src/architecture_tests.rs` `REGISTERED_MODULES`. Approving this plan is the tech
lead sign-off the hexagonal skill asks for before adding a module. Tests sit inline (`#[cfg(test)] mod tests`) or in a
sibling `<file>/tests.rs` once a file grows, as in the neighbouring modules.

```
src/self_update/
├── mod.rs
├── domain/
│   ├── self_update_error.rs          SelfUpdateError (checksum mismatch, missing checksum, bad version, not native, io)
│   ├── value_objects/
│   │   ├── release_asset.rs          ReleaseAsset::for_platform(os, arch) -> Option<…>; name() = tabsmith-<os>-<arch>[.exe]
│   │   ├── checksum_list.rs          ChecksumList::parse(SHA256SUMS text).digest_for(&asset) -> Option<&str>
│   │   ├── install_location.rs       InstallLocation::new(exe, user_home).is_native()
│   │   └── update_outcome.rs         Skipped(SkipReason) | UpToDate(version) | Updated { from, to }
│   ├── services/
│   │   └── update_schedule.rs        UpdateSchedule::is_due(last_check_secs, now_secs)   // ponytail: 24h knob
│   └── repositories/
│       └── update_check_repository.rs   trait { last_check_secs(); record_check(now_secs) } + InMemory (cfg(test))
├── application/
│   ├── update_tabsmith_use_case.rs   execute(UpdateTrigger) -> Result<UpdateOutcome, SelfUpdateError>
│   ├── update_trigger.rs             Manual (`tabsmith update`) | Scheduled (background)
│   └── ports/
│       ├── release_channel.rs        trait { latest_version(); download(version, file) } + InMemoryReleaseChannel (cfg(test))
│       ├── sha256_digest.rs          trait { hex_digest(&[u8]) -> String } + HexEncodingDigest (cfg(test))
│       └── installed_executable.rs   trait { location(); replace(bytes, &expected_version) } + InMemoryExecutable (cfg(test))
└── infrastructure/
    ├── self_update_module.rs         build_self_update_module(&SharedDependencies, settings) -> SelfUpdateModule
    ├── adapters/
    │   ├── github_release_channel.rs        ureq, 60s global timeout, REPO = "PedroLaRosa/tabsmith"
    │   ├── ring_sha256_digest.rs
    │   ├── file_installed_executable.rs     tmp next to exe → 0755 → `tmp --version` → rename (decision 3)
    │   └── marker_file_update_check_repository.rs   mtime of `~/.tabsmith/update-check`
    └── cli/
        ├── update_command.rs         `tabsmith update`
        └── background_update.rs      start_background_update(): thread + catch_unwind + logger
```

One use case with a trigger, not two use cases: a use case may not call another one, and `Manual` differs from
`Scheduled` only by the gates it skips (setting, schedule). The flow of `UpdateTabsmithUseCase::execute`:

1. `Scheduled` only: if `app.disableAutoupdates` is set, return `Skipped(Disabled)`. The setting comes from the
   `settings` domain `SettingsRepository`, which may be imported across modules. Then, if
   `!UpdateSchedule::is_due(...)`, return `Skipped(NotDue)`; otherwise `record_check(now)`.
2. If the location isn't native: `Manual` returns `Err(NotNativeInstall { path })`, and `Scheduled` returns
   `Skipped(NotNative)`.
3. Get `latest_version()`. If it's not newer, return `UpToDate`.
4. `download(v, asset)` and `download(v, "SHA256SUMS")`. If `digest_for(asset)` is missing or differs from
   `hex_digest(bytes)`, return `Err`. The executable stays untouched.
5. `replace(bytes, &v)` → `Updated { from, to }`.

The use case is built with the shared `Clock` (`dependencies.clock`); tests use `FixedClock`.

Test doubles, following testing-standard (real → in-memory fake, no mocks): `InMemoryReleaseChannel` (a version plus a
`HashMap` of files), `HexEncodingDigest` (the "digest" of `b"ok"` is `"6f6b"`, so the fake's `SHA256SUMS` stays
readable), `InMemoryExecutable` (a configurable location, the bytes it was given, an optional sanity-check failure),
`InMemoryUpdateCheckRepository`, `InMemorySettingsRepository` (already in `settings`) and `FixedClock` (already in
`shared`).

## Steps

Branch `feat/native-install` (this plan is its first commit). Load the skills `xp-tdd`, `testing-standard`,
`coding-standards` and `hexagonal-architecture` first (AGENTS.md protocol). Make one Conventional Commit per GREEN and
per REFACTOR, plus one per non-Rust step. `docs/git-convention.md`, which AGENTS.md references, doesn't exist, so follow
the style of recent `git log` (`feat(self_update): …`). Open the PR to `main` at the end; the three CI checks gate the
merge.

### 1. `self_update` module, inside-out (TDD, one test at a time)

Domain:
1. `ReleaseAsset`: macos/aarch64 → `tabsmith-darwin-arm64`.
2. `ReleaseAsset`: x86_64 → `x64`; linux → `linux`.
3. `ReleaseAsset`: windows → `win32` with `.exe`.
4. `ReleaseAsset`: an unsupported OS or arch → `None`.
5. `ChecksumList`: finds the digest for an asset in `sha256sum` output.
6. `ChecksumList`: reads binary-mode lines (`<hex> *<file>`).
7. `ChecksumList`: an asset that isn't listed → `None`.
8. `UpdateSchedule`: never checked → due.
9. `UpdateSchedule`: checked under 24h ago → not due.
10. `UpdateSchedule`: 24h or more → due.
11. `UpdateSchedule`: a last check in the future (clock moved back) → due.
12. `InstallLocation`: `<home>/.local/bin/tabsmith` is native; `target/release/tabsmith` and `~/.cargo/bin/tabsmith`
    are not; on Windows the `.exe` name is native.

Application (`UpdateTabsmithUseCase`, all on fakes):

13. Latest equals current → `UpToDate`, and nothing is downloaded.
14. Latest is newer and the checksum matches → executable replaced with those bytes, `Updated { from, to }`.
15. Checksum mismatch → `Err`, executable untouched.
16. Asset missing from `SHA256SUMS` → `Err`, executable untouched.
17. Unparseable latest version → no download, executable untouched.
18. `Manual` on a non-native install → `Err(NotNativeInstall)`.
19. `Scheduled` with `app.disableAutoupdates` → `Skipped(Disabled)`, no check recorded.
20. `Scheduled` before 24h → `Skipped(NotDue)`, no channel call.
21. `Scheduled` when due → the check is recorded even if the channel fails.

Infrastructure:

22. `RingSha256Digest`: `"abc"` → `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`.
23. `FileInstalledExecutable` (Unix-only, real `shared::infrastructure::testing::TempDir`): the new bytes are a
    `#!/bin/sh` script echoing `tabsmith 9.9.9`, and they replace the exe.
24. `FileInstalledExecutable`: a `--version` mismatch leaves the exe untouched and no temp file behind.
25. `MarkerFileUpdateCheckRepository`: nothing recorded → `None`; after `record_check` → `Some`.

`GithubReleaseChannel` gets no unit test because it's thin; the release `verify` job (step 4) covers it end to end.
It reads `releases/latest/download/VERSION`, then `releases/download/v{ver}/{file}`, using a ureq agent built like
`ghost_text::infrastructure::adapters::http_llm_client` but with a 60s global timeout.

The binary swap doesn't reuse `shared::infrastructure::atomic_file::AtomicFile`, because the `--version` check has to
run between the write and the rename, and none of its methods leave that gap. Mark this with a `ponytail:` comment.

### 2. Wire into the CLI, settings, shared kernel and PTY start

- `src/main.rs`: `mod self_update;`.
- `src/architecture_tests.rs`: add `"self_update"` to `REGISTERED_MODULES`.
- `src/bootstrap/cli.rs`:
  - `/// Update tabsmith to the latest release` `Update` in `Command`, dispatching to `run_update_command(&self_update_module())`.
  - `InternalCmd::Pty` → `start_background_update(); run_pty_command(...)`.
  - A `self_update_module()` builder next to the others (`SharedDependencies::from_env()` + `JsonFileSettingsRepository`).
- `update_command.rs` prints `Updated tabsmith from X to Y. Open a new terminal to use it.` or
  `tabsmith is up to date (X)`. On `NotNativeInstall` it prints
  `tabsmith runs from <path>, not ~/.local/bin; update it the way you installed it (e.g. cargo install --path .)` and
  exits 1.
- `src/settings/domain/services/settings_catalog.rs`: `APP_DISABLE_AUTOUPDATES = "app.disableAutoupdates"` and
  `SettingDefinition::new(APP_DISABLE_AUTOUPDATES, "bool", "false", "Never update in the background (tabsmith update still works)")`,
  plus a default test like `should_default_autocomplete_to_not_disabled`.
- `src/shared/domain/value_objects/data_directory.rs`: `update_check_file()` → `<root>/update-check`, plus its test
  (and add it to `should_fail_every_path_under_an_unknown_root`).
- `Cargo.toml`: `ring = "0.17"`.
- `docs/SPEC.md`:
  - §1: new F-row (native installer + auto-update: MUST)
  - §0.4: decisions 3 and 4 (fixed path, no `versions/`, started from the composition root)
  - §2.2 and §2.10: `update`
  - §3.1: `update-check`
  - §3.3: the new setting
  - §4.1: the `self_update` module
  - §4.2: `ring`
  - §4.3 and §4.4: the update thread at `_ pty` start
- `docs/ubiquitous-language.md`: a `## self_update` section with these terms: *native install*, *release channel*,
  *release asset*, *checksum list*, *update check*, *update trigger*.

### 3. Installer scripts in `scripts/` (next to `update-specs.sh`)

- **`install.sh`** (POSIX sh, so `| sh` also works on Alpine without bash; body in `main "$@"`):
  1. Version comes from `$1`, default `latest`, which is resolved through `VERSION`.
  2. OS from `uname -s` (Darwin/Linux, which includes WSL; anything else gets told to use `install.ps1`). Arch from
     `uname -m` (`x86_64|amd64` → x64, `arm64|aarch64` → arm64). Under Rosetta (`sysctl -n sysctl.proc_translated` = 1)
     it picks arm64.
  3. `curl -fsSL` the binary and `SHA256SUMS` into a temp file inside `~/.local/bin`, so the rename stays on one
     filesystem. Verify with `sha256sum`, or `shasum -a 256` on macOS. `chmod 755`, then `mv -f` over
     `~/.local/bin/tabsmith`.
  4. Print `tabsmith --version`. If `~/.local/bin` isn't on PATH, print a hint (the rc integration uses the absolute
     path, so autocompletion works either way).
  5. On a first install only, run `tabsmith setup </dev/tty` when `/dev/tty` is readable (stdin is the piped script).
     Otherwise print `Run: tabsmith setup`.
- **`install.ps1`** (`param([string]$Version = 'latest')`, works on Windows PowerShell 5.1 and pwsh 7):
  1. Turn on TLS 1.2. Arch from `[RuntimeInformation]::OSArchitecture`, which is correct for x64 PowerShell under
     ARM64 emulation.
  2. `Invoke-WebRequest -UseBasicParsing` the binary and `SHA256SUMS`, then compare with `Get-FileHash`.
  3. Same swap as decision 3, into `%USERPROFILE%\.local\bin`.
  4. Add that directory to the user PATH (HKCU via `[Environment]::SetEnvironmentVariable(...,'User')`) and to
     `$env:Path` if it's missing.
  5. On a first install, run `tabsmith setup` unless `[Console]::IsInputRedirected` (CI).
- **`install.cmd`**: `@echo off` plus a single line,
  `powershell -NoProfile -ExecutionPolicy Bypass -Command "& ([scriptblock]::Create((irm https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.ps1))) %*"`.

### 4. GitHub Actions

- **`ci.yml`**: change `push:` to `push: { branches: ['**'] }` so tags don't run it twice, and add `workflow_call:` so
  the release workflow can reuse it as a gate. The job key and matrix stay the same, so the required checks
  `test (ubuntu-latest)`, `test (macos-latest)` and `test (windows-latest)` keep their names, and branch protection
  keeps working.
- **New `release.yml`**: runs `on: push: tags: ['v*']`, plus `pull_request` with `paths: ['.github/workflows/release.yml', 'scripts/install.*']`
  for a dry run of the build matrix inside the PR. `workflow_dispatch` alone can't do that dry run, because GitHub only
  dispatches workflows that already exist on the default branch. `permissions: contents: write`. Action versions match
  ci.yml (`actions/checkout@v4`, `dtolnay/rust-toolchain@stable` with `targets:`, `actions/upload-artifact@v4` and
  `download-artifact@v4`).
  - `test`: `uses: ./.github/workflows/ci.yml`.
  - `build` (`needs: test`), matrix `include`:

    | platform | target | runner |
    |---|---|---|
    | darwin-arm64 | aarch64-apple-darwin | macos-latest |
    | darwin-x64 | x86_64-apple-darwin | macos-latest (cross) |
    | linux-x64 | x86_64-unknown-linux-musl | ubuntu-latest + `musl-tools` |
    | linux-arm64 | aarch64-unknown-linux-musl | ubuntu-24.04-arm + `musl-tools` |
    | win32-x64 | x86_64-pc-windows-msvc | windows-latest |
    | win32-arm64 | aarch64-pc-windows-msvc | windows-11-arm |

    Env: `MACOSX_DEPLOYMENT_TARGET=11.0`. On Windows targets, `RUSTFLAGS=-C target-feature=+crt-static`, so the binary
    doesn't need `vcruntime140.dll` on a fresh Windows. Steps: `cargo build --locked --release --target $T`, a smoke
    test of `--version` on every row except darwin-x64, a rename to `tabsmith-<platform>[.exe]`, then upload.
  - `publish` (`needs: build`, `if: startsWith(github.ref, 'refs/tags/v')`):
    1. Check that the tag equals `v` + the version in Cargo.toml.
    2. Download all artifacts.
    3. `echo <ver> > VERSION`, `sha256sum tabsmith-* > SHA256SUMS`, copy `scripts/install.*`.
    4. `gh release create "$GITHUB_REF_NAME" --generate-notes dist/*`. gh uploads the assets before the release goes
       live, so `latest/download` never points at a release with missing files.
  - `verify` (`needs: publish`), on the same 6 runners plus an `alpine` container job (`apk add curl`, `| sh -s`):
    1. Run the real one-liner with the explicit version: `install.sh`, `install.ps1`, and `install.cmd` on Windows.
    2. Assert that `~/.local/bin/tabsmith --version` equals the tag.
    3. If an earlier release exists (`gh release list`), install it, run `tabsmith update`, and assert the new version.
       This is the end-to-end test of the updater and `GithubReleaseChannel`, and it starts working from the second
       release.

    If `verify` fails, run `gh release delete <tag>` and fix forward.

### 5. Docs

`README.md`:
- **Install**: the three one-liners (`https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.sh` …),
  the version-pinned forms, and the supported platforms (macOS 11+ x64/arm64; any Linux or WSL x64/arm64, including
  Alpine; Windows 10+ x64/arm64). From source stays as an alternative.
- **Updating**: automatic for native installs, `tabsmith update`, `tabsmith settings app.disableAutoupdates true`.
- **Uninstall**: `tabsmith uninstall`, then delete `~/.local/bin/tabsmith` (Windows: `tabsmith.exe*`).
- **Releasing** (with `main` protected): open a PR that bumps `version` in `Cargo.toml` and merge it once CI is green.
  Then, on the merged `main`, run `git tag vX.Y.Z && git push origin vX.Y.Z`.

## Files

- New: `src/self_update/**` (tree above), `scripts/install.sh`, `scripts/install.ps1`, `scripts/install.cmd`,
  `.github/workflows/release.yml`
- Modified: `src/main.rs`, `src/architecture_tests.rs`, `src/bootstrap/cli.rs`,
  `src/settings/domain/services/settings_catalog.rs`, `src/shared/domain/value_objects/data_directory.rs`,
  `Cargo.toml`, `Cargo.lock`, `.github/workflows/ci.yml`, `README.md`, `docs/SPEC.md`, `docs/ubiquitous-language.md`
- Reused:
  - `SharedDependencies` (data directory and `Clock`, with `FixedClock` in tests)
  - `SemanticVersion` (`specs` domain)
  - `SettingsRepository` / `InMemorySettingsRepository` and `JsonFileSettingsRepository` (`settings`)
  - `shared::infrastructure::os::process_info::exe_path` and `data_directory_from_env::user_home_from_env`
  - `shared::infrastructure::logger`
  - `shared::infrastructure::testing::TempDir`
  - the ureq agent pattern in `ghost_text::infrastructure::adapters::http_llm_client`
  - the existing `setup --no-confirm` flag

## Verification

1. `cargo test --locked`: the new `self_update` tests pass, `architecture_tests` stays green with the module
   registered, and the existing suite and `init_*` snapshots stay green.
2. `sh -n scripts/install.sh`. Optionally run `shellcheck`.
3. The PR's `release.yml` dry run: all 6 build rows must pass. The rows most likely to fail first are `win32-arm64`,
   where ring needs clang (if it's missing, add an LLVM install step), and the musl rows, where ring/zstd/quickjs need
   `musl-gcc`.
4. Merge, then tag `v0.1.0`: `verify` must pass on macOS, Linux x64/arm64, Alpine, and Windows x64/arm64 (ps1 + cmd).
5. Manual, on this Mac and in a WSL distro: run the `curl … | sh` one-liner, answer the `setup` prompt, open a new tab,
   and confirm the dropdown appears and `tabsmith doctor` is clean.
6. Next release `v0.1.1` (bump PR, merge, tag): the update leg of `verify` must pass. On the Mac, delete
   `~/.tabsmith/update-check`, open a tab, wait a few seconds, open another, and check that `tabsmith --version` reports
   `0.1.1` and `~/.tabsmith/logs/pty.log` shows the update.

## Skipped (add when needed)

- Homebrew cask / WinGet / apt-dnf-apk repos: add once there are users asking for them.
- A short install URL (e.g. `tabsmith.dev/install.sh`): add with a domain; it's only a redirect to the release asset.
- `stable` / `latest` channels: add once releases need a soak period.
- macOS notarization and Windows Authenticode: add if Gatekeeper or SmartScreen complaints show up (the certificates
  cost money).
- A GPG-signed manifest: add if a mirror or CDN goes in front of GitHub.
- A `doctor` row for the last update result: add if users can't find the logs.
- Locking between two tabs that check at the same moment: both download, both renames are atomic, and the last one
  wins with the same bytes. Add a lock if the logs show it causing trouble.
