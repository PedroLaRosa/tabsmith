# tabsmith

IDE-style autocomplete for your terminal: a dropdown menu of subcommands, options and arguments as you type, plus
inline gray "ghost text" suggestions. It is a rebranded, autocompletion-only re-implementation of the
[Kiro CLI](https://kiro.dev/docs/cli/autocomplete/) autocomplete (formerly Amazon Q / Fig), written in Rust and
working on **macOS, Linux and Windows**.

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
- **Hundreds of CLIs** out of the box (git, docker, npm, kubectl, aws, terraform, cargo, …): the official
  [Fig completion specs](https://github.com/withfig/autocomplete) are embedded and run in QuickJS, including dynamic
  suggestions (git branches, npm scripts, docker containers, files and folders).
- **Inline ghost text** from your history and the specs; `→` / `End` / `Ctrl-E` / `Ctrl-F` accept it,
  `Alt-F` / `Ctrl-→` accept one word. Optionally backed by an LLM (OpenAI-compatible or Anthropic) with your own key.
- Shells: **zsh, bash ≥ 4.4, fish ≥ 3.3, PowerShell 7** (`pwsh`). Works over SSH and inside tmux.

## Install

```sh
cargo install --path .        # or: cargo build --release && cp target/release/tabsmith ~/.local/bin/
tabsmith setup                # adds the shell integration to your dotfiles (asks first)
```

Open a new terminal tab and start typing. `tabsmith doctor` checks everything if suggestions don't show up.

Manual integration instead of `setup`: put the *pre* line at the very top and the *post* line at the very bottom of
your rc file (`~/.zshrc`, `~/.bashrc`, or `$PROFILE` for pwsh):

```sh
eval "$(tabsmith init zsh pre)"     # top
eval "$(tabsmith init zsh post)"    # bottom
```

fish: `tabsmith integrations install dotfiles fish` (writes `conf.d/00_tabsmith_pre.fish` / `99_tabsmith_post.fish`).

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

The LLM is asked only when your history has no match. It receives your recent command history and the current
directory; lines that look like they contain secrets (`password`, `token`, `api_key`, …) are filtered out first.

## How it works

The shell integration's *pre* block replaces the interactive shell with `tabsmith _ pty`, which runs your shell
inside a pseudo-terminal and passes everything through. The *post* block marks prompts and commands with
`OSC 697` escape sequences. tabsmith reads the command line from its own copy of the screen (alacritty_terminal),
parses it against the completion spec, and draws the menu and ghost text over the screen cells, restoring them
exactly afterwards. Design details: [`docs/SPEC.md`](docs/SPEC.md).

## Known limitations

- `cmd.exe`, nushell and macOS's built-in `/bin/bash` 3.2 are not supported.
- On Windows, specs whose generators shell out to `bash` only work when Git Bash is on `PATH`.
- Ghost text is not drawn while the shell shows its own suggestion (zsh-autosuggestions, fish, PSReadLine).

## Licence

MIT OR Apache-2.0. The bundled completion specs are © Hercules Labs (Fig), MIT — see `assets/SPECS_LICENSE`;
update them with `scripts/update-specs.sh [version]`.
