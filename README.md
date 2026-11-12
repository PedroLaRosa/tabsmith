# tabsmith

IDE-style autocomplete for your terminal: a dropdown menu of subcommands, options and arguments as you type, plus
inline gray "ghost text" suggestions. Written in Rust, it works on **macOS, Linux and Windows**.

```
~/project % git ch█
              > changelog
              > checkout
              > cherry-pick <commit...>
              Run git-changelog
```

- **Dropdown menu** drawn inside the terminal, right under the cursor (or above it at the bottom of the screen).
  Arrow keys / `Ctrl-N` / `Ctrl-P` to move, `Enter` to insert, `Tab` to insert the common prefix, `Esc` to hide,
  `Ctrl-R` history mode, `Ctrl-K` description pop-out.
- **Hundreds of CLIs** out of the box (git, docker, npm, kubectl, aws, terraform, cargo, …): the Fig-format
  completion specs from [`@tabsmith/autocomplete-specs`](https://www.npmjs.com/package/@tabsmith/autocomplete-specs)
  are embedded and run in QuickJS, including dynamic
  suggestions (git branches, npm scripts, docker containers, files and folders).
- **Inline ghost text** from your history and the specs; `→` / `End` / `Ctrl-E` / `Ctrl-F` accept it,
  `Alt-F` / `Ctrl-→` accept one word. Optionally backed by an LLM (OpenAI-compatible or Anthropic) with your own key.
- Shells: **zsh, bash ≥ 4.4, fish ≥ 3.3, PowerShell 7** (`pwsh`). Works over SSH and inside tmux.

## Install

macOS, Linux, WSL:

```sh
curl -fsSL https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.ps1 | iex
```

Windows cmd:

```bat
curl -fsSL https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.cmd -o install.cmd && install.cmd && del install.cmd
```

The installer downloads the binary for your platform, checks it against the release's `SHA256SUMS`, puts it at
`~/.local/bin/tabsmith` (`%USERPROFILE%\.local\bin\tabsmith.exe`, added to your user `PATH`) and, on a first
install, runs `tabsmith setup`, which adds the shell integration to your dotfiles (it asks first). To install a
specific version, pass it on: `| sh -s 0.2.0`, `& ([scriptblock]::Create((irm https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.ps1))) 0.2.0`
or `install.cmd 0.2.0`.

Platforms: macOS 11+ (Apple Silicon and Intel); any Linux or WSL on x64 or arm64, Alpine included (static musl
binaries); Windows 10+ on x64 or arm64.

To check a downloaded binary by hand: `gh release verify-asset -R PedroLaRosa/tabsmith vX.Y.Z ./tabsmith-<platform>`.

From source (`~/.cargo/bin/tabsmith`; this install does not update itself):

```sh
cargo install --path .
tabsmith setup                # adds the shell integration to your dotfiles (asks first)
```

With [just](https://just.systems) the same is `just install` then `just setup`; `just --list` shows the other recipes
(`update-specs`, `refresh-specs`, `test`, …).

Open a new terminal tab and start typing. `tabsmith doctor` checks everything if suggestions don't show up.

To edit your rc file yourself, answer `n` to `setup`: it prints the exact *pre* line for the very top and *post* line
for the very bottom of `~/.zshrc`, `~/.bashrc` (or `$PROFILE` for pwsh) and writes the script files they source;
`doctor` and `uninstall` recognise those lines. Sourcing the integration directly also works:

```sh
eval "$(tabsmith init zsh pre)"     # top
eval "$(tabsmith init zsh post)"    # bottom
```

but `doctor` then reports the integration as missing, and its automatic fix adds the standard lines as well.

fish: `tabsmith integrations install dotfiles fish` (writes `conf.d/00_tabsmith_pre.fish` / `99_tabsmith_post.fish`).

### Updating

An install made with the one-liners updates itself: when a terminal tab opens, tabsmith looks for a newer release at
most once a day, in the background. It installs one only if its `SHA256SUMS` carries the release signing key's
signature and the binary matches it; the next tab runs the new version. What happened is logged to
`~/.tabsmith/logs/pty.log`.

```sh
tabsmith update                                    # update now
tabsmith settings app.disableAutoupdates true      # no background updates (tabsmith update still works)
```

A copy anywhere but `~/.local/bin` (cargo's `~/.cargo/bin`, a `target/` build) is never touched: update it the way
you installed it. A binary you put at `~/.local/bin/tabsmith` yourself is the native install, so release builds replace
it (unless `app.disableAutoupdates` is set).

### Uninstall

`tabsmith uninstall` (add `--purge` to also delete `~/.tabsmith`), then delete `~/.local/bin/tabsmith` (Windows:
`%USERPROFILE%\.local\bin\tabsmith.exe*`).

## Commands

| Command | |
|---|---|
| `tabsmith settings [KEY] [VALUE]` | Get/set a setting (`-d` deletes, wildcards allowed; `list`, `list --all`, `open`) |
| `tabsmith theme [dark\|light\|system\|NAME]` | Dropdown theme (`--list`, `--folder` for custom JSON themes) |
| `tabsmith inline enable\|disable\|status` | Inline ghost text on/off |
| `tabsmith inline set-customization [NAME]` | Ghost text source: `history+specs` (default), `history`, `specs`, or an LLM profile |
| `tabsmith inline show-customizations` | List the sources |
| `tabsmith integrations install\|uninstall\|reinstall\|status dotfiles [SHELL]` | Manage the shell integration |
| `tabsmith init <shell> <pre\|post>` | Print the integration code |
| `tabsmith doctor [--all] [--strict]` | Diagnose and fix common problems |
| `tabsmith setup` / `tabsmith uninstall [--purge]` | Install / remove the integration |
| `tabsmith update` | Update a native install to the latest release |
| `tabsmith diagnostic`, `tabsmith debug key-tester\|shell\|logs` | Troubleshooting |

## Settings

Stored in `~/.tabsmith/settings.json` (override the directory with `TABSMITH_HOME`). Highlights:

```sh
tabsmith settings autocomplete.disable true          # dropdown off (ghost text stays)
tabsmith settings autocomplete.fuzzySearch true
tabsmith settings autocomplete.onlyShowOnTab true    # open the menu with Tab only
tabsmith settings autocomplete.height 10             # rows; autocomplete.width = columns
tabsmith settings autocomplete.keybindings.tab insertSelected
tabsmith settings autocomplete.keybindings.control+k ignore
tabsmith settings beta.history.mode show             # mix history into the dropdown
```

`tabsmith settings list --all` prints every key with its type and default.

### LLM ghost text (optional)

Define a profile, then select it. API keys are read from environment variables only, never stored.

```sh
tabsmith settings inline.profiles.claude '{"provider":"anthropic","model":"claude-opus-5-5","api_key_env":"ANTHROPIC_API_KEY","effort":"low"}'
tabsmith settings inline.profiles.local '{"provider":"openai","model":"qwen2.5-coder","base_url":"http://localhost:11434/v1"}'
tabsmith inline set-customization claude
```

The LLM is asked only when your history has no match. It receives the OS, shell, current directory and your last
`inline.historyCount` (49) commands; lines that look like they contain secrets (`password`, `token`, `api_key`, …)
are filtered out first, and commands your shell keeps out of its own history (a leading space with
`ignorespace`/`histignorespace`, fish private mode, PSReadLine's sensitive lines) are never recorded by tabsmith.

## How it works

The shell integration's *pre* block replaces the interactive shell with `tabsmith _ pty` (pwsh runs it as a child
and exits with its code), which runs your shell inside a pseudo-terminal and passes everything through. The *post*
block marks prompts and commands with `OSC 697` escape sequences. tabsmith reads the command line from its own copy
of the screen (alacritty_terminal), parses it against the completion spec, and draws the menu and ghost text over the
screen cells, restoring them exactly afterwards. Design details: [`docs/SPEC.md`](docs/SPEC.md).

## Known limitations

- `cmd.exe`, nushell and macOS's built-in `/bin/bash` 3.2 are not supported.
- Windows: the build and unit tests run in CI, but the console/ConPTY paths were only reviewed, not exercised on a real
  Windows console. Specs whose generators shell out to `bash` only work when Git Bash is on `PATH`.
- Ghost text is not drawn while the shell shows its own suggestion (zsh-autosuggestions, fish, PSReadLine). With
  PSReadLine's ListView predictions the dropdown stays hidden while the line is not empty.
- No dropdown on a line without a prompt in front of it: an empty `PS1`, a prompt ending in a newline that a theme
  prints without tabsmith's markers, zsh `Alt-Enter` and fish continuation lines, or a newline-ended `PS1` while zsh
  lists completions below it. Inline TUIs (fzf `Ctrl-T`, fish's pager) are left alone on purpose.
- `Esc` with the dropdown open hides it and is not passed to the shell (vi mode: `tabsmith settings
  autocomplete.keybindings.esc ignore`).
- Your startup files run twice: the outer shell reads them up to the pre block, the wrapped shell reads them all
  (zsh `.zshenv` and `/etc/zprofile`, a login bash's `/etc/profile`, pwsh's AllHosts `profile.ps1`).
- fish: only `--private` reaches the wrapped fish (`-C`, `--features` and `-d` are dropped).
- pwsh: the session's environment and aliases are not reported, so generators see the environment tabsmith started
  with; a custom PSReadLine `AddToHistoryHandler` runs twice per line.
- bash: under `HISTCONTROL=ignoreboth` an immediate repeat of the previous command is not recorded in tabsmith's
  history, and `set +o history` is ignored; on bash 4.4/5.0 your `PROMPT_COMMAND`s see tabsmith's `PIPESTATUS`.
- Emoji and ambiguous-width characters can misplace the overlay when the terminal measures them differently from
  `unicode-width`; text printed before tabsmith started cannot be restored if the dropdown covers it.
- The Fig `history` template returns nothing; the description pop-out (`Ctrl-K`) hides the loading spinner.
- In VS Code's terminal every shell start runs `code --locate-shell-integration-path` (the VS Code CLI's start-up time).
- Logs in `~/.tabsmith/logs` are not rotated; two `integrations install` runs at the same moment can duplicate the
  dotfile blocks.
- Updates trust tabsmith's built-in root certificates, not the system's: behind a TLS-inspecting proxy `tabsmith update`
  fails (the installers use the system's, so rerunning the one-liner works). Release binaries are not notarized or
  Authenticode-signed; downloads through `curl` and PowerShell are not quarantined, but one saved from a browser is.

## Releasing

Maintainers only; `main` is protected, so a release goes through a pull request.

1. Open a PR that bumps `version` in `Cargo.toml` and commits the `Cargo.lock` that follows (any `cargo build`
   updates it; CI and the release build with `--locked`); merge it once CI is green.
2. On the merged `main`: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. The release workflow tests and builds the six binaries, then waits: approve its `publish` job (the `release`
   environment). It writes `VERSION` and `SHA256SUMS`, signs `SHA256SUMS`, checks the signature against the key the
   binaries carry, and publishes the release. The `verify` jobs then run the one-liners and `tabsmith update` from the
   previous release on macOS, Linux x64/arm64, Alpine and Windows x64/arm64.
4. If `verify` fails, `gh release delete vX.Y.Z` puts `latest` back on the previous release; fix forward with the next
   patch version (an immutable release's tag cannot be used again).

One-time setup, before the first release (needs a repo admin):

1. Generate the release signing key outside the repo with OpenSSL 3 (on macOS `/usr/bin/openssl` is LibreSSL: `brew
   install openssl@3` and use `$(brew --prefix openssl@3)/bin/openssl`), and commit the public half:
   ```sh
   openssl genpkey -algorithm ed25519 -out ~/tabsmith-release-key.pem
   openssl pkey -in ~/tabsmith-release-key.pem -pubout -outform DER | tail -c 32 | xxd -p -c 32 > scripts/release-signing-key.pub
   ```
   Never commit the `.pem`. Until this is done, `scripts/release-signing-key.pub` holds a placeholder whose private
   half was thrown away: nothing can be signed for it, so `publish` refuses to release.
2. Create the `release` environment (Settings → Environments) with the maintainer as required reviewer and deployments
   limited to `v*` tags, then `gh secret set RELEASE_SIGNING_KEY --env release < ~/tabsmith-release-key.pem`. Move the
   `.pem` into a password manager and delete the file: a lost key means installed copies cannot update (users rerun
   the installer).
3. Turn on immutable releases (Settings → General → Releases); it only applies to releases created afterwards.

## Licence

MIT OR Apache-2.0. The bundled completion specs are © Hercules Labs (Fig), MIT — see `assets/SPECS_LICENSE`;
update them with `scripts/update-specs.sh [version]`.
