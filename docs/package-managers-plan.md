# Homebrew (macOS + Linux) and Chocolatey (Windows) for tabsmith

Handoff plan for an agent session that implements it. Read `AGENTS.md` first: load the `xp-tdd`, `testing-standard`,
`coding-standards` and `hexagonal-architecture` skills before touching code, and commit after each task. `main` is
protected, so each PR below is its own branch and pull request.

## Context

tabsmith ships through the one-liners (`scripts/install.{sh,ps1,cmd}`), which put the release binary at
`~/.local/bin/tabsmith` where it updates itself. The goal is two more channels:

- `brew install pedrolarosa/tap/tabsmith` on macOS and Linux
- `choco install tabsmith` on Windows (x64 and ARM64)

Facts the plan rests on (checked 2026-11-17/18):

- **Release assets already fit.** `release.yml` publishes six raw binaries (`tabsmith-{darwin,linux}-{arm64,x64}`,
  `tabsmith-win32-{arm64,x64}.exe`) plus `SHA256SUMS` (signed, attested, immutable release). No build changes.
- **Self-update already leaves package-manager copies alone.** `InstallLocation::is_native()` only accepts
  `~/.local/bin/tabsmith[.exe]`; the background check skips anything else and `tabsmith update` refuses it
  (`update_tabsmith_use_case.rs:74`).
- **Bug to fix first (Linux Homebrew).** The shell integration writes `std::env::current_exe()` into the dotfiles as
  `<BIN>` (SPEC §5.5). On Linux that is the resolved, versioned keg
  (`/home/linuxbrew/.linuxbrew/Cellar/tabsmith/1.0.0/bin/tabsmith`), which `brew upgrade` deletes. The dotfile guard
  `[ -x '<BIN>' ]` then silently skips tabsmith in every new shell. macOS reports the invoked path
  (`/opt/homebrew/bin/tabsmith`), so it is mostly fine. Chocolatey's real exe path (`lib\tabsmith\tools\tabsmith.exe`)
  has no version in it.
- **No licence file.** `Cargo.toml` says `MIT OR Apache-2.0` but the repo has no LICENSE file. Chocolatey needs
  `legal/LICENSE.txt` to embed binaries.
- **homebrew-core is out for now.** Self-submission needs ≥225 stars, 90 forks or 90 watchers (Package Acceptance
  Policy); the repo has 0. Hence a tap. homebrew-core would also require building from source.
- **Formula, not cask.** The binaries are not signed or notarized: casks get the quarantine flag and Homebrew is
  disabling casks that fail Gatekeeper; Linux cask support is still limited. A formula downloads with curl (no
  quarantine) and one file covers macOS and Linux.
- **Homebrew tap trust (6.0+; this machine runs 7.0.13).** A fully qualified `brew install user/tap/formula` trusts
  that formula only; anything else on the tap needs `brew trust`.
- **Chocolatey on Windows ARM64.** Chocolatey currently treats ARM64 hosts as 32-bit and `PROCESSOR_ARCHITECTURE`
  reports `AMD64` under emulation (choco PR #3907, still open). Detect ARM64 with `Win32_Processor.Architecture -eq 12`,
  as `install.ps1`'s `Get-TabsmithArch` does, not with Chocolatey's helpers.
- **Chocolatey moderation.** Every version gets a human review until the package is trusted; automated checks start
  ~30 min after a push. Unsigned binaries risk antivirus false positives, so the first version may be held.
- **Watch.** JumpCloud reports that from 2027-01-01 organisations must stop using the Chocolatey community repository
  directly (Chocolatey's own notice not found; personal use seems unaffected). Homebrew drops Intel macOS in Sept 2027.

## Done already (2026-11-18)

- **Tap repo** `PedroLaRosa/homebrew-tap` (public; first named `homebrew-tabsmith`, renamed so the install command
  does not repeat tabsmith), `Formula/tabsmith.rb` for v1.0.0 plus a short README.
  The four sha256 values match the attested v1.0.0 `SHA256SUMS`. Checked on macOS arm64:
  `brew install pedrolarosa/tap/tabsmith` installs and prints `tabsmith 1.0.0`, `brew test` passes,
  `brew audit --strict --online` is clean, `brew style` is clean. No `version` line: audit flags it as redundant
  with the `/v1.0.0/` URL. The test install and tap were removed afterwards.
- **Deploy key** "tabsmith release workflow (packages environment)" on the tap, read-write. Its private half exists only
  as the secret `HOMEBREW_TAP_DEPLOY_KEY`.
- **Environment** `packages` on `PedroLaRosa/tabsmith`: deployments limited to `main`, no required reviewer (the
  `release` job was already approved). It holds `HOMEBREW_TAP_DEPLOY_KEY`; `CHOCO_API_KEY` is still to add.

The current formula, which PR 2 turns into a template:

```ruby
class Tabsmith < Formula
  desc "IDE-style autocomplete for your terminal: dropdown menu and inline ghost text"
  homepage "https://github.com/PedroLaRosa/tabsmith"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/PedroLaRosa/tabsmith/releases/download/v1.0.0/tabsmith-darwin-arm64"
      sha256 "8b87a068fcee65bed2359541a03f2a39e06f5f376d49c998a17bdd9303eb48a5"
    end
    on_intel do … tabsmith-darwin-x64 … end
  end
  on_linux do
    on_arm do … tabsmith-linux-arm64 … end
    on_intel do … tabsmith-linux-x64 … end
  end

  def install
    bin.install Dir["tabsmith-*"].first => "tabsmith"
  end

  def caveats
    # setup / brew upgrade / tabsmith uninstall lines, plus on Linux only:
    # "On Linux, run tabsmith doctor after each brew upgrade to repoint the shell integration."
  end

  test do
    assert_equal "tabsmith #{version}", shell_output("#{bin}/tabsmith --version").strip
  end
end
```

## PR 1: prerequisites in this repo

### 1a. Stable `<BIN>` path (TDD)

Both places that write `<BIN>` sit in `src/modules/shell_integration/infrastructure/adapters/os_shell_environment.rs`:
`current_executable()` (line ~30, the init script) and `integration_paths()` (line ~74, the dotfile lines). Both call
the shared `exe_path()` (`shared/infrastructure/os/process_info.rs`). Do **not** change `exe_path()`: `self_update`
uses it too (`self_update_module.rs:30`) and must keep the real file.

Add one helper in the adapter and use it in both places: the first `tabsmith[.exe]` on `PATH`, when it is the same file
as `current_exe()`, else `current_exe()`.

```rust
// ponytail: first PATH hit only; Homebrew's bin/ link outlives the Cellar path that brew upgrade deletes
fn stable_exe_path() -> io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let on_path = std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|dir| dir.join(exe.file_name().unwrap_or_default()))
            .find(|candidate| candidate.is_file())
    });
    let same_file = |p: &Path| std::fs::canonicalize(p).ok() == std::fs::canonicalize(&exe).ok();
    Ok(on_path.filter(|p| same_file(p)).unwrap_or(exe))
}
```

Pass `PATH` and the exe in as arguments if that is what makes it testable without touching the process environment.
Tests (Unix, real temp dir):

- A `PATH` dir holding a symlink to the binary gives the symlink path.
- A `PATH` whose first `tabsmith` is a different file gives `current_exe()`.
- No `tabsmith` on `PATH` gives `current_exe()`.

Copies in `~/.local/bin` resolve to the same path as before, so `doctor` sees no change for them (except on Linux
when `~/.local` or `~/.local/bin` is a link: `current_exe()` was the resolved path, so `doctor` repoints the scripts
once). Update SPEC §5.5
(the `<BIN>` paragraph) and, if the decision table covers it, add a D-row.

### 1b. Update message

`SelfUpdateError::NotNativeInstall` (`self_update/domain/self_update_error.rs:28`) says
`update it the way you installed it (e.g. cargo install --path .)`. Name `brew upgrade tabsmith` and
`choco upgrade tabsmith` too, and update the test that checks the message.

### 1c. Licence files

Add `LICENSE-MIT` and `LICENSE-APACHE` at the root (standard texts; copyright holder as in the git history).

## PR 2: Homebrew automation

1. **Template.** `packaging/homebrew/tabsmith.rb` = the tap's formula with `@VERSION@`, `@DARWIN_ARM64@`,
   `@DARWIN_X64@`, `@LINUX_ARM64@` and `@LINUX_X64@` placeholders. Keep the Linux `tabsmith doctor` caveat while
   upgrades from 1.0.0 are likely (1.0.0 users need one `doctor` after that upgrade, whichever later version they
   land on), then delete it.
2. **Job `homebrew`** in `release.yml`:
   - `needs: [publish, verify]`, so only a verified release reaches Homebrew users.
   - `environment: packages`, default read-only token.
   - Download `SHA256SUMS` from the published release (`gh release download "v$VERSION" --pattern SHA256SUMS`), not
     from the build artifacts: it is what users download.
   - Fill in the template: `sum() { awk -v a="tabsmith-$1" '$2 == a { print $1 }' SHA256SUMS; }` and `sed`.
     Fail if any value is empty.
   - Push over SSH with `HOMEBREW_TAP_DEPLOY_KEY`. Use plain `ssh-agent`/`ssh-add` in bash, no new action. Pin
     GitHub's published ed25519 host key in `known_hosts` instead of `ssh-keyscan`. Commit message
     `feat(formula): update tabsmith to $VERSION`.
   - Pin every action to a full commit SHA, like the rest of the workflow.
3. **Job `verify-brew`**:
   - `needs: homebrew`; matrix `macos-latest`, `ubuntu-latest`, `ubuntu-24.04-arm`.
   - Set up Homebrew where it isn't preinstalled (pinned `Homebrew/actions/setup-homebrew`).
   - Run `brew install pedrolarosa/tap/tabsmith` and check that `"$(brew --prefix)/bin/tabsmith" --version`
     prints `tabsmith $VERSION`.
   - Run `brew test` and `brew audit --strict --online`.
4. **README.**
   - Install: `brew install pedrolarosa/tap/tabsmith` then `tabsmith setup`.
   - Updating: Homebrew copies update with `brew upgrade tabsmith`.
   - Uninstall: `tabsmith uninstall`, then `brew uninstall tabsmith`.
   - Releasing: the `packages` environment and its secrets.

## PR 3: Chocolatey

Package in `packaging/chocolatey/`:

- `tabsmith.nuspec`:
  - `id` tabsmith, `version`, `title`, `authors`, `owners`.
  - `projectUrl`, `projectSourceUrl`, `packageSourceUrl`, `docsUrl`, `bugTrackerUrl`, `licenseUrl`,
    `requireLicenseAcceptance` false.
  - `tags` (`autocomplete terminal shell cli`), `summary`, `description`, `releaseNotes` (the GitHub release URL).
- `legal/LICENSE.txt`: both licence texts.
- `legal/VERIFICATION.txt`: the release download URLs, how to compare `Get-FileHash` with `SHA256SUMS`, and that
  `SHA256SUMS.sig` and the release attestation vouch for the sums.
- `tools/tabsmith-win32-x64.exe` and `tools/tabsmith-win32-arm64.exe`, embedded by the release job (not committed).
- `tools/chocolateyinstall.ps1`:
  - `$arch = if ((Get-CimInstance Win32_Processor | Select-Object -First 1).Architecture -eq 12) { 'arm64' } else { 'x64' }`
  - Rename `tabsmith-win32-$arch.exe` to `tabsmith.exe` and delete the other exe, so Chocolatey's automatic shim
    (it runs after this script) makes a single `tabsmith` command.
  - Print `Run: tabsmith setup`. Never run setup here: Chocolatey runs elevated, so setup would edit the admin's
    profile.
- `tools/chocolateybeforemodify.ps1`:
  - Rename the running `tabsmith.exe` to `tabsmith.exe.<ticks>.old` before an upgrade or uninstall. Every open
    terminal tab runs it, and Windows lets you rename a running exe but not replace or delete it (same trick as
    `Move-TabsmithIntoPlace` in `install.ps1`).
  - Best-effort delete of older `*.old` copies first.

Release job `chocolatey`:

- `needs: [publish, verify-windows]`, `runs-on: windows-latest`, `environment: packages`.
- Download both exes and `SHA256SUMS` from the published release, check the exes against it, and copy them into
  `tools/`.
- `choco pack --version $VERSION`, then smoke-test with `choco install tabsmith -s . -y` and check `tabsmith --version`.
- `choco push tabsmith.$VERSION.nupkg --source https://push.chocolatey.org/ --api-key $env:CHOCO_API_KEY`.

README: add `choco install tabsmith`, `choco upgrade tabsmith` and `choco uninstall tabsmith` to Install, Updating
and Uninstall.

## Still manual (the maintainer)

1. Create a Chocolatey community account and API key, and add it to the `packages` environment:
   `gh secret set CHOCO_API_KEY --env packages --repo PedroLaRosa/tabsmith`.
2. Push v1.0.0 to Chocolatey by hand (`choco pack` + `choco push` from `packaging/chocolatey/` with the v1.0.0 exes)
   and get it through review. Merge the automated `chocolatey` job only after that.

## Verification

- PR 1: `cargo test --locked` green on the three CI OSes.
- PR 1, by hand on Linux with Homebrew:
  1. Install the PR build where `PATH` sees it through a symlink, as Homebrew does, and run `tabsmith setup`.
  2. The dotfile line must name the link, not the Cellar path.
  3. Upgrade the formula; a new shell must still start tabsmith.
- PR 2: run the release workflow on a branch (dry run: test + build only). Then, on the next real release, the
  `homebrew` and `verify-brew` jobs go green and the tap gets one commit.
- PR 3: `choco pack` and `choco install -s .` on `windows-latest` and `windows-11-arm` in the dry run.
  By hand on a Windows VM: with a `tabsmith _ pty` session open, run `choco upgrade tabsmith` (the rename must let it
  through), then `choco uninstall tabsmith`.

## Skipped (add when needed)

- **Shell completions in the formula** (`generate_completions_from_executable`): the `completion` command is hidden.
- **homebrew-core:** when the repo meets notability. It needs a build-from-source formula (`depends_on "rust" => :build`).
- **WinGet / Scoop:** if users ask; Chocolatey was the request.
- **Authenticode signing and notarization:** would clear Chocolatey antivirus hits and allow a cask.

## Sources

- Homebrew 6.0.0: https://brew.sh/2026/06/11/homebrew-6.0.0/
- Homebrew tap trust: https://docs.brew.sh/Tap-Trust
- Homebrew Package Acceptance Policy: https://docs.brew.sh/Package-Acceptance-Policy
- Homebrew 5.0.0: https://brew.sh/2025/11/12/homebrew-5.0.0
- Chocolatey moderation: https://docs.chocolatey.org/en-us/community-repository/moderation/
- choco ARM64 PR: https://github.com/chocolatey/choco/pull/3907
- JumpCloud on the 2027 Chocolatey change: https://jumpcloud.com/support/prepare-now-for-chocolatey-deprecation
