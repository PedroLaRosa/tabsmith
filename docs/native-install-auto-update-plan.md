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

Decisions already made: releases go on a **public** GitHub repo, so release assets download without a token. v1
includes **background auto-update**.

## Key design decisions

1. **Hosting = GitHub Releases only.** Each release carries 6 binaries, `SHA256SUMS`, `VERSION`, `install.sh`,
   `install.ps1` and `install.cmd`. The public URLs are `https://github.com/OWNER/tabsmith/releases/latest/download/<file>`,
   so there is no Pages site, bucket or domain to run. `VERSION` (plain text such as `0.2.0`) works like Claude Code's
   `latest` channel file: installers and the updater read it, then download the pinned
   `releases/download/v<ver>/<file>` so every file comes from the same release.
2. **6 artifacts cover all 8 of Claude Code's platform slots.** Linux ships one static **musl** binary per arch. It runs
   on glibc distros (Ubuntu, Debian, WSL) and on Alpine, so there's no separate gnu build and no glibc-version floor.
   Names follow Claude Code: `tabsmith-darwin-arm64`, `-darwin-x64`, `-linux-x64`, `-linux-arm64`, `-win32-x64.exe`,
   `-win32-arm64.exe`. These are raw binaries, not archives, the same as Claude Code. rquickjs-sys ships prebuilt
   bindings for all 6 targets (checked in `~/.cargo/registry/.../rquickjs-sys-0.14.0/src/bindings`).
3. **One fixed install path, no `versions/` folder (a deliberate difference from Claude Code).** `tabsmith setup`
   writes the absolute `current_exe()` path into the rc scripts (`src/integrations.rs:201` `exe_path`, SPEC §5.5). On
   Linux `current_exe` resolves symlinks, so with a symlink into `versions/` every rc would point at a versioned file,
   and deleting an old version would break shell startup. The binary therefore stays at `~/.local/bin/tabsmith` /
   `%USERPROFILE%\.local\bin\tabsmith.exe` and is replaced with an atomic rename:
   - Unix: `rename(tmp, exe)`. Running sessions keep the old inode. The rename also gives the file a new inode, which
     matters on macOS: overwriting a signed binary in place gets it SIGKILLed.
   - Windows: rename the running `tabsmith.exe` to `tabsmith.exe.<pid>.old` (Windows allows renaming a running image
     but not deleting it), move the new file into place, and delete `*.old` on a best-effort basis on later runs.
4. **Auto-update runs inside `tabsmith _ pty`.** That process starts with every terminal tab, the equivalent of Claude
   Code's startup check. It spawns a background thread that checks at most once every 24h, using the mtime of the
   marker file `~/.tabsmith/update-check`. It touches the marker before checking, and does no JSON writes, so it can't
   race the engine's `state.json` writes. It does nothing unless `current_exe()` is in `~/.local/bin`: cargo installs,
   `target/` dev builds and package managers are left alone. Opt-out is `app.disableAutoupdates` (Kiro's key, per
   `docs/research/cli-surface.md:274`, without Kiro's inverted-logic bug from `figterm.md:943`). A new version takes
   effect in the next tab. Failures go to the logs; nothing is ever printed over the user's screen.
5. **Never install a broken binary.** Every shell start execs this file, so a truncated download would break every new
   terminal. Before the rename, the updater and both installers check SHA256 against `SHA256SUMS`, and the updater also
   runs `tmp --version` and expects `tabsmith <ver>`. `install.sh` wraps its body in `main "$@"` so a cut-off
   `curl | bash` runs nothing.
6. **SHA256 comes from `ring`**, which is already compiled in through ureq → rustls. It becomes a direct dependency
   (`ring = "0.17"`, matching the Cargo.toml style; `Cargo.lock` and `--locked` pin it), so no new crate is built.
7. **No code signing in v1.** Files that `curl` and `Invoke-WebRequest` download carry no quarantine or Mark-of-the-Web
   tag, so Gatekeeper and SmartScreen don't block them. On Apple Silicon the linker already applies an ad-hoc
   signature, and the release smoke test (`--version` on macos-latest arm64) catches a signature broken by `strip`.

## Steps

Branch `feat/native-install`. At step 0, load the skills `xp-tdd`, `testing-standard`, `coding-standards` and
`hexagonal-architecture` (AGENTS.md protocol). Make one Conventional Commit per GREEN and per REFACTOR, plus one per
non-Rust step. `docs/git-convention.md`, which AGENTS.md references, doesn't exist, so follow the style of recent
`git log` (`feat(doctor): …`).

### 1. `src/update.rs`: updater (TDD, inside-out)

One flat module, like `doctor.rs`, with tests inline in `#[cfg(test)] mod tests` as in the neighbouring files. The
module follows the hexagonal dependency direction inside one file:

- **Domain (pure, no I/O):**
  - `asset_name()`: chosen per `cfg` target and returns `tabsmith-<os>-<arch>[.exe]`.
  - `is_newer(latest, current)`: numeric `x.y.z` comparison. Anything unparseable returns false, so it never
    downgrades or "updates" to garbage.
  - `checksum_for(sums, asset)`: parses `sha256sum` output.
  - `is_due(last_check, now)`: the 24h rule. Mark it with a `ponytail:` comment as a knob.
- **Port:** `trait Releases { fn latest_version(&self) -> Result<String>; fn fetch(&self, version: &str, file: &str) -> Result<Vec<u8>>; }`
  The in-memory `FakeReleases` test double (a HashMap of files plus a version) lives next to it, so the use-case tests
  need no mocks.
- **Use case:** `update(releases, exe, current) -> Result<Outcome { UpToDate(v) | Updated { from, to } }>` runs check →
  fetch binary and `SHA256SUMS` → verify → `install(exe, bytes, version)`. `install` writes a pid-unique temp file next
  to the exe, chmods it to 755, runs the `--version` sanity check and then does the rename swap (decision 3).
- **Adapter:** `GithubReleases`, a ureq agent with a 60s global timeout. It reads `releases/latest/download/VERSION`,
  then `releases/download/v{ver}/{file}`. The constant `REPO = "OWNER/tabsmith"` lives here. It has no unit test
  because it's thin; release verification (step 4) covers it.
- **Entry points:**
  - `update_cmd()` for `tabsmith update`. It prints `Updated tabsmith from X to Y. Open a new terminal to use it.` or
    `tabsmith is up to date (X)`. On a non-native install it says how it was installed and exits 1.
  - `spawn_background()`: `app.disableAutoupdates` → native-install gate → marker → thread → `update`, logging through
    `crate::logger`.

TDD order, one test at a time:
1. `is_newer`
2. `asset_name`
3. `checksum_for`
4. `is_due`
5. use case: already up to date
6. use case: checksum mismatch leaves the exe untouched
7. use case: success swaps the bytes. Unix-only test: the fake serves a `#!/bin/sh` script that echoes
   `tabsmith 9.9.9`, written into a real temp dir.
8. use case: a sanity-check mismatch leaves the exe untouched
9. native-install gate: an exe in `~/.local/bin` is native, any other path is not

### 2. Wire into the CLI, settings and PTY

- `src/main.rs`: add `/// Update tabsmith to the latest release` `Update` to `Command`, dispatching to
  `update::update_cmd()`, and add `mod update;`.
- `src/settings.rs` `CATALOG`: add `entry("app.disableAutoupdates", "bool", "false", "Never update in the background (tabsmith update still works)")`.
  Extend the existing catalog test if one lists the keys.
- `src/pty.rs` `run()` (line 44): call `crate::update::spawn_background()` before `Session::start`.
- `Cargo.toml`: `ring = "0.17"`.
- `docs/SPEC.md`:
  - §1: new F-row (native installer + auto-update: MUST)
  - §0.4: decision on the fixed path and no `versions/`
  - §2.2 and §2.10: `update`
  - §3.1: `update-check` marker
  - §3.3: the new setting
  - §4.1: `update.rs`
  - §4.2: `ring`
  - §4.4: the update thread at `_ pty` start

### 3. Installer scripts in `scripts/` (next to `update-specs.sh`)

- **`install.sh`** (POSIX sh, so `| sh` also works on Alpine without bash; body in `main "$@"`):
  1. Version comes from `$1`, default `latest`, which is resolved through `VERSION`.
  2. OS from `uname -s` (Darwin/Linux; anything else gets told to use `install.ps1`). Arch from `uname -m`
     (`x86_64|amd64` → x64, `arm64|aarch64` → arm64). Under Rosetta (`sysctl -n sysctl.proc_translated` = 1) it picks
     arm64.
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
  `powershell -NoProfile -ExecutionPolicy Bypass -Command "& ([scriptblock]::Create((irm https://github.com/OWNER/tabsmith/releases/latest/download/install.ps1))) %*"`.

### 4. GitHub Actions

- **`ci.yml`**: change `push:` to `push: { branches: ['**'] }` so tags don't run it twice, and add `workflow_call:` so
  the release workflow can reuse it as a gate.
- **New `release.yml`**: runs `on: push: tags: ['v*']` and `workflow_dispatch` (a dry run of the build matrix with no
  tag needed). `permissions: contents: write`. Action versions match ci.yml (`actions/checkout@v4`,
  `dtolnay/rust-toolchain@stable` with `targets:`, `actions/upload-artifact@v4` and `download-artifact@v4`).
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
  - `publish` (`needs: build`, only on tags):
    1. Check that the tag equals `v` + the version in Cargo.toml.
    2. Download all artifacts.
    3. `echo <ver> > VERSION`, `sha256sum tabsmith-* > SHA256SUMS`, copy `scripts/install.*`.
    4. `gh release create "$GITHUB_REF_NAME" --generate-notes dist/*`. gh uploads the assets before the release goes
       live, so `latest/download` never points at a release with missing files.
  - `verify` (`needs: publish`), on the same 6 runners plus an `alpine` container job (`apk add curl`, `| sh -s`):
    1. Run the real one-liner with the explicit version: `install.sh`, `install.ps1`, and `install.cmd` on Windows.
    2. Assert that `~/.local/bin/tabsmith --version` equals the tag.
    3. If an earlier release exists (`gh release list`), install it, run `tabsmith update`, and assert the new version.
       This is the end-to-end test of the updater and the GitHub adapter, and it starts working from the second
       release.

    If `verify` fails, run `gh release delete <tag>` and fix forward.

### 5. Docs

`README.md`:
- **Install**: the three one-liners, the version-pinned forms, and the supported platforms (macOS 11+ x64/arm64; any
  Linux or WSL x64/arm64, including Alpine; Windows 10+ x64/arm64). From source stays as an alternative.
- **Updating**: automatic for native installs, `tabsmith update`, `tabsmith settings app.disableAutoupdates true`.
- **Uninstall**: `tabsmith uninstall`, then delete `~/.local/bin/tabsmith` (Windows: `tabsmith.exe*`).
- **Releasing**: bump `version` in `Cargo.toml`, commit, then `git tag vX.Y.Z && git push origin vX.Y.Z`.

## Files

- New: `src/update.rs`, `scripts/install.sh`, `scripts/install.ps1`, `scripts/install.cmd`, `.github/workflows/release.yml`
- Modified: `src/main.rs`, `src/settings.rs`, `src/pty.rs`, `Cargo.toml`, `Cargo.lock`, `.github/workflows/ci.yml`,
  `README.md`, `docs/SPEC.md`
- Reused: `settings::home()`, `settings::user_home()`, `Settings::load().bool(..)`, `integrations::exe_path()`,
  `crate::logger::{warn,info}`, the ureq agent pattern in `src/inline.rs:345`, and the `setup --no-confirm` flag that
  already exists.

## Open input

Replace `OWNER/tabsmith` with the real repo slug once the GitHub repo exists. It appears once in each of `install.sh`,
`install.ps1`, `install.cmd`, `src/update.rs` and `README.md` (`grep -rn OWNER/`).

## Verification

1. `cargo test --locked`: the new `update` tests pass and the existing suite and `init_*` snapshots stay green.
2. `sh -n scripts/install.sh`. Optionally run `shellcheck`.
3. Dry run: `gh workflow run release.yml --ref feat/native-install`. All 6 build rows must pass. The rows most likely to
   fail first are `win32-arm64`, where ring needs clang (if it's missing, add an LLVM install step), and the musl rows,
   where ring/zstd/quickjs need `musl-gcc`.
4. Tag `v0.1.0`: `verify` must pass on macOS, Linux x64/arm64, Alpine, and Windows x64/arm64 (ps1 + cmd).
5. Manual on this Mac: run the `curl … | bash` one-liner, answer the `setup` prompt, open a new tab, and confirm the
   dropdown appears and `tabsmith doctor` is clean.
6. Next tag `v0.1.1`: the update leg of `verify` must pass. On the Mac, delete `~/.tabsmith/update-check`, open a tab,
   wait a few seconds, open another, and check that `tabsmith --version` reports `0.1.1` and `~/.tabsmith/logs` shows
   the update.

## Skipped (add when needed)

- Homebrew cask / WinGet / apt-dnf-apk repos: add once there are users asking for them.
- `stable` / `latest` channels: add once releases need a soak period.
- macOS notarization and Windows Authenticode: add if Gatekeeper or SmartScreen complaints show up (the certificates
  cost money).
- A GPG-signed manifest: add if a mirror or CDN goes in front of GitHub.
- A `doctor` row for the last update result: add if users can't find the logs.
