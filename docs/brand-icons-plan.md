# Brand icons for every tool, in every terminal

Handoff plan for an agent session that implements it. Read `AGENTS.md` first: load the `xp-tdd`, `testing-standard`,
`coding-standards` and `hexagonal-architecture` skills before touching code, and commit after each task. `main` is
protected, so each PR below is its own branch and pull request. `AGENTS.md` points at `docs/git-convention.md`, which
is not in the repo: follow the Conventional Commits style of `git log` instead.

The plan comes from a hot-seat interview on 2026-11-27. "Decided" is what the user chose. "Parked" is what the
interview did not reach: each entry has a default so the work is not blocked, and each default is the interviewer's
suggestion, not a user decision.

## Context

[figxit](https://github.com/en3sis/figxit) shows a brand icon on each row (git, docker, pnpm…), and the goal is the
same look in tabsmith, consistent across terminals and operating systems.

Facts the plan rests on (checked 2026-11-27):

- **figxit does not draw in the terminal.** Its popup is a native macOS window (Swift/AppKit). It ships 100
  simple-icons SVGs mapped to 151 commands, and is macOS only. tabsmith draws inside the terminal grid on macOS, Linux
  and Windows, so the mechanism cannot be copied, only the result.
- **tabsmith already draws figxit's style.** The 42 PNGs in `assets/icons` are white glyphs on tinted rounded squares
  (`git.png` red, `command.png` purple, `option.png` green), 32×32, from Amazon Q's autocomplete (Apache-2.0).
- **The gap is coverage, mapping and reach.** 21 of the 42 are brands (`node.png` is a graph-node glyph like `commit.png`, not the Node.js logo). `icon_resolver::slot` picks an icon from the
  spec's `icon` or the row kind, and receives only the row, so `git ch` rows show the generic `command` icon. Images go
  out through the kitty graphics protocol, which `kitty_graphics::detect` turns on for kitty and Ghostty only (and
  for tmux with `allow-passthrough` and one of those two as client). Every other terminal gets ASCII (` >`, ` -`).
- **No image protocol reaches every terminal.** Alacritty and Terminal.app have none. Windows Terminal 1.22+ takes
  sixel only. iTerm2 takes the kitty protocol from the 3.6 line (a bug report against 3.6.6 and a third-party support
  table say so; neither lists Unicode placeholders). WezTerm implements it too, but sources disagree on whether it is
  on by default.
- **The engine already knows the command.** `CompletionResult` carries `tokens` (after alias expansion) and
  `command_index` ("index of the token that names the spec in use: `sudo git ch` → 1"). `ParsedCommandLine` also has
  `first_token` and an unused `spec_name`.
- **The renderer does not.** `OverlayRenderer::frame(model, menu, input)` calls `list_row(it, selected, w)`, which
  calls `icon_resolver::slot(it, self.images)`. Neither `Menu` nor `FrameInput` carries the command.
- **simple-icons** is CC0-1.0, 3,476 icons at release 16.38.1. It no longer carries aws, azure, slack, heroku,
  twitter or okteto; the current PNGs cover all six.
- **Name matching is not safe.** 148 of the 732 bundled root spec names equal a simple-icons name, and some are the
  wrong brand: `less` is Less CSS, `man` the truck maker, `make` Make.com, `nano` the cryptocurrency. Exact matching
  also misses `kubectl`, `cargo`, `gh`, `brew`, `psql` and `node`.
- **This Mac** has Ghostty, kitty, iTerm2 3.7.9 (in `~/Applications`) and VS Code. WezTerm is not installed
  (`brew install --cask wezterm` gives the February 2024 stable). There is no Windows machine or VM.

## Decided

| # | Decision | Why, where it was contested |
|---|---|---|
| Q1 | Same icons and colours everywhere, drawn by the best channel each terminal has. | Identical pixels would need a native window per OS. |
| Q2 | Images use the kitty graphics protocol only. | Sixel for Windows Terminal was chosen, then dropped: nobody here can see it on its target. |
| Q6 | The kitty path also serves iTerm2 3.6+ and WezTerm, each only if its manual check passes. | A terminal that draws or clears wrongly stays on the fallback. |
| Q3/Q9 | Where no images exist, the icon slot is a filled tile in the brand colour with the row-kind character on it. | Chosen from a five-variant preview, over two-letter monograms and over unfilled coloured text. |
| Q4 | A spec `icon` wins. Otherwise subcommand and argument rows show the brand of the command. Options, files, folders, history and shortcuts keep their kind icon. Command-name rows show the brand of their own name. | Brand on every row would hide the difference between subcommands and options. |
| Q7 | The brand set is a curated table in the repo. The 42 current PNGs stay; new ones are drawn once by a script from simple-icons, in the same style. | Automatic name matching shows wrong logos. |
| Q10 | The command is the innermost one, after alias expansion and spec hand-over (P1, answered when PR 1 started). | Built as the root of the innermost spec's name: `tokens[command_index]` names `s3` in `aws s3 ls`, because aws and gcloud subcommands load their own spec files. |

## Behaviour (Given / When / Then)

"With images" means `GraphicsSupport::Direct` or `Tmux`. "Without images" means `GraphicsSupport::Off`. Scenario ids
are for naming tests.

### B1. Which icon a row gets

- **B1.1 Subcommand row takes the command's brand**
  - **Given** the command line `git ch` and a brand for `git`
  - **And** the subcommand row `checkout`, whose spec names no icon
  - **When** the menu draws that row with images
  - **Then** the icon slot shows the git icon
- **B1.2 Argument row takes the command's brand**
  - **Given** the command line `kubectl get ` and a brand for `kubectl`
  - **And** an argument row `pods`, whose spec names no icon
  - **When** the menu draws that row with images
  - **Then** the icon slot shows the kubernetes icon
- **B1.3 A spec icon wins over the brand**
  - **Given** the command line `git checkout ` and a branch row whose spec icon is `⭐`
  - **When** the menu draws that row
  - **Then** the icon slot shows `⭐`, as today
  - **And given** a row under `git` whose spec icon is `fig://icon?type=alert`, **then** it shows the alert icon
- **B1.4 A spec icon tabsmith cannot draw counts as no icon**
  - **Given** a subcommand row under `git` whose spec icon is a narrow emoji (`⚙️`) or a remote URL
  - **When** the menu draws that row with images
  - **Then** the icon slot shows the git icon
- **B1.5 Option rows keep the option icon**
  - **Given** the command line `git commit --` and the option row `--amend`
  - **When** the menu draws that row with images
  - **Then** the icon slot shows the option icon, not the git icon
- **B1.6 File, folder, history, shortcut, special and auto-execute rows keep theirs**
  - **Given** the command line `git add ` and a file row `README.md`
  - **When** the menu draws that row with images
  - **Then** the icon slot shows the file icon
  - The same holds for each of the other kinds named above, one case per kind
- **B1.7 A command without a brand looks as it does today**
  - **Given** the command line `mytool su` and no brand for `mytool`
  - **When** the menu draws the subcommand row `sub` with images
  - **Then** the icon slot shows the `command` icon
- **B1.8 Several commands share one brand**
  - **Given** brands for `kubectl`, `kubectx` and `kubens` that all name kubernetes
  - **When** a subcommand row is drawn under each of them
  - **Then** all three show the kubernetes icon
- **B1.9 Command-name rows show their own brand**
  - **Given** `autocomplete.firstTokenCompletion` is on and the command line `p`
  - **When** the menu draws the rows `python3`, `pnpm` and `pwd` with images
  - **Then** `python3` shows the python icon and `pnpm` the pnpm icon
  - **And** `pwd`, which has no brand, shows the `command` icon
  - **And** an alias row (kind shortcut) keeps the shortcut icon
- **B1.10 The command is the innermost one** (Q10)
  - **Given** the command line `sudo git ch`
  - **When** the menu draws the subcommand row `checkout` with images
  - **Then** the icon slot shows the git icon
  - **And given** `alias k=kubectl` and the command line `k get `, **then** argument rows show the kubernetes icon

### B2. Rows without images

- **B2.1 Brand tile**
  - **Given** a terminal without images, the command line `git ch` and a brand for `git`
  - **When** the menu draws the subcommand row `checkout`
  - **Then** the icon slot is two cells with git's brand colour as background
  - **And** it holds a space followed by `>`, the subcommand character, in bold
- **B2.2 Ink follows the tile colour**
  - **Given** a tile colour `(r, g, b)`
  - **When** `(299·r + 587·g + 114·b) / 1000` is above 160
  - **Then** the character is black; otherwise it is white
  - Example: pnpm orange `F69220` gets black, git red `F05032` gets white
- **B2.3 Kind tile**
  - **Given** a terminal without images and the option row `--amend`
  - **When** the menu draws that row
  - **Then** the icon slot is a tile in the option colour holding ` -`
  - Each kind keeps today's character: subcommand `>`, option `-`, argument and special `*`, file `f`, folder `d`,
    history `h`, shortcut `@`, auto-execute `↪`
- **B2.4 Command without a brand**
  - **Given** a terminal without images and the command line `mytool su`
  - **When** the menu draws the subcommand row `sub`
  - **Then** the icon slot is a tile in the `command` colour holding ` >`
- **B2.5 The selected row keeps the tile**
  - **Given** a terminal without images and a branded row that is selected
  - **When** the menu draws that row
  - **Then** the tile has the brand colour and its own ink, not the selected-row colours
  - **And** the cells before and after the slot have the selected-row background
- **B2.6 Spec-named icons become tiles in that icon's colour**
  - **Given** a terminal without images and a row whose spec icon is `fig://icon?type=aws`
  - **When** the menu draws that row
  - **Then** the tile has the aws colour and the row-kind character
  - **And given** `fig://template?color=2ecc71&badge=🔥`, **then** the tile is `2ecc71` with the row-kind character
- **B2.7 Emoji rows stay text**
  - **Given** a terminal without images and a row whose spec icon is the wide emoji `⭐`
  - **When** the menu draws that row
  - **Then** the icon slot shows `⭐` with no tile, as today
- **B2.8 The slot stays two columns**
  - **Given** any row kind, with or without a brand, in any graphics mode
  - **When** the icon slot is resolved
  - **Then** its text is exactly two columns wide
- **B2.9 Colour depth**
  - **Given** a terminal without 24-bit colour
  - **When** a tile is drawn
  - **Then** its colours go through the same conversion as theme colours (`OverlayRenderer::pen`)
- **B2.10 tmux without a usable client falls back to tiles**
  - **Given** tmux with `allow-passthrough` off, or with a client that is not kitty or Ghostty
  - **When** the menu draws a branded row
  - **Then** the row shows the brand tile

### B3. The brand set

- **B3.1 Every mapped command has an icon and a colour**
  - **Given** the brand table
  - **When** each entry is looked up
  - **Then** its icon name is in `ICON_NAMES`, a PNG of that name is embedded, and the icon has a colour
- **B3.2 Every icon has a colour**
  - **Given** `ICON_NAMES`
  - **When** each name's colour is looked up
  - **Then** one exists (the tiles of B2 need it for kind and spec-named icons too)
- **B3.3 Known wrong matches stay out**
  - **Given** the commands `less`, `man`, `make` and `nano`
  - **When** their brand is looked up
  - **Then** there is none
- **B3.4 The current PNGs are kept**
  - **Given** a brand that already has a PNG in `assets/icons` (git, docker, npm, aws, azure, slack, heroku…)
  - **When** the script that draws new icons runs
  - **Then** that PNG is unchanged
- **B3.5 New icons match the current style**
  - **Given** a brand taken from simple-icons
  - **When** the script draws it
  - **Then** the result is a 32×32 PNG: the rounded square of `template.png` filled with the brand's colour, with the
    glyph centred on it in white, or in black when B2.2 gives black for that colour
- **B3.6 The script is repeatable**
  - **Given** the pinned simple-icons version and the brand table
  - **When** the script runs twice
  - **Then** the PNGs are byte-identical, so a run with nothing new leaves `git status` clean

### B4. Which terminals get images

- **B4.1 iTerm2 3.6 or later**
  - **Given** iTerm2 3.6 or later, outside tmux
  - **When** graphics support is detected
  - **Then** it is `Direct`
- **B4.2 Older iTerm2**
  - **Given** iTerm2 older than 3.6
  - **When** graphics support is detected
  - **Then** it is `Off`, and rows show tiles
- **B4.3 WezTerm**
  - **Given** WezTerm, outside tmux
  - **When** graphics support is detected
  - **Then** it is `Direct`
- **B4.4 iTerm2 or WezTerm behind tmux**
  - **Given** tmux whose client is iTerm2 or WezTerm, with `allow-passthrough` on
  - **When** graphics support is detected
  - **Then** it is `Off`: the tmux path needs kitty Unicode placeholders, which neither is known to support
- **B4.5 kitty and Ghostty are unchanged**
  - **Given** kitty or Ghostty, outside tmux or behind tmux with passthrough
  - **When** graphics support is detected
  - **Then** the result is what it is today

B4.1 and B4.3 ship only if the manual check in PR 4 passes for that terminal. If one fails, its scenario flips to
`Off` and the plan and SPEC record why.

## Work, as four pull requests

Each PR updates SPEC §7.4.2 and `docs/ubiquitous-language.md` for what it changes.

### PR 1 · The brand rule, with the icons already in the repo

Scenarios B1.1 to B1.10. No new assets: the table starts with the 21 brands that have a PNG today (for example
`git`, `docker` and `docker-compose`, `npm` and `npx`, `yarn`, `kubectl`, `gcloud`, `aws`, `az`, `gh`, `glab`,
`gradle`, `firebase`, `heroku`, `netlify`, `vercel`, `adb`).

- The rule belongs next to `icon_resolver.rs` (dropdown domain). The table is a constant, like `ICON_NAMES`.
- The command name has to reach the resolver. It exists as `CompletionResult.tokens[command_index]`. Trace how items
  travel from `CompletionResult` to `Menu` and pick the smallest route that respects the layering: either the engine
  stamps each row, or the menu carries the command name and `slot()` takes it as input.
- First-token lists (B1.9) need to be told apart: the typed word is not a command there, each row is.
- Visible result: in kitty and Ghostty, `git ch` rows show the git icon.

### PR 2 · Tiles where there are no images

Scenarios B2.1 to B2.10. Replaces `ascii_slot` output for `GraphicsSupport::Off`.

- Add a colour per icon name. For the current square PNGs it is the fill of the square. `file`, `folder` and history
  have no square: see P4.
- `IconSlot` needs to carry the tile colour; `list_row` in `overlay_renderer.rs` draws the two cells with
  `self.pen(ink, tile, bold)`, as it already does for badges.
- The preview the user chose from is not in the repo. Its shape: `' '` + kind character, bold, on the tile colour,
  with the row background on the cells either side.

### PR 3 · The curated brand set

Scenarios B3.1 to B3.6.

- **Review list.** These 148 bundled spec names equal a simple-icons name. Review each, drop the wrong brands, then
  add aliases by hand:

  `ansible appwrite argo asciinema astro autocannon babel bat bazel bit black bun capacitor circleci claude clion
  clojure cmake coda composer curl dapr dart deno docker dotenv dotnet electron eleventy elixir elm esbuild eslint
  exercism expo fastlane fastly ffmpeg firebase firefox flutter gatsby git github go goland gradle hasura helm hermes
  hexo htop hugo hyper ionic jest julia jupyter just k6 kitty laravel lerna less locust make man meteor mix mysql
  nano nativescript netlify nextflow nginx ngrok nhost npm nuxt nvm nx okta ollama opencode pandoc php phpstorm pi
  pipx pm2 pnpm pocketbase podman poetry prettier prisma pulumi pycharm pytest python quasar r railway rancher rclone
  remix rich rubocop ruby rubymine ruff sanity sequelize serverless sidekiq speedtest spotify spring stencil
  streamlit stripe supabase swc swift symfony tailscale tailwindcss task terraform tmux trivy turbo typeorm typst uv
  v vapor vault vercel vim vite vtex webpack webstorm wezterm yarn zapier zig`

  - Confirmed wrong: `less`, `man`, `make`, `nano`.
  - Check by opening the icon: `bat`, `bit`, `black`, `coda`, `hermes`, `hyper`, `mix`, `pi`, `r`, `rich`, `spring`,
    `task`, `v`.
- **Aliases to consider** (confirm each simple-icons name exists in the pinned version): `cargo`, `rustc`, `rustup` →
  rust; `python3`, `pip`, `pip3` → python; `brew` → homebrew; `psql`, `pg_dump`, `createdb` → postgresql; `pnpx` →
  pnpm; `bunx` → bun; `mvn` → apachemaven; `java`, `javac` → openjdk; `tofu` → opentofu; `nvim` → neovim; `wrangler`
  → cloudflare; `fly`, `flyctl` → flydotio; `tsc` → typescript; `zsh`, `bash`, `fish`.
- **The script.** One recipe in the `justfile` reads the table, takes each glyph from simple-icons pinned to an exact
  version (16.38.1 was current on 2026-11-27), and writes PNGs into `assets/icons`. The PNGs are committed. No new
  runtime dependency and no build-time step: `cargo build` must not need the rasteriser.
- **Licences.** Extend `assets/icons/README.md`: simple-icons is CC0-1.0, and the logos remain trademarks of their
  owners. Some simple-icons entries carry their own licence or brand guidelines in the project's data file: skip any
  entry that is not plain CC0 unless its terms allow this use.
- **Size.** A current PNG is 3–4 KB. About 150 more adds roughly 0.5 MB to a 12.8 MB release binary.
- Image ids are positions in `ICON_NAMES`. Above 255 icons an id no longer fits a 256-colour index; `icon_id::color`
  already switches to 24-bit there, so check the tmux placeholder test still covers both cases.

### PR 4 · iTerm2 and WezTerm

Scenarios B4.1 to B4.5. Independent of PRs 1 to 3.

- `kitty_graphics::detect` reads the environment directly. Split the decision from the lookup so B4 can be tested
  with given values, as `tmux_graphics(out)` already is.
- Confirm the variables on the real terminals with `env | grep -i term` before relying on them. Expected, not
  verified: iTerm2 sets `TERM_PROGRAM=iTerm.app` and `TERM_PROGRAM_VERSION`; WezTerm sets `TERM_PROGRAM=WezTerm`.
- **Manual check, per terminal, with the user watching** (unit tests only prove the bytes):
  1. Icons appear in the slot on the first menu, aligned with the row.
  2. Row text is not covered: the image sits under the text.
  3. Moving the selection up and down leaves no stale icon.
  4. Scrolling a long list updates the icons.
  5. Esc removes every icon.
  6. A menu drawn above the cursor, at the bottom of the screen, looks the same.
  7. Resizing the window leaves no stray icon.
  8. `vim` then quit, and `clear`, leave no icon behind.
  9. A tinted badge row (shortcut `💡`) draws.
- A terminal that fails any step stays `Off`. Record the terminal version and the failing step.

## Parked

Not decided in the interview. Defaults are suggestions; ask the user before building on one that matters.

| # | Open item | Default used by this plan | What would settle it |
|---|---|---|---|
| P2 | Brand colours close to the menu background: bun, rust and apple are black in simple-icons. | Draw them as they are. The chosen preview showed a black `bun` tile on a dark menu. | Looking at PR 3's icons on a dark and a light theme. |
| P3 | A setting to turn row images off. None exists today. | Not added. | A terminal version that passes the manual check and still misdraws for someone. |
| P4 | Tile colours for file, folder and history, which have no square PNG. | file `64748B`, folder `3B82F6`, history `8E8E93`. | Looking at PR 2 in Alacritty or Terminal.app. |
| P5 | What rasterises the SVGs in PR 3's script. | The implementer's choice, within "no runtime dependency, no build-time step". | Nothing: an implementation detail. |
| P6 | Whether `autocomplete.firstTokenCompletion` should default to on, so that command-name rows (B1.9) are seen. | Stays off. | A separate decision about that feature. |

## Out of scope

- **A native overlay window** like figxit's (Q1): separate GUI code per OS, and it stops working over SSH.
- **Sixel and Windows Terminal images** (Q2, dropped): needs a Windows Terminal 1.22+ to test on. Windows Terminal
  gets the tiles.
- **iTerm2's own inline-image protocol** for iTerm2 older than 3.6 and VS Code.
- **Nerd Font glyphs**: tabsmith cannot detect the font, so it could only be opt-in.
- **Two-letter monograms** on the tiles: rejected in the preview.
- **figxit's other icon features** (verb icons for Makefile targets and scripts, red icons for production rows): not
  discussed.
