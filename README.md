# tabsmith

IDE-style autocomplete for your terminal: a dropdown menu of subcommands, options and arguments as you type, plus
inline gray "ghost text" suggestions. Written in Rust, it works on **macOS, Linux and Windows**.

**Amazon killed [Fig](https://github.com/withfig/fig), Tabsmith brought it from the ashes to make it shines again.**

![tabsmith demo: dropdown completions and ghost text for git switch, claude and brew install](assets/demo.gif)

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

###  macOS, Linux, WSL

```sh
curl -fsSL https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.sh | sh
```

### Windows PowerShell

```powershell
irm https://github.com/PedroLaRosa/tabsmith/releases/latest/download/install.ps1 | iex
```

### Windows cmd

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

### With Homebrew on macOS or Linux (from the [tap](https://github.com/PedroLaRosa/homebrew-tap); Homebrew updates

this copy, it does not update itself):

```sh
brew install pedrolarosa/tap/tabsmith
tabsmith setup                # adds the shell integration to your dotfiles (asks first)
```

### From source (`~/.cargo/bin/tabsmith`; this install does not update itself)

```sh
cargo install --path .
tabsmith setup                # adds the shell integration to your dotfiles (asks first)
```

## Check if everything is correct

Open a new terminal tab and start typing. `tabsmith doctor` checks everything if suggestions don't show up.

To edit your rc file yourself, answer `n` to `setup`: it prints the exact _pre_ line for the very top and _post_ line
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

A copy anywhere but `~/.local/bin` (Homebrew's, cargo's `~/.cargo/bin`, a `target/` build) is never touched: update
it the way you installed it: `brew upgrade tabsmith` for Homebrew, `choco upgrade tabsmith` (elevated) for
Chocolatey. A binary you put at `~/.local/bin/tabsmith` yourself is the native install, so release builds replace
it (unless `app.disableAutoupdates` is set).

### Uninstall

`tabsmith uninstall` only takes the shell integration out of your dotfiles; the binary stays until you delete it.
Run it first, while `tabsmith` is still on your `PATH`:

```sh
tabsmith uninstall --purge     # --purge also deletes ~/.tabsmith (settings, specs, logs)
```

Then delete every copy you installed (`which -a tabsmith` lists them; a leftover copy keeps `tabsmith -V` working):

```sh
rm ~/.local/bin/tabsmith       # one-liner install (Windows: %USERPROFILE%\.local\bin\tabsmith.exe*)
brew uninstall tabsmith        # Homebrew
choco uninstall tabsmith       # Chocolatey (elevated)
cargo uninstall tabsmith       # cargo install --path . / just install
```

On Windows, close the terminals that run tabsmith before `choco uninstall`: a running exe cannot be deleted, so
Chocolatey would leave a renamed copy in `C:\ProgramData\chocolatey\lib\tabsmith` for you to delete afterwards.

Open a new terminal: `tabsmith -V` should now report `command not found`. An integration installed with
`TABSMITH_HOME` set is only recognised with the same value: `TABSMITH_HOME=<dir> tabsmith uninstall`.

## Commands

| Command                                                                        |                                                                                     |
| ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| `tabsmith settings [KEY] [VALUE]`                                              | Get/set a setting (`-d` deletes, wildcards allowed; `list`, `list --all`, `open`)   |
| `tabsmith theme [dark\|light\|system\|NAME]`                                   | Dropdown theme (`--list`, `--folder` for custom JSON themes)                        |
| `tabsmith inline enable\|disable\|status`                                      | Inline ghost text on/off                                                            |
| `tabsmith inline set-customization [NAME]`                                     | Ghost text source: `history+specs` (default), `history`, `specs`, or an LLM profile |
| `tabsmith inline show-customizations`                                          | List the sources                                                                    |
| `tabsmith integrations install\|uninstall\|reinstall\|status dotfiles [SHELL]` | Manage the shell integration                                                        |
| `tabsmith init <shell> <pre\|post>`                                            | Print the integration code                                                          |
| `tabsmith doctor [--all] [--strict]`                                           | Diagnose and fix common problems                                                    |
| `tabsmith setup` / `tabsmith uninstall [--purge]`                              | Install / remove the integration                                                    |
| `tabsmith update`                                                              | Update a native install to the latest release                                       |
| `tabsmith diagnostic`, `tabsmith debug key-tester\|shell\|logs`                | Troubleshooting                                                                     |

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

## Licence

MIT OR Apache-2.0, at your option: [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE). The bundled
completion specs are © Hercules Labs (Fig), MIT — see `assets/SPECS_LICENSE`; update them with
`scripts/update-specs.sh [version]`.
