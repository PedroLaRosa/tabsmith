# Phase 8 (dropdown): preparation

Status: **prepared, not started.** Prepared against `7aafcef` (tag `phase8-start`; the end of Phase 7 plus the CI-only
commits of PR #2, so `src/` is the Phase 7 end state; gate green there: 986 passed, 0 failed, 1 ignored
(`all_bundle_modules`), clippy 3 warnings (budget 3, all used), golden identical, testlock against `ef807c1`
MISSING 0).

**How it was researched.** Seven read-only researchers read `7aafcef` and wrote notes under
`target/hexagonal-refactor/p89/` (git-ignored; this file is the merge and wins where they differ):
- `p8-keys.md` (`src/keys.rs`: traps **T1-T24, N1-N8**, gaps **PG1-PG14**);
- `p8-menu.md` (`src/menu.rs`: **T1-T29**, **G0-G12**);
- `p8-render-pure.md` (the pure half of `src/render.rs`: **T1-T16**, **G1-G13**);
- `p8-icons.md` (`src/icons.rs`: **T1-T22, N1-N9**, **4.1-4.14**);
- `p8-theme-cli.md` (the `theme` command, the two parsers, `ThemeRepository`: **T1-T17**, **G1-G12**, about 120 `theme`
  invocations run on the debug binary);
- `p8-renderer.md` (the impure rest of `render.rs`, the kitty half of `icons.rs`: **1-25**, **G1-G13**);
- `p8-consumers-gates.md` (importers, fitness rules, guards, the step order: **K-F, K-R, K-C, K-I, K-T, K-U**).

A planner merged them into this file: traps **P8-T1...**, decisions **R1-R30**, rechecks **K1-K22**, deviations, test
moves, the guard design, the hand-off notes and 19 steps (`8.0a` ... `8.7`) in three runs. The step list is returned to
the orchestrator as structured data; each run's step prompts are written from the tables below, as Phase 7's
`phase-7-ghost-text.args.json` was.

Plan reference: [the plan](../hexagonal-refactor-plan.md).
- Section 5, "Phase 8: dropdown" (five steps; this file cuts them into 19) and "Phase 9: terminal_session" (the
  consumer of this phase's output).
- Sections 1 (Rust conventions), 2 (allowed dependencies: `dropdown -> shared, settings, specs, completion`;
  `terminal_session -> ... dropdown`; the bridge row "theme and settings repositories used by terminal_session"), 4
  (`dropdown`, `terminal_session`, `completion` layouts), 6 (checklist rows `keys.rs`, `menu.rs`, `render.rs`,
  `icons.rs`), 7 (risks), 8 (verification, manual smoke items 1-3, 5, 6) and 9 (D1-D9 and the Phase 1-7 notes).

Glossary: [`docs/ubiquitous-language.md`](../ubiquitous-language.md), sections dropdown and terminal_session. Source
maps: [`keys`](source-map/keys.md), [`menu`](source-map/menu.md), [`render`](source-map/render.md),
[`icons`](source-map/icons.md), [`cli`](source-map/cli.md) (theme part), [`pty`](source-map/pty.md) (keys, menu,
renderer, theme touchpoints). Their numbers are at `af1b304`: much moved since (Phase 2 settings catalog, Phase 6
`Suggestion`/`CompletionResult` accessors, Phase 7 `ghost_text`), so **locate every site by symbol; this file cites
`7aafcef`**.

**Ids in this file.** Traps **P8-T1...** (each names its research origin in brackets: `keys T9`, `menu T10`, `render
T3`, `icons T4`, `theme T3`, `renderer 17`, `cons 44`); decisions **R1-R30**; rechecks **K1-K22**; steps **8.0a-8.7**;
open questions **O1-O8**. Phases 4-7 ids are prefixed (`P7-T25`, `P6-Q15`). Never cite a bare `T`, `R` or `G`.

**Line numbers.** Every line below is `7aafcef`. `git diff phase8-start HEAD -- src` is empty until the first step
commits. After that, every site is found by symbol.

## What Phases 4-7 leave for Phase 8

Assumed, and verified at `7aafcef` unless a K row says otherwise:
- `src/keys.rs`, `src/menu.rs`, `src/render.rs`, `src/icons.rs` (legacy, 3777 lines) are unchanged but for the Phase 2/6
  import paths. `src/dropdown/` does not exist. `src/main.rs:13-17` declares `mod icons; mod keys; mod menu; mod pty;
  mod render;`.
- Rows are private behind accessors: `Menu` reads `Suggestion`/`CompletionResult` only through `names()`, `kind()`,
  `query()`, `first_name()`, `take_suggestions()`, ... (Phase 6). The builders the tests need are `#[cfg(test)]`:
  `Suggestion::{with_generator, with_insertion, with_original_kind, with_icon}`,
  `CompletionResult::{with_request_id, with_loading, with_phase, with_triggered_generators}`,
  `GeneratorKey::from_raw`.
- The settings catalog owns every key and default (Phase 2): the menu reads `AUTOCOMPLETE_HEIGHT/_ONLY_SHOW_ON_TAB/
  _DISABLE/_ALWAYS_SHOW_DESCRIPTION/_SCROLL_WRAP_AROUND/_NAVIGATE_TO_HISTORY`, the renderer `AUTOCOMPLETE_WIDTH` and
  `AUTOCOMPLETE_THEME`, through `Settings::bool/f64/string`, which fall back to the catalog default on a missing or
  wrong-typed value; per-key default tests exist.
- `Settings::with_prefix` iterates a `BTreeMap`, so user key bindings come out sorted, pinned by
  `should_give_the_keys_under_a_prefix_without_it_in_sorted_order` (`settings.rs:317`).
- `DataDirectory::themes_dir()` (`data_directory.rs:52`), `SharedDependencies { data_directory, clock }` and the
  injected-repository module pattern (`InlineCommandModule`, `bootstrap/cli.rs:159-161`) exist.
- `completion/domain/test_fixtures.rs` exists (`#[cfg(test)] pub(crate) mod test_fixtures;`,
  `completion/domain/mod.rs:5-6`) with `pr_for, served_request, opts, sub, opt, arg, names, ctx`; `item`, `comp` and
  `settings` join it (Phase 6 hand-off; `settings` only with `#[cfg(test)]` on each item, CON-T28).
- `ghost_text` names nothing of `dropdown`; `pty.rs` keeps `ghost_key`, `is_ctrl`, `note_forwarded` and the test
  `ghost_keys_and_words_keys` (Phase 7 hand-off). `cli::settings_repository()` still exists for `theme`.
- `terminal_session` (unregistered) has `application/{ports/completion_engine.rs, report_session_context_use_case.rs}`,
  `domain/{entities/session_context.rs, repositories/, value_objects/session_id.rs, terminal_session_error.rs}`,
  `infrastructure/{adapters/file_session_context_repository.rs, cli/pre_cmd_command.rs, terminal_session_module.rs}`;
  `adapters/mod.rs` has one line. `render.rs` borrows `&types::Model`; `types::Model` survives to Phase 9.
- `REGISTERED_MODULES` = completion, ghost_text, history, settings, shared, shell_integration, specs
  (`architecture_tests.rs:9-10`); `LEGACY_MODULES` lists `cli doctor engine icons inline integrations js keys menu
  parser pty render shell_parser spec suggest tracker types` and stays as it is.

### Re-check at the start of run 1 (the items marked DEFERRED are re-read against the tree the run really starts from)

| K | Claim | Step | Fact at `7aafcef` |
|---|---|---|---|
| K1 | The legacy importers are exactly `pty.rs` (`:5` keys, `:7` menu, `:8` render, `:452` `crate::icons::detect()`, `:1758` `menu::tests::{comp,item,settings}`), `render.rs` (`:3` icons, `:4` menu, `:893` fixtures, `:1065` `crate::keys::Action`), `icons.rs:379` (`menu::tests::item`), `menu.rs:3` (`keys::Action`) | 8.1a-8.4b | `grep -rnE 'crate::(keys\|menu\|render\|icons)\b' src` outside the four files = those 11 lines. DEFERRED: rerun |
| K2 | 38 tests live in the four files + cli: `keys::tests` 7, `menu::tests` 12, `render::tests` 11, `icons::tests` 6, `cli::tests::{theme_list_order, theme_file_parsing}` | all | `cargo test --locked -- --list` shows exactly these |
| K3 | `Settings::with_prefix` is key-sorted and `Keymap::set` replaces on a duplicate: the lexicographically last binding wins (`control+x` < `ctrl+x`, `alt+K` < `alt+k`) | 8.0b, 8.1a | `keys.rs:412-428, 431-439`; the settings test above pins the order; no keys test pins the replacement |
| K4 | `Menu` reads the clock twice only: `Instant::now()` `menu.rs:115`, `t.elapsed()` `:328`; no `SystemTime`, env, thread, `cfg`, logger | 8.2a | grep clean; `Clock::now_instant` exists (`shared/application/ports/clock.rs:18`) but Phase 8 callers pass `Instant::now()` |
| K5 | `Renderer::menu_patches` calls `menu.loading()` (`render.rs:454-456`); `FrameInput` (`:180-190`) has no time field; literals at `pty.rs:994` and the test helper `input()` `render.rs:980` | 8.2a | confirmed |
| K6 | No consumer of `Theme` fields outside `render.rs` (`pty.rs` uses `Theme::dark()`, `Theme::from_settings` x3, the type); `description_border` has no reader outside `from_json`/`dark`/`light` and `PartialEq` | 8.3b, 8.5d | greps: `pty.rs:451, 1021, 1284, 1793`; no `.description_border` read in `render.rs` drawing code |
| K7 | `Theme::from_settings` reads the theme file on every call (`render.rs:85-90`: `data_directory_from_env().themes_dir().ok()` + `read_to_string` + `from_str::<Value>`), silently | 8.5d | confirmed; call sites `pty.rs:451` (start), `:1021` (late OSC 11), `:1284` (settings reload) |
| K8 | The two theme parsers differ: strict `cli::parse_theme` (`cli.rs:86-94`, serde `ThemeFile`) and lenient `Theme::from_json` (`render.rs:96-128`, `Value` walk) | 8.0a, 8.5b, 8.5d | confirmed on ~120 CLI invocations (researcher `p8-theme-cli`), table in R16 |
| K9 | `icons.rs` has no `cfg(windows|unix|target_os)`; `PNGS` has 42 names and `assets/icons` 42 `.png` files, ids = index + 1 | 8.4a, 8.4b | the `pngs!` list (`icons.rs:19-60`) has 42 names and `assets/icons` 42 `.png` files (+ `template.png`/`template.rgba`, 4096 bytes) |
| K10 | `include_bytes!` depth from `src/terminal_session/infrastructure/adapters/` is `../../../../assets/icons/` (4 levels), as `embedded_spec_bundle.rs:21` | 8.4b | confirmed by that precedent; fitness rule 8 flags `include!`, `#[path`, `#[macro_export`, `#[macro_use`, not `include_bytes!` or a plain local `macro_rules!` |
| K11 | MSRV 1.85 (`Cargo.toml: rust-version`), `unicode-width = "0.2"`, no new crate | all | confirmed; `Instant::duration_since` saturates since 1.60 |
| K12 | The fitness scanner treats a fixture file as production code (`architecture_tests.rs:156,165`: only items under `#[cfg(test)]` / `#[cfg(all(test, ..))]` and `*/tests.rs` are exempt); a `#[cfg(test)] use ..;` line followed by an attributed fn is read as two test-only items | 8.2a | read from `TestOnlyItem` (`:196-235`); verify by the local registration run after 8.2a |
| K13 | `rust_files()` panics when a registered module's folder is missing (`architecture_tests.rs:126-127`), so the local registration of `dropdown` can start only once `src/dropdown/` exists (8.1a) | 8.1a-8.6 | confirmed |
| K14 | `pty.rs` calls into the area at: `Keymap::from_settings` `:453, :1282, :1794`; `decoder.decode` `:1044`; decoder reset `:1074`; `menu.on_completion` `:891`; `menu.loading` `:1242`; `renderer.frame` `:1001`; `set_theme` `:1021, :1284`; `take_clear` `:700, :765, :1291`; `drop_clear` `:1199`; `full_repaint` `:1209`; `FrameInput` literal `:994-1000` | 8.1a-8.6 | confirmed by grep |
| K15 | `cli.rs` theme section: `ThemeArgs` `:27-37`, `settings_repository` `:57-60`, `BUILTIN_THEMES` `:65`, `ThemeAuthor` `:67-72`, `ThemeFile` `:74-84`, `parse_theme` `:86-94`, `theme` `:96-135`, `theme_list` `:137-159`, `theme_banner` `:161-175`, tests `theme_list_order` `:465`, `theme_file_parsing` `:477`; `bootstrap/cli.rs:53` `Theme(cli::ThemeArgs)`, `:133` `cli::theme(a)` | 8.5b, 8.5c | confirmed |
| K16 | The golden capture covers `theme --help`, `theme` (unset), `--list` (no folder), `--folder`, `theme light`, `theme nope`, `theme` over `{oops`, no-home forms; nothing of custom files, banners, `invalid theme file` texts, built-in/file collisions, non-string values, save failures | 8.0a | `golden.sh:29, 54, 70-75, 109, 132-135`; hence `theme-diff.sh` |
| K17 | `debug_shell_survives_closed_terminal` (`cli.rs:593`) is the only automated start-up run of `pty::run` (renderer, `icons::detect`, `Theme::from_settings`, `Keymap`, `Menu`); it names its own path (D9) but stays in `cli.rs` through Phase 8 | all | confirmed; nothing re-execs a Phase 8 module path |
| K18 | Hand greps empty at HEAD: `grep -rnE 'completion::application::(command_line_parser\|generator_runner\|test_fixtures)' src --exclude-dir=completion`; `grep -rn 'pub(crate) use' src` outside `architecture_tests.rs` strings | all | confirmed |
| K19 | Clippy budget 3, all used: every step prunes the imports it leaves unused and adds no `new` with more than 7 parameters, no `Default` unit struct, no `match` with duplicated bodies | all | `target/hexagonal-refactor/gate-last/clippy.txt` = 3 warnings |
| K20 | Working tree at launch: `docs/hexagonal-refactor/phase-workflow.js` has one uncommitted change (it reads `args.branch`). Commit it with the hand-over docs; step 8.0a stops on a dirty `git status` | launch | `git status --short` at `7aafcef` |
| K21 | No Phase 8 new test name exists yet (R25): `grep -rn "fn <name>(" src` empty for every name of "Test moves" | 8.0a | DEFERRED: run for all names at launch |
| K22 | The baseline binary `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` exists (`make-baseline.sh`) and `target/debug/tabsmith` builds | 8.0a | present |

## Where the code is today (`7aafcef`)

| What | Where (lines) | Used by | Goes to (step) |
|---|---|---|---|
| `KeyCode`, `Mods`, `Key`, `KeyEvent`, free `decode` | `keys.rs:6-57` | `pty.rs` (the whole `on_keys`, `ghost_key` `:186`, `is_ctrl` `:244`, `note_forwarded` `:938`) | `key_code.rs`, `modifiers.rs`, `key.rs` (8.1a); `key_event.rs` in terminal_session (8.1b); private fields (8.1c) |
| `Decoder`, `scan_paste`, `decode_one/esc/csi`, `PASTE_START/END` | `keys.rs:59-266` | `pty.rs:345, 533, 1044, 1074, 1795` | `StdinKeyDecoder` in `terminal_session/infrastructure/adapters/stdin_key_decoder.rs` (8.1b) |
| `Action`, `Action::from_id`, `Binding` | `keys.rs:268-327` | `pty.rs`, `menu.rs` (35 uses) | `key_action.rs`, `key_binding.rs` (8.1a) |
| `parse_keystroke`, `normalize`, `DEFAULTS`, `Keymap` | `keys.rs:329-449` | `pty.rs:343, 453, 1057, 1282, 1794` | `keystroke_parser.rs`, `keymap.rs` (8.1a) |
| 7 keys tests + helpers | `keys.rs:451-717` | | "Test moves" (8.1a, 8.1b) |
| `Vis`, `Effect` | `menu.rs:19-45` | `pty.rs` effect loop `:1117-1135` | `menu_visibility.rs`, `menu_effect.rs` (8.2b) |
| `Menu` (`new`, `on_completion`, `set_items`, `error_reset`, `shown`, `arg_hint`, `intercepting`, `has_items`, `handle`, `show_on_tab`, accessors, `popout`, `loading`, `navigate`, `select`, `scroll_into_view`, `insert_full`, `common_prefix`), `max_rows`, `typing`, `same_item`, `matching_type` | `menu.rs:47-468` | `pty.rs` (15 sites, K14), `render.rs:4, 343, 403-455, 600` | `entities/menu.rs` (8.2b), `common_prefix.rs` (8.2c) |
| 12 menu tests, helpers `item comp settings subs typed prefix_fx with` | `menu.rs:472-1025` (555 lines) | `item/comp/settings`: `pty.rs:1758`, `render.rs:893`, `icons.rs:379` | `entities/menu/tests.rs` (8.2b); fixtures to `completion/domain/test_fixtures.rs` (8.2a) |
| `Rgb`, `Theme`, `SHADES`, `ACCENTS`, `dark`, `light`, `from_settings`, `from_json` | `render.rs:22-130` | `pty.rs:8, 451, 1021, 1284, 1793` | `rgb.rs`, `theme.rs` (8.3a, 8.3b); resolver, palette, mapper, repository (8.5d) |
| `parse_color`, `xterm256` | `render.rs:132-177` | `Renderer::rgb` `:473`, `from_json` | `color_parser.rs` (8.3a) |
| `clean`, `cw`, `width`, `fit`, `wrap` | `render.rs:825-888` | `Run::put`, `list_row`, `desc_row`, `popout_rows`, `ghost_patch`, `describe`; `icons::width` is the same code | `text_fitting.rs` (8.3a) |
| `Side`, `Layout`, `layout` | `render.rs:618-676` | `menu_patches` only | `menu_side.rs`, `menu_layout.rs`, `menu_layout_calculator.rs` (8.3c) |
| `describe` | `render.rs:599-616` | `desc_row`, `popout_rows` | `menu_describer.rs` (8.3c) |
| `FrameInput`, `Renderer` (+`SPINNER`, `POP_W`, `HEAD`, `Composed`, `Patch`, `Run`), `ghost_patch`, `tail`, `cup`, `restore_cells`, `sgr`, `color`, `blank` | `render.rs:179-597, 678-822` | `pty.rs:8, 342, 451, 700, 765, 994-1001, 1021, 1199, 1209, 1284, 1291, 1793` | `terminal_session/infrastructure/adapters/overlay_renderer.rs` (8.6) |
| 11 render tests + helpers | `render.rs:890-1437` (547 lines) | | "Test moves" |
| `pngs!`, `PNGS`, `TEMPLATE_RGBA`, `tinted` | `icons.rs:11-64, 274-283` | `slot`, `badge_slot` | `icon_assets.rs` (8.4b) |
| `Graphics`, `Slot`, `Image`, `slot`, `placeholders`, `color`, `wide_emoji`, `spec_image`, `type_image`, `ascii_slot`, `text`, `png_slot`, `badge_slot`, `pad2`, `width`, `param`, `hex` | `icons.rs:66-312` | `render.rs:3, 201, 253, 259, 309, 353, 377-382, 484-531`; `pty.rs:452` | `graphics_support.rs`, `icon_id.rs`, `icon_slot.rs`, `icon_resolver.rs` (8.4a, 8.4b) |
| `CHUNK`, `detect`, `tmux_graphics`, `transmit`, `place`, `CLEAR_PLACEMENTS`, `base64` | `icons.rs:66-67, 101-136, 314-374` | `render.rs`, `pty.rs:452` | `kitty_graphics.rs` (8.4b) |
| 6 icons tests | `icons.rs:376-598` | | "Test moves" (8.4b) |
| `theme` command: `ThemeArgs`, `BUILTIN_THEMES`, `ThemeAuthor`, `ThemeFile`, `parse_theme`, `theme`, `theme_list`, `theme_banner`, `settings_repository` + 2 tests | `cli.rs:27-175, 462-493` | `bootstrap/cli.rs:53, 133` | `dropdown/{domain,application,infrastructure}` (8.5a-8.5c) |

Sizes: `keys.rs` 717 lines, `menu.rs` 1025, `render.rs` 1437, `icons.rs` 598, `cli.rs` 679, `pty.rs` 2524. What stays in legacy
for Phase 9 and 10: `pty.rs` (all of it), `tracker.rs`, `types.rs`, the rest of `cli.rs`. Nothing of the four files outlives
Phase 8: `keys.rs` dies in 8.1b, `menu.rs` in 8.2b, `icons.rs` in 8.4b, `render.rs` in 8.6 (the plan's step 5 "delete
`keys.rs`, `menu.rs`, `render.rs` and `icons.rs`" happens file by file, with each file's last item, as in Phases 5-7).

## Behaviour that must not change

Grouped by the step that moves the code. "Pinned" = a test at `7aafcef` fails if it breaks; "8.0b" = pinned by a
guard that step adds before any code moves. Every line is `7aafcef`.

### Keys (8.1a, 8.1b, 8.1c)
- **P8-T1 Override precedence** [keys T1]. `Keymap::from_settings` (`keys.rs:412-428`) walks `settings.with_prefix(
  "autocomplete.keybindings.")` (sorted, user keys only, prefix stripped) and `set` (`:431-439`) replaces on a duplicate
  normalized key, so the lexicographically last wins (`control+x` < `ctrl+x`, `alt+K` < `alt+k`, byte order). Keep the loop
  order and the replace. Unpinned today: 8.0b pins it. The prefix literal stays a literal in `keymap.rs` (the catalog has
  only the template row `autocomplete.keybindings.<keystroke>`).
- **P8-T2 Value filtering** [keys T2, T3, T4]. A non-string value is skipped (`:420`, `SettingValue::as_str` is `Text` only;
  the test asserts `"f5": 3` leaves `\x1b[15~` unbound). `"ignore"` is compared before `from_id` strips `autocomplete.`
  (`:421-423`), so `autocomplete.ignore` is `Unknown`. `selectSuggestion` goes through `parse::<u8>()` (`:313`): `+5` and
  `05` are accepted, a value outside `1..=10` is `None`; `from_id("selectSuggestion")` is `None`.
- **P8-T3 `parse_keystroke`** [keys T5-T8, N4]. An unknown or miscased modifier is silently ignored (`_ => {}` `:342`): `Ctrl+k`
  parses to the plain key `k`; names are case-sensitive (`Enter`, `ESC`, `space` give `None` and the default stays). The
  split loop reads `Some(("", ""))` as a literal `+` (`:335`): `+`, `++`, `ctrl++` are `+`; `a+`, `ctrl+`, `""` are `None`;
  `+x` is plain `x`. `f[1..].parse().ok()?` (`:363-364`): `f0` and `f01` parse, `f256` makes the whole keystroke `None`, `f`
  is `Char('f')`; keep the `starts_with('f')` guard (it makes byte 1 a boundary). `shift+<ascii lowercase>` becomes the
  uppercase char with shift cleared (`:372-375`); `shift+A`, `shift+1` keep shift.
- **P8-T4 `normalize` stays in the keymap path** [keys T9]. Applied in `set` (`:432`) and `lookup` (`:443`): lowercases only
  with ctrl or alt and only ASCII letters. It must not move into the decoder or `Key::new`: `ghost_key` matches `Char('f')
  if alt` (`pty.rs:202`), so `\x1bF` (decodes `Char('F')`+alt) must not accept a word. `ctrl+shift+k` parses to `K`+ctrl and
  normalizes to `k`+ctrl.
- **P8-T5 `decode_one`** [keys T10, T11]. Arm order `\r \n \t 0x7f 0x08` before `0x01..=0x1a` (`:126-131`): ctrl+h is
  Backspace, ctrl+i Tab, ctrl+j and ctrl+m Enter; `0x1b` is handled before `0x00..=0x1f`. UTF-8 (`:132-147`): an invalid
  lead or a truncated sequence is `Unknown` of length 1, no carry across reads.
- **P8-T6 ESC handling** [keys T12, T13]. SS3 always consumes 3 bytes (`:165-180`), `ESC O` and `ESC [` at the end of a chunk
  fall to the `Some(_)` arm (alt+`O`, alt+`[`), `R` is `F(3)` in SS3 only. The ESC-ESC alt prefix needs `b.len() > 3`, `b[2]`
  in `[`/`O` and `!b[1..].starts_with(PASTE_START)`, in that order (`:155-163`).
- **P8-T7 CSI** [keys T14-T17]. Final `R` is `CursorReport` before the private-marker check (`:202-210`); the terminator scan
  starts at index 2 over `0x40..=0x7e` (`:192-195`), no terminator is `Unknown` over the whole slice; params are non-UTF-8 as
  `""`, unparsable as `0`, never empty (`p[0]` safe); modifier bits 1 shift, 2 alt, 4 ctrl, 8 meta, higher bits dropped
  (`:218-223`); `\x1b[Z` forces shift (`:232-243`); F-key maths `(n-10)`, `(n-11)`, `(n-12)` (`:251-253`); `\x1b[u` is
  `Char('\0')`.
- **P8-T8 Paste** [keys T18]. `scan_paste` (`:99-114`) restarts with `usize::from(b == 0x1b)` on a mismatch and matches the end
  marker across chunks through `paste: Option<usize>`; the whole remaining paste region of a chunk is ONE `Paste` event
  including the start marker (`:84`); every event has length >= 1 and the ranges tile the chunk (`i += len`, `:93`).
- **P8-T9 The pty couples to the decoder** [keys T19, N2, N3; cons 6]. `on_keys` appends to `held` and returns before decoding
  when `held` is non-empty (`pty.rs:1035-1037`); a key that would act while `echo_pending` holds `chunk[ev.bytes.start..]`
  (`:1072`), resets the decoder by assigning `Decoder::default()` (`:1074`) and breaks; `release_held` replays the raw
  bytes and is called from `on_output` (`:770`), `on_completion` (`:893`) and `on_timers` (`:1225`). So the decoder stays a
  field of the session, decoded once per `on_keys` call, and `#[cfg(test)] tests::maybe_panic();` stays the first statement
  of the per-event loop (`:1045-1046`).
- **P8-T10 Lookup and routing** [keys T20, N1]. `Keymap::lookup` is consulted only when `prompt` holds (`:1056-1060`);
  `KeyBinding::Ignore` has no arm of its own and falls into the final `_ =>` (`:1084`); the guard order is `Action if
  intercepting` (`:1078`), `Unknown if intercepting` (`:1079`), `_ if tab_show` (`:1080`), `Show/Toggle if global`
  (`:1081-1085`). `typed` counts only `Char` with default modifiers (`:1043, 1051, 1099-1101`), so `Paste`, `Unknown` and
  `CursorReport` stay variants of `KeyCode` inside `Key`.
- **P8-T11 Shapes** [keys T21-T23, N5, N6]. `Modifiers == Modifiers::default()` means "no modifiers" (`pty.rs:188, 944, 1062,
  1100`; meta is part of `Eq`); `Key` stays `Copy` (`:1047`); `Keymap` has no derives and is a pure function of `Settings`
  (rebuilt at `:453`, `:1282`, `:1794`); the `Hash` derives stay verbatim; `DEFAULTS` keeps the `control+p`/`control+n`
  spellings and the 10 rows in order. `KeyCode::F(0)` from `f0` never matches a decoded key; not a bug to fix. `r.Event.KeyEvent`
  at `pty.rs:1520` is the Windows `KEY_EVENT_RECORD` field, unrelated to our `KeyEvent`.
- **P8-T12 No fallible path** [keys]. Nothing in `keys.rs` returns an error, logs, prints, reads env or has `cfg`: no
  `DropdownError` is needed for it, and nothing for D5.

### Menu (8.2a, 8.2b, 8.2c)
- **P8-T13 Visibility order** [menu T1, T2, T3]. In the Parse phase (`menu.rs:119-162`): the base transition (`:137-151`), then the
  `onlyShowOnTab` override (`:152-157`: `self.vis = orig`, then `HiddenUntilShown` only if there is a new arg; `HiddenByInsertion`
  and `HiddenUntilKeypress` stay), then `backspaced || (large && !self.just_inserted)` gives `HiddenUntilKeypress` (`:158-160`),
  then `just_inserted = false` (`:161`). Generators-phase replies skip the whole block (`:119`): no vis change, `just_inserted`
  not cleared, yet `self.last = Some(c)` (`:163`) and `set_items` (`:164`) run. The first reply after a reset is "large"
  when its command text has 2+ chars (`:122`, `!typing(.., "", 1)`).
- **P8-T14 Edit counting** [menu T4, T5]. `typing` counts `chars()` (`:72-73`); `steps = request_id.saturating_sub(..).max(1) as
  usize` (`:129`); `backspaced` is `n > 0 && n < l.tokens().len() && l.tokens()[n-1] == c.tokens()[n-1]` (`:132`, safe only
  because of the two guards).
- **P8-T15 Items and selection** [menu T6-T9]. `set_items` runs after the vis update (`:164, 169-188`): a sticky selection
  survives only if `vis == Visible` post-transition; items are replaced even while hidden. `same_item` compares names, kind and
  description only (`:78-80`; never derive `PartialEq` on `Suggestion`, its `insert.text` changes per keystroke). `select` sets
  `sticky = i != self.selected` before `selected = i` (`:348-352`). `navigate` (`:331-346`): wrap wins (`rem_euclid`, never hides),
  `navigateToHistory` toggles `history_mode` and returns `[Rerequest]`, otherwise Up at 0 hides, Down never hides, final
  `clamp(0, len-1)`.
- **P8-T16 Loading clock** [menu T10, T26, T27]. `loading_since` keeps the first start (`:114-118`, `.or_else`), any non-loading
  reply of either phase sets `None`, an error reply returns at `:110-112`; `loading` is strictly `> 200 ms` (`:326-329`). With
  `now` passed in use `now.duration_since(t)` (saturating: a `now` before `t` is "not loading", like `elapsed()`); keep a monotonic
  `Instant`, never `Clock::now_ms`. `pty.rs:1242` reads `menu.loading()` several statements after `on_timers` took its `now`
  (`:1213`): the call sites pass `Instant::now()` where the old read happened (R7). `error_reset_and_loading` and `render`'s
  `round_trip_arg_hint_spinner_and_wide_ghost` depend on the real clock today (flaky only if the machine stalls over 200 ms).
- **P8-T17 `handle`** [menu T11-T13]. `if self.items.is_empty() { return Vec::new(); }` guards every action (`:225-227`).
  `SelectSuggestion(n)` is `scroll_top + usize::from(n.max(1)) - 1`, relative to the scrolled window, 0 counts as 1, out of range
  ignored, `vis` untouched (`:274-280`; `.max(1)` stays even if `KeyAction` narrows the payload). `ToggleAutocomplete` moves only
  `Visible` to `HiddenUntilShown`, every other state to `Visible` (`:252-259`). The description actions are no-ops when
  `alwaysShowDescription` (`:264-272`); `popout()` ORs the setting (`:322`).
- **P8-T18 Reset and tab** [menu T14, T15, T17]. `error_reset` keeps only `popout` (`:191-197`, a new field silently joins the
  reset). `show_on_tab`: exactly one item gives `insert_full(0, false)` whatever the vis, otherwise `Visible` even with no items;
  it ignores `settings` (`let _ = settings;`, `:285-292`). `insert_full` (`:362-385`): `with_execute()` only when `execute && kind
  != AutoExecute`; `record` is `None` when the first name CONTAINS `↪` or `last` is `None`; `last_inserted_gen = item.generator()`
  overwrites with `None` too; `&self.items[idx]` panics on a bad index; keep the single method body (it holds `item` while it
  writes disjoint fields, `:364-379`). `MenuEffect::Insert.full` is read only by tests; keep the field.
- **P8-T19 `common_prefix`** [menu T18-T22]. Order (`:388-468`): `items.len() == 1` inserts in full BEFORE the Special/AutoExecute
  guard (`:390-392`); an AutoExecute row without `original_kind` fails through `sel.original_kind()?` (`:394`); `(!auto).then(..)`
  at `:409` and `:443` so auto rows never full-insert; every `None` returns before mutating (the `OrNavigateDown` /
  `OrInsertSelected` fallbacks see an untouched menu); `just_inserted = true` of the partial path (`:457`) is the only mutation.
  Case: filter on lowercased `first_name` with `starts_with(lowercased query)`, `lcp` counted in chars over the sorted
  lowercased names, `shared` taken from the ORIGINAL-case selected name (`:402-416`), `shared == sel.query()` and
  `strip_prefix(sel.query())` are case-sensitive (`:417, 453`). Every `char::is_whitespace()` becomes `"\\ "` (`:420-429`).
  The full reconstruction is `query + insert.text` only when `insert.delete() == 0`, `full.starts_with(&text)` else bell, full
  insertion if `text == full || format!("{text} ") == full` (`:431-442`). `matching_type` is asymmetric (`:83-89`): a candidate
  `../` matches only a selected `../`, a selected `../` matches every file or folder. The split keeps "decide first, mutate last".
- **P8-T20 Settings reads** [menu 0]. `max_rows` is `f64(height).max(1.0) as usize` (`:66`; 2.9 gives 2, <= 0 gives 1, wrong type
  gives the catalog 7) and `scroll_into_view` relies on `rows >= 1`; `shown` is true for an arg hint with a non-empty name OR
  description (`:200-212`); `alwaysShowDescription` is read before the match in `handle` (`:228`).
- **P8-T21 Fixtures are values** [menu T25]. `comp` has `req 1`, `cursor = buffer.len()` in BYTES, tokens from `split(' ')` (a
  trailing space gives an empty token), `command_index 0`, `search_term` = last token; `item` strips the query only when
  `name.starts_with(query) && !query.is_empty()` with BYTE slicing, otherwise `delete = query.chars().count()` and the full
  name; description `about {name}`, priority 50.0, `highlight 0..query chars`, `prefix_match true`. Render and pty tests mutate
  them. The bodies move byte-identical.

### Colour, text, layout, describe (8.3a, 8.3b, 8.3c)
- **P8-T22 `parse_color`** [render T1]. `u8::from_str_radix` (`render.rs:135`) and `str::parse::<u8>` (`:150`) accept a leading
  `+` (`#+f0000`, `rgb(+1,2,3)`): a hand-written parser changes behaviour. Input is `trim()`med; `h.len()` is bytes; `#abcd` is
  `None`; 3 digits are `*17`; `#rrggbbaa` premultiplies `(c*a+127)/255` in `u32`; only lowercase `rgb(`, exactly 3 parts.
- **P8-T23 `xterm256`** [render T2]. `min_by_key` makes the lower cube level win a tie (`:158-162`); grey is `(avg - 8 + 5) / 10`
  clamped to 0..=23 (`:164`) and beats the cube only on strict `<` (`:172`). Measured: (0,0,0) 16, (255,255,255) 231,
  (128,128,128) 244, (8,8,8) and (7,7,7) 232, (238,238,238) 255, (247,247,247) 231, (30,90,199) 26, (48,48,48) 236.
- **P8-T24 `system` luma** [render T3]. `(0.299*r + 0.587*g + 0.114*b) / 255.0 >= 0.5` in f64, evaluated left to right (`:77-80`);
  the integer form disagrees on 44 of the 16.7M colours; keep the expression character for character. No reply (`None`) is dark.
- **P8-T25 `Theme::from_settings`** [render T4, T11, T12; theme T4]. (a) `Settings::string` answers the catalog default `dark` for an
  unset or wrong-typed `autocomplete.theme`, so `""` only comes from a user-set `""`, which maps to dark (`:75`); (b) the names
  `dark`, `light`, `system`, `""` never read a file; (c) a custom name reads the file on EVERY call, silently (no home, read error,
  invalid UTF-8, invalid JSON all give dark, `:85-90`), nothing is logged; (d) the path is exactly `dir.join(format!("{name}.json"))`,
  the name unsanitised (`../x`, an absolute path replaces the dir, `with_extension` would change `my.theme`; 8.5b's `should_join_the_name_unsanitised` and 8.5d's `find_palette` test pin the dotted and the absolute name, which `theme-diff.sh` group C does not: it only has `sub/x`, `../up`, a non-ASCII name and `''`, and re-locking its hash was not worth the churn); (e) only the HOME
  family and `TABSMITH_HOME` matter, `TABSMITH_SETTINGS_PATH` does not move the themes dir. It runs at three moments (`pty.rs:451`
  start, `:1021` late OSC 11 only when `bg.is_some()`, `:1283-1284` reload), each re-reading the file; `set_theme` stays
  unconditional (it clears `last`, `render.rs:264-267`). `terminal_bg` is `Option<(u8,u8,u8)>` end to end (`pty.rs:248, 352, 1641`).
- **P8-T26 `Theme::from_json`** [render T5, T6; theme 0.5]. The Fig format needs `version` to be the STRING `"1.0"` AND `theme` an
  object (`:99-101`); anything else, `{}`, `[]`, `5`, `null`, `{"version":"1.0"}`, `{"version":1,..}`, takes the base16 path and yields
  the filler theme (bg (24,24,24)), not dark. Each colour is `pointer(p).and_then(as_str).and_then(parse_color)` (`:97`): a number,
  a null or an unparsable string falls back for that field only (Fig to dark's field, base16 to the filler). Base16 roles: main_bg
  shade0, main_text shade6, matching_bg shade1, selected_bg shade2, selected_text shade7, selected_matching_bg accent6,
  description_text shade5, description_border shade1 (`:118-127`). `description_border` is built and compared by `PartialEq` but
  never read to draw. The lenient reader must stay a `Value` walk: a typed struct would reject whole files.
- **P8-T27 `layout`** [render T7]. Side tie-break (`:648-653`): Below if `below >= h`, else Above if `above >= h`, else `below >= above`
  picks Below; shrink only when `items > 0 && room >= 2` (`n = room - 1; h = room`); `items == 0` needs one row; `x0` is `(cx -
  query_w - 4).clamp(0, cols - w)` with `query_w` the SELECTED item's `query` (`:405-408`). Plain `usize` `-` stays (a debug panic
  stays a panic); preconditions are `compose`'s guards (`:329-336`).
- **P8-T28 `describe`** [render T8]. It uses the UNCLAMPED `menu.selected()` (`:601`) while `menu_patches` clamps (`:404`); a blank
  description counts as missing (`:604`) yet the shown text is `clean(&d)` untrimmed (`:612`); `file`/`folder` words only for
  `SuggestionKind::File/Folder` without a usable description; an own description has an EMPTY label, the hint gives
  `(clean(a.name()), clean(a.description()))`. The `_ => None` (`:609`) is a catch-all over `Option<SuggestionKind>`: D8 explicit arms
  `Some(Subcommand | Option | Arg | Special | Shortcut | History | AutoExecute) | None => None`.
- **P8-T29 Text fitting** [render T9, T10, T13]. Width is `UnicodeWidthChar::width` per char with `unwrap_or(0)` (`:833`), never
  `UnicodeWidthStr`, never `width_cjk` (the tmux placeholder U+10EEEE counts 1 and its marks 0; `tmux_images_are_placeholders`);
  `clean` maps each `is_control` char to one space, no collapsing; `fit` returns the input unchanged when it fits (even `w == 0`) and
  appends `…` only when `w > 0`; `wrap` splits on `split_whitespace`, starts with one empty line, hard-breaks long words, truncates
  with `fit(&format!("{l}…"), w)`, `max == 0` gives `[]`. Keep the `usize` arithmetic (no `saturating_*` added or removed).
  Look-alikes that must not be merged: `tracker.rs:211 parse_color`, `icons.rs:306 hex`, `cli.rs` strict theme parser; only
  `icons.rs:294 width` equals `text_fitting::width`.
- **P8-T30 Autocomplete width** [render T15, renderer 15]. `menu_patches` reads `s.f64(AUTOCOMPLETE_WIDTH).max(10.0) as usize` (`:409`;
  20.7 gives 20, a string gives the catalog 40, NaN gives 10); it stays in the renderer verbatim.

### Icons (8.4a, 8.4b)
- **P8-T31 `slot` order** [icons T1-T3, N4]. The wide-emoji check (`icons.rs:141-143`) runs BEFORE `g == Graphics::Off` (`:144-146`); in
  Off a `fig://` image is ignored; in Tmux a badge slot falls to `_ => text(&s.text)` (`:154-161`): image dropped AND `badge: false`,
  so the renderer draws it in the base pen, not white. Placeholders put kitty characters into the slot text for Tmux; accept and
  document it on `IconId`.
- **P8-T32 Ids** [icons T4-T7, 21]. PNG id = position in `PNGS` + 1 (`:246`), tinted id = `0x0100_0000|r<<16|g<<8|b` (`:257`); the
  array ORDER is the contract (also the tmux `38;5;{id}` foreground and the `Renderer.sent` key, `render.rs:203, 294, 376`).
  `placeholders` hard-codes diacritic 1 for any non-zero top byte (`:166-169`); `color(id)` is `;38;5;{id}` below 256, else
  24-bit even when `!truecolor` (`:172-178`), leading `;`, never routed through `Renderer::rgb` or `xterm256`. Operator
  precedence (`>>` before `&`, `<<` before `|`) is load-bearing: move the text. `badge_slot(None)` is the plain template PNG (id 39),
  not a tinted RGBA (`:260`); `.expect("template is in PNGS")` (`:242, 260`) stays pointing at the new list.
- **P8-T33 Resolution rules** [icons T8-T14]. `tinted`: `scale = a*b/256` in `u16` (255*255 gives 254), alpha copied, `chunks_exact(4)`
  (`:275-283`); `wide_emoji` strips EVERY U+FE0F then needs exactly 1 char of width 2 (`:182-185`); `spec_image` prefixes are exactly
  `fig://icon?` / `fig://template?`, no percent-decoding (`:189-197`); `hex` is 6 bytes, no `#`, accepts a leading `+`, and is not
  `parse_color`; `param` takes the first matching key, `split_once('=')`, a `&` inside a badge truncates it; badge text drops
  U+FE0F, empty gives `"  "`, `pad2` pads only width-1 strings (`:262-292`); `type_image` (`:200-212`): Special tinted (0x62,0x8d,0xad)
  with `➡️`, Shortcut (0x34,0x98,0xdb) with `💡`, History `text("📚")` with no image in any mode; `ascii_slot` (`:215-227`) is exhaustive.
  The slot text of every ROW KIND is 2 columns in every mode (the `resolves_like_q` loop). A spec badge is NOT fitted: `pad2` pads only width-1 text, so `fig://template?badge=abc`
  stays 3 columns and a width-0 badge stays 0 (existing behaviour, do not fit to 2). `IconSlot::new` and `icon_resolver` add no width check, `debug_assert` or padding.
- **P8-T34 Kitty protocol** [icons T15-T20]. `transmit` (`:317-347`): first chunk `\x1b_G{action},i={id},{format},q=2,m={more};{chunk}\x1b\\` in
  exactly this key order (Tmux action `a=T,U=1,c=2,r=1`, else `a=t`), later chunks `\x1b_Gm={more};..`, `more = (k+1)*CHUNK <
  enc.len()`, `CHUNK = 4096`, empty payload gives `""`, Tmux wraps EVERY chunk in `\x1bPtmux;..\x1b\\` and doubles every ESC; Png is
  `f=100`, Rgba `f=32,s=32,v=32`. `place` (`:351-353`) uses the patch index + 1, not the image id (`render.rs:382`);
  `CLEAR_PLACEMENTS` only for Direct (`render.rs:309-311, 353-355`). `base64` is hand-rolled; no crate. `tmux_graphics`: `trim`,
  `split_once(' ')`, first field exactly `on`/`all`, rest searched case-sensitively for `kitty`/`ghostty`. `detect` (`:101-122`): `TMUX`
  set even to `""` selects the tmux path, `env::var(..).ok()` treats non-UTF-8 as unset, argv `tmux display -p [-t $TMUX_PANE]`,
  `.output()` with exit status ignored, spawn error gives Off, NO timeout, no test; it is called once at session start
  (`pty.rs:452`) inside the "everything that can fail goes before the child exists" block.
- **P8-T35 Seam hazards** [icons N1-N3, N6, N7; renderer 5, 21]. `Renderer.sent` is keyed by the `u32` kitty id (`render.rs:376`); keep it
  `u32 = icon.value()`. `tinted` runs today on every `slot()` call and will run only at transmit time (once per colour per
  session): invisible in bytes while `sent` keeps its key, a performance-only difference. `IconId::Png(usize)` is not validated (only
  the resolver builds one). `GraphicsSupport` stays `Copy + PartialEq` (`render.rs:309, 353, 380, 497`). `include_bytes!` paths resolve
  relative to the file that invokes `pngs!`: macro, its one invocation and `TEMPLATE_RGBA` stay in `icon_assets.rs`.

### Theme resolution and the `theme` command (8.5a-8.5d)
- **P8-T36 Order of effects** [theme T1, cons 44]. `theme()` (`cli.rs:96-135`): `data_directory_from_env().themes_dir()?` first, even for
  `--list` (no home: `error: could not determine the home directory (set HOME or TABSMITH_HOME)`, exit 1, ahead of the settings
  load); `--folder` (`create_dir_all` then `println!("{}", dir.display())`, an io error with no path context, `File exists (os
  error 17)`; no settings read); `--list` (never errors afterwards; one `println!` per name); then `repository.load()?` STRICT before
  looking at NAME (a corrupted file wins over "does not exist" and over an invalid theme file); no NAME prints the current value;
  NAME reads the file FIRST for every name, built-ins and `""` included.
- **P8-T37 Banner before save** [theme T2, T15]. The banner is printed before the save (`:127` vs `:133`): a save failure leaves the banner on
  stdout and `error: Permission denied (os error 13)` on stderr, exit 1. Use an `announce` callback (precedent
  `DeleteSettingsUseCase::execute(.., announce)`). One print call per item (`print!` for the banner ending `\n\n`, `println!` for
  the built-in line and per listed name); never join them into one write.
- **P8-T38 File first, built-ins second** [theme T3, T4]. A readable `themes/dark.json` shadows the built-in `dark` in the CLI (banner,
  validation, even an invalid `light.json` makes `theme light` fail) while the renderer never reads it; `theme ""` fails unless a
  file named `.json` exists, in which case `""` is saved; the CLI list is `["dark","light","system"]` without `""` (`cli.rs:65`), while
  the renderer maps `""` to dark. Any read failure (not found, permission, a directory named `x.json`, invalid UTF-8) is "no theme"
  silently, in both paths; no logging may be added.
- **P8-T39 Strict parser texts** [theme T5, T5b, T6, T7, T17]. The serde texts are public output: `invalid type: .., expected struct ThemeFile
  at line L column C`, ``duplicate field `author` ``, `expected a map`, `EOF while parsing a value`, `trailing characters`, `expected value
  at line 1 column 1` (BOM). They depend on the Rust idents `ThemeFile`/`ThemeAuthor` (serde's `expecting` uses the ident), the field
  types `Option<String>` / `Option<Map<String, Value>>`, `#[serde(flatten)] rest`, and `from_str` on the raw text (`from_value` loses
  `at line L column C`). A derived `ThemeAuthor` accepts a 3-element ARRAY positionally (name, twitter, github) and reports `invalid
  length N, expected struct ThemeAuthor with 3 elements` otherwise: a hand-written validator would change both. `version` and `theme`
  are `#[allow(dead_code)]` yet load-bearing (without them the keys fall into `rest`); keep the attributes (D7 spirit). The `rest`
  loop iterates a `Map` (BTreeMap): the FIRST offending `shade*`/`accent*` key in sorted order is reported (`accent0` before
  `shade9`), `null` counts as non-string, any key with the prefix counts (`shadeX`). The strict parser never validates colours, the lenient
  one never validates structure; unifying either way changes accepted inputs (K8, R16).
- **P8-T40 `theme_list`** [theme T8]. Exact extension `json` (not `JSON`, not `json.bak`), `file_stem()` (so `.hidden.json` is `.hidden`,
  `a.b.json` is `a.b`), a directory `dirtheme.json` IS listed, non-UTF-8 stems skipped, entries that error skipped, a `read_dir` error
  gives the built-ins only, byte-order `sort()` then `dedup()` (an uppercase stem sorts before every lowercase one, `Beta` before `a.b`: neither `theme-diff.sh` group A nor the legacy test has one, so 8.5b's list test adds `Beta.json` to pin it), built-in stems dropped (`dark.json` hidden from the list yet
  selectable), built-ins first. `Zed.json` and `zed.json` are one file on macOS, so a test uses one of them, not both.
- **P8-T41 Showing the value** [theme T9]. No NAME: a non-string `autocomplete.theme` (including `null`) prints `format!("{:#}",
  setting_value_to_json(v))` (pretty, 2 spaces, numbers keep their kind, keys sorted); anything else prints `settings.string(KEY)`
  (raw text, `""` an empty line, unset the catalog `dark`). `ShowThemeUseCase` must look at `settings.get` first, because
  `Settings::string` would print `dark` for a wrong-typed value. dropdown's infrastructure owns its own `SettingValue` to JSON copy
  (rule 3 bars `settings::infrastructure::..setting_value_json`).
- **P8-T42 Env and platform** [theme T11, T13, T14]. The data directory is read inside `theme()` today; building it in bootstrap earlier is
  side-effect free (Phase 7 precedent, deviation 12). `theme` needs HOME or `TABSMITH_HOME` even when `TABSMITH_SETTINGS_PATH` is
  set. No `cfg`, panic, thread or logging in the theme code; new chmod-based tests need `#[cfg(unix)]`. clap ids `list`, `folder`,
  `theme` are the `conflicts_with_all` names and appear in help and completion output (D4; the golden captures `theme --help` and
  `completion bash|zsh|fish`).
- **P8-T43 Look-alikes** [theme 0.5, T12]. The renderer and the CLI disagree by design: `theme NAME` can pass the strict parse while the
  renderer draws the filler (`{}`) or ignores the file (a built-in name), and the CLI prints a non-string `autocomplete.theme` as JSON
  while the renderer treats it as `dark`. Both stay.

### The overlay renderer (8.4b, 8.6)
- **P8-T44 State machine** [renderer 1-5, 10]. The clear is computed at DRAW time (`render.rs:385-386`, stored by `frame` `:298`) and taken by
  the pty BEFORE it writes new output (`pty.rs:765`, `:700`, `:1291`); dedupe `frame == last && !clear.is_empty()` (`:289`); `last` is reset
  by `set_theme`, `take_clear`, `drop_clear` (`:264-279`); `compose` runs before the check; output order `take_clear()` bytes, image
  transmissions (`sent.insert` at emit time), then the frame; `self.clear` and `self.last` are assigned after `take_clear`; `sent` is never
  cleared; `full_repaint` calls `drop_clear()` first. `frame` returns `take_clear()` when `compose` is `None`.
- **P8-T45 Patches and placements** [renderer 6, 7, 9, 16-18]. The placement id is the patch index + 1 over ALL patches (list rows, pop-out
  rows or the description row, ghost last), placed at `p.x + 1`, only in Direct; wide-char widening is asymmetric (`:361-364`, pad pen and
  `' '`, restore range `xs..p.x+p.w+right`); `sel = selected.min(len-1)` (`:404`) while `describe` is unclamped; pop-out side (`:430-435`: right
  if `x0+w+POP_W <= cols`, else left if `x0 >= POP_W`, else the description bar with the spinner); `list_row` hint only if the FULL title fits
  (`:505`), the ellipsis is never highlighted (`:507-511`), badge pen white (255,255,255) (`:496`), a Tmux slot pen carries a second `;38;5;id`
  (`:498`); `desc_row` always emits the italic pen, even for an empty description, and the hard-coded `No description` (140,140,140) (`:542-562`);
  `ghost_patch` uses the fixed `\x1b[0m\x1b[90m` pen, not the theme (`:704`, do not "fix").
- **P8-T46 Guards** [renderer 8, 11-13]. `compose`'s guards (`:328-337`) are mirrored by `pty.rs:979-990 drawable` (comment at `:978` names `render.rs`:
  repoint it in 8.6); `tail`, `restore_cells`, `sgr`, `color` (`NamedColor` discriminants of alacritty_terminal 0.26.0) and `blank` (drives ghost clipping
  AND the repaint trim) move verbatim. A panic inside the renderer is swallowed by `Session::guard` (`pty.rs:693-702`) and makes the session a
  permanent pass-through: an index or overflow slip shows up as "no menu", not as a failure; hence the byte-hash guard (8.0b).
- **P8-T47 Name clashes in one file** [renderer 1]. In the renderer `color` is the model-cell colour fn, NOT the icon `color`; keep both qualified. In
  `pty.rs`, alacritty's `r.Event.KeyEvent` (P8-T11) is not ours.

### Cross-cutting
- **P8-T48 Moves** [all]. Code moves by script or `git mv`, then renames, then extracts; tests move with their names; `pub` becomes `pub(crate)`; domain
  fields private (accessors, `*Parts` structs where a constructor would need more than 7 parameters: clippy budget 3 is all used).
- **P8-T49 Fitness reading** [cons 4A]. The scanner reads string and char literals as code, but only an unpaired `{` or `}` in one shifts its use-group tracking
  (`paths_in`); `(` and `[` matter only inside a bodyless `#[cfg(test)]` item (`TestOnlyItem::holds`). Verbatim-moved literals such as `color_parser`'s `"rgb("` and `')'` stay as they
  are (measured clean); if a registration run reports a line, escape the char (`'\x7b'`), never rewrite the logic. `keystroke_parser.rs` has `'+'` and `'f'` only; the decoder literals
  (`b'['`, `b"\x1b[200~"`) live in unregistered `terminal_session`. A `//` after `:` (`fig://icon?`) does not start a comment
  (`without_line_comment`). Run the local registration after every step (R24).
- **P8-T50 Windows and Linux** [all]. None of the five legacy areas holds a `cfg(windows)`, `cfg(unix)` or `cfg(target_os)` (`grep -n 'cfg(' ` finds only
  `#[cfg(test)]`); CI still compiles `detect()`'s `Command::new("tmux")` and the `include_bytes!` depth on ubuntu and windows first. New chmod
  tests carry `#[cfg(unix)]`.

## Decisions

R1-R30 are binding; behaviour first (D1). Each gives the options considered and the one taken.

### R1 Phase start, guards first, three runs
- `phase8-start` is the local lightweight tag at `7aafcef` (src-identical to the end of Phase 7). It stays there: the hand-over docs commit changes
  only docs, and the testlock baselines are `ef807c1` and `phase8-start`.
- Step **8.0a** adds `theme-diff.sh` and the three lenient-theme characterisation tests; **8.0b** adds the icon and overlay byte-hash tests and
  the keymap/menu characterisation tests. All pass on the old code before anything moves (P8-T1, T3, T4, T19, T26, T32-T34, T44).
- Runs: **1** = 8.0a, 8.0b, 8.1a, 8.1b, 8.1c, 8.2a, 8.2b, 8.2c, 8.4a (guards, keys, menu, the icon seams); **2** = 8.4b alone (the riskiest step:
  one compile-coupled commit that changes what `slot()` returns); **3** = 8.3a, 8.3b, 8.3c, 8.5a, 8.5b, 8.5c, 8.5d, 8.6, 8.7.
- Rejected: the plan's five steps as five (each is 600-1400 lines with a different concern; Phase 7 also cut four into eleven); icons last, with a
  bridge `icons::image(IconId)` across two steps (a shim living two steps; `slot()` cannot return ids while the renderer still wants bytes, so
  the split has to be one commit anyway).

### R2 Step order
Plan order is keys, menu, pure render and icons, theme, renderer. Taken: guards, keys (8.1), menu (8.2), the icon seams (8.4a), the icon split (8.4b),
the pure render parts (8.3), the theme (8.5), the renderer (8.6), the close (8.7). Why: icons need only the dropdown scaffold (8.1a) and the
`item` fixture (8.2a), not text fitting (the one `width` copy is deduped in 8.3a); doing the riskiest step before the render.rs edits of 8.3 and
8.5 keeps its diff small; `describe` needs `Menu` (8.2b) so 8.3c follows it; the theme repository shape needs `Theme` (8.3b). Rejected: 8.3 before
8.4 (the plan order; it makes 8.4b wait behind 8 steps for no dependency).

### R3 Key value objects: private fields with const builders (keys K-F; cons K-F)
Options: (A) `pub(crate)` fields on `Key`/`Modifiers`/`KeyEvent` (zero test edits, an exception to "domain fields are private", the Phase 7 grep `pub(crate) field
in the domain outside *Parts` would list them); (B) private fields, `const fn` builders and accessors. Taken: **B**, in its own step (8.1c) after the code has
moved verbatim. `Modifiers::{none, with_ctrl, with_alt, with_shift, with_meta}` are `const fn` (so test constants like `const CTRL: Modifiers =
NONE.with_ctrl(true)` keep working) with getters `ctrl() alt() shift() meta()`; `Key::new(code, modifiers)`, `code()`, `modifiers()`, `with_alt()`;
`KeyEvent::new(key, bytes)`, `key()`, `bytes() -> Range<usize>`. `Modifiers` keeps `Default` and `Hash`. The mask in `decode_csi` is written as a chain
(`Modifiers::none().with_shift(m & 1 != 0).with_alt(m & 2 != 0).with_ctrl(m & 4 != 0).with_meta(m & 8 != 0)`), never a 4-bool `new`. Cost: five key
tests and `ghost_keys_and_words_keys` are CHANGED by 8.1c (struct literals and field reads), each a restructuring; three of them (`keystroke_grammar`, `decode_table`, `ghost_keys_and_words_keys`) are already CHANGED from 8.1a, because renaming the struct-literal type `Mods { .. }` to `Modifiers { .. }` is not a path for testlock's loose form. Because 8.1a and 8.1b move the code
verbatim with `pub(crate)` fields, the only intermediate state is a domain VO with public fields for two steps; nothing greps for it until 8.7.
New files beyond section 4: `key_code.rs`, `modifiers.rs` (one type per file).

### R4 `KeyEvent` and the decoder (keys PG1, PG3, PG10, PG12)
- `KeyEvent` (a `Key` plus the byte range of its stdin chunk) is not placed by the plan or the glossary. Taken: `terminal_session/domain/value_objects/key_event.rs`.
  It is produced by an infrastructure adapter and consumed by Phase 9's `KeyRouter` (domain) and `HandleKeystrokesUseCase` (application), which may not
  import infrastructure (rules 1-2), and only terminal_session reads byte ranges. Rejected: dropdown (dropdown has no business with stdin ranges), the
  adapter file (an application use case could not import it).
- `Decoder` becomes `StdinKeyDecoder` (adapters take a technology prefix and the file is named after the type), `terminal_session/infrastructure/adapters/stdin_key_decoder.rs`,
  `#[derive(Default)]`, `pub(crate) fn decode(&mut self, &[u8]) -> Vec<KeyEvent>` (port-ready shape). The free fn `decode` (test-only) becomes `#[cfg(test)]` (D7).
- Phase 8 puts no port in front of it: `pty.rs` keeps a `StdinKeyDecoder` field and the reset `self.decoder = StdinKeyDecoder::default()` (P8-T9). The port
  versus domain-service choice (keys PG10) is Phase 9's; hand-off note below.

### R5 `keymap_defaults_and_overrides` stays whole (keys PG7, cons K-D6)
It chains decoder (terminal_session) to `Keymap` (dropdown) through `Settings`. Options: the strict D6 split (a pure `Keymap` half with literal keys in `keymap.rs`
plus a decoder half asserting `decode(bytes) == key` for the 17 rows: six of those rows are not covered by `decode_table`, so the split adds assertions) or keeping it
whole. Taken: **whole**, verbatim, in `stdin_key_decoder.rs`'s tests (terminal_session to dropdown and settings is an allowed direction; the dropdown tests stay free of
terminal_session). Dropdown's `Keymap` gets its own tests from the 8.0b characterisation set instead. Recorded as a D6 deviation. The helper `settings(json)`
(`keys.rs:666`) is copied verbatim next to it (it imports `settings_from_json` like `ghost_text_source_resolver.rs:42` does; D2 exempts it).

### R6 `KeystrokeParser` is free functions; `normalize` is its second (keys PG4, PG5)
`dropdown/domain/services/keystroke_parser.rs` holds `pub(crate) fn parse_keystroke(&str) -> Option<Key>` and `pub(crate) fn normalize(Key) -> Key` (precedent:
`shell_quoting::quote`, `launch_policy::decide`); `Keymap` is its only caller of `normalize`. The glossary gains a `KeystrokeParser` row. `keymap.rs` keeps `DEFAULTS` private
(`&[(&str, KeyAction)]`, same 10 rows, same order) and `from_settings(&Settings)` with its signature. Names: `Mods` to `Modifiers`, `Action` to `KeyAction`, `Binding` to
`KeyBinding` (glossary); `KeyBinding::{Action(KeyAction), Ignore, Unknown}` verbatim.

### R7 The clock: `now` is a parameter; the renderer reads it itself (menu G1, K-C)
Options for `Renderer::menu_patches`'s second reader of `Menu::loading`: (A) add `now: Instant` to `FrameInput`, filled at `pty.rs:994`; (B) the renderer calls
`Instant::now()` at `render.rs:455`, exactly where the old read happened. Taken: **B**. It changes nothing observable, keeps `FrameInput` and the render tests' `input()` helper
untouched, and the renderer is infrastructure (allowed to read the clock). Phase 9 adds `now` to `FrameInput` when a `Clock` reaches the screen adapter. So:
`Menu::on_completion(c, settings, now: Instant)` (third parameter) and `Menu::loading(&self, now: Instant)`; `loading_since` stays an `Option<Instant>`; `loading` is
`self.loading_since.is_some_and(|t| now.duration_since(t) > Duration::from_millis(200))`. Callers: `pty.rs:891` and `:1242` pass `Instant::now()` (literally where the old
read happened; `on_timers`' tick `now` is rejected, it moves the comparison by microseconds for nothing), `render.rs:455` reads `Instant::now()`, the render test helper
`menu()` and the spinner test pass `Instant::now()`. `Session::on_completion` and the renderer still read the clock until Phase 9 (plan section 4's "Instant passed in, never
read inside" holds for `Menu` only; recorded).

### R8 `error_reset_and_loading` keeps its private write
It writes `m.loading_since = Some(Instant::now() - Duration::from_millis(300))` (`menu.rs:730`). Options: rebuild the scenario through `t0`/`t0 + 300 ms` (no real-time
dependence, no `Instant - Duration` underflow risk on a fresh Windows boot) or keep the private write and add only `Instant::now()` arguments. Taken: **keep the
write** (the locked test changes only by the new parameter); the deterministic variant is what the three new boundary tests use (P8-T16).

### R9 Menu move and split (menu G2, G3, G5, G6, G12)
- `git mv src/menu.rs src/dropdown/domain/entities/menu.rs` with the tests still inline (git keeps the rename), then the test module is sliced to the sibling
  `entities/menu/tests.rs` (555 lines, ~490 after the fixtures leave: over section 1's ~500 once `Menu` grows); `#[cfg(test)] mod tests;` at the bottom of `menu.rs`.
  The sibling is a child module, so `m.loading_since = ..` (`:730`) and `m.just_inserted` (`:846`) keep compiling.
- `Vis` becomes `MenuVisibility`, `Effect` becomes `MenuEffect` (`Clone, Debug, PartialEq`, not `Eq`, doc comments verbatim). `max_rows` stays `pub(crate)` in `menu.rs` (shared by
  `Menu` and the renderer; `MenuLayoutCalculator::layout` already takes `n_max`). `Menu::vis()` gets `#[cfg(test)]` (D7); `show_on_tab`'s unused `settings` parameter stays;
  `MenuEffect::Insert.full` stays (tests assert it): all three go on the Phase 11 watch list.
- `common_prefix`: `services/common_prefix.rs` holds `pub(crate) fn decide(items: &[Suggestion], selected: usize) -> CommonPrefixOutcome` and the private `matching_type`;
  `CommonPrefixOutcome` is a VO in its own file, `Failed | InsertSelected | InsertPrefix(Insertion)`. `Menu::common_prefix` becomes the apply step:
  `Failed => None`, `InsertSelected => Some(self.insert_full(self.selected, false))`, `InsertPrefix(ins) => { self.just_inserted = true; Some(vec![MenuEffect::Insert { ins, full: false, record: None }]) }`.
  Same effects, same mutation order, same `Option` contract for the callers at `:235, 239, 242`; the service borrows `&items[selected]` instead of cloning `sel`
  (same results; a bad index still panics, never reached because `handle` guards empty). Names are the plan's `CommonPrefix` plus a new outcome (glossary rows).
- D8: `v => v` at `menu.rs:150` becomes `MenuVisibility::Visible => MenuVisibility::Visible, MenuVisibility::HiddenUntilShown => MenuVisibility::HiddenUntilShown`; the inner
  `match action { ToggleDescription => !self.popout, a => a == ShowDescription }` (`:266-269`) becomes three arms (`ToggleDescription`, `ShowDescription`, `HideDescription`), each
  `if !always_desc { self.popout = !self.popout / true / false }; Vec::new()`. The other `_ =>` arms (`:174, :375, :414`) match `Option`s and tuples, not domain enums.

### R10 Fixtures go to `completion/domain/test_fixtures.rs` (menu G4)
`item`, `comp` and `settings` (`menu.rs:479-537`) move there, `pub(crate)`, bodies byte-identical (diffed against `git show phase8-start:src/menu.rs`). `item` and `comp` import only completion and
specs domain types and need no attribute. `settings(json)` uses `serde_json` and `settings_from_json` (infrastructure), which rules 1 and 3 forbid in a file read as production code, so
its `use` line and its fn each carry `#[cfg(test)]` (CON-T28). Importers switch in the same commit: `menu.rs` tests, `pty.rs:1758`, `render.rs:893`, `icons.rs:379`. Rejected: a settings-owned
home (`settings/infrastructure/testing/`; one more module nobody asked for; there are already six private copies of the helper). `subs`, `typed`, `prefix_fx` and `with` stay private in `menu/tests.rs`.

### R11 `Rgb` stays a tuple alias (render G2, cons K-R)
The glossary calls it a VO; a newtype would change every tuple literal in `colours_and_themes`, `row_contents` (`r.pen((180,180,180), (95,89,56), "")`), `parse_osc11`, `probe_replies`
(`pty.rs:248, 1641`) and the icon code. Taken: `value_objects/rgb.rs` = `pub(crate) type Rgb = (u8, u8, u8);` with a `// ponytail:` note (precedent `AliasTable`, `SpecFunctionRef`); the
glossary row is amended to "VO (alias of a tuple)". `IconId::Tinted(u8, u8, u8)` and `badge_slot(Option<(u8,u8,u8)>, ..)` keep plain tuples.

### R12 `Theme` and `ThemeParts` (render G3, cons K-R)
`value_objects/theme.rs`: `Theme` (`Clone, Debug, PartialEq`, no `Copy`) with private fields, `ThemeParts` (the eight named `pub(crate) Rgb` fields; a positional `new` would have 8
parameters, one over clippy's limit, and the budget is spent), `Theme::new(ThemeParts)`, eight accessors (`main_bg() main_text() matching_bg() selected_bg() selected_text()
selected_matching_bg() description_text() description_border()` returning `Rgb`), `Theme::dark()` and `Theme::light()` with the same values (P8-T26). The renderer's field reads
(`render.rs:438, 485-489, 537-560`) and the tests' become accessor calls. `SHADES`/`ACCENTS` stay in `render.rs` until 8.5d moves them into `ThemePalette`.

### R13 The one temporary shim: `impl Theme { from_settings, from_json }` in `render.rs` (render G1)
At the end of 8.3b the custom-theme branch (env, fs, serde) has no home yet. An inherent `impl Theme` block in another module of the same crate is legal: `render.rs` keeps
`from_settings`, `from_json`, `SHADES`, `ACCENTS` (and so `serde_json::Value`, `data_directory_from_env`), building through `Theme::new(ThemeParts{..})` and the accessors. `pty.rs`'s three
`Theme::from_settings(..)` calls and `colours_and_themes_theme_json`/`_theme_choice` (the two halves of `colours_and_themes` that stay in `render.rs` in 8.3a) keep calling it.
**8.5d deletes it** with `SHADES`, `ACCENTS` and the two serde imports. It never leaves Phase 8. Rejected: a half resolver in 8.3b (without the repository it would be dead code
with no test), free fns `theme_from_settings`/`theme_from_json` (the same shim under another name, with the `pty.rs` call sites edited twice).

### R14 `ThemeRepository` has four methods, two result types (theme G1, G2, cons K-T)
`domain/repositories/theme_repository.rs`, trait + `#[cfg(test)] InMemoryThemeRepository` in the same file (plan section 1):
```rust
pub(crate) trait ThemeRepository {
    fn folder(&self) -> Result<PathBuf, DropdownError>;                       // themes dir; the no-home error
    fn find(&self, name: &str) -> Result<Option<ThemeDefinition>, DropdownError>;   // STRICT (the CLI)
    fn list(&self) -> Vec<String>;                                            // every *.json stem, sorted, deduped
    fn find_palette(&self, name: &str) -> Option<ThemePalette>;               // LENIENT (drawing); 8.5d
}
```
`find`: `Ok(None)` = no readable file (also no home, a directory named `x.json`, invalid UTF-8), `Err(DropdownError::InvalidThemeFile { path, reason })` = readable but not a valid theme.
`find_palette`: `None` = unreadable OR not valid JSON (the drawing falls back to dark); any valid JSON gives a palette (the filler for `{}`). `folder()` is how `--folder` and
`ThemeNotFound` learn the directory, and how the handler keeps "no home fails first" (P8-T36). The plan's `{find, list}` cannot be literal (D1): the CLI needs the strict read and the
folder, the renderer the lenient read. Options: one trait (taken, YAGNI) or a one-method `ThemePalettes` port for the renderer (terminal_session would hold a smaller port; rejected, a
second trait for one method). Naming clash settled: the glossary's `ThemeDefinition` replaces `cli::ThemeFile` (the STRICT result: the author, for the banner), and the lenient result is a
new VO `ThemePalette` (colour roles with fallbacks, `theme() -> Theme`).

### R15 `ThemeChoice`, `ThemePalette` and `resolve_theme` (render G4-G6, theme G3)
- `value_objects/theme_choice.rs`: `ThemeChoice { Dark, Light, System, Custom(String) }`, `ThemeChoice::BUILT_IN_NAMES: [&str; 3] = ["dark", "light", "system"]` (the CLI's `BUILTIN_THEMES`, order kept),
  `from_name(&str)` (the RENDERER rule: `""` and `"dark"` are `Dark`, then `light`, `system`, else `Custom`) and `built_in(&str) -> Option<ThemeChoice>` (the CLI rule: only the three names, `""`
  excluded, P8-T38). Only the NAME list is deduped; the two rules stay two places (D1).
- `value_objects/theme_palette.rs` (8.5d): `ThemePalette` = the lenient result, `Fig` (eight `Option<Rgb>` roles) or `Base16` (the seven values the old code reads: shade0, 1, 2, 5, 6, 7 and accent6, each `Option<Rgb>`),
  built with a `*Parts` struct, and `theme() -> Theme` applying the fallbacks (Fig: per field to `Theme::dark()`; base16: per key to the parsed `SHADES`/`ACCENTS` filler; both consts move here; P8-T26).
- `services/theme_resolver.rs` (8.5d): `pub(crate) fn resolve_theme(settings: &Settings, terminal_bg: Option<Rgb>, themes: &dyn ThemeRepository) -> Theme`: `settings.string(AUTOCOMPLETE_THEME).unwrap_or_default()`
  then `ThemeChoice::from_name`; `Dark` and `Light` direct; `System` by the f64 luma of `terminal_bg` (P8-T24); `Custom(name)` is `themes.find_palette(&name).map(ThemePalette::theme).unwrap_or_else(Theme::dark)`. The
  repository is consulted only for `Custom`. Options (render G6): (a) the service takes the domain-owned trait object (taken: plan section 2's bridge row says the theme repository is injected into
  terminal_session as a trait object), (b) a pure `resolve_theme(&ThemeChoice, bg, Option<&ThemePalette>)` plus glue at each of the three call sites (rejected: three copies of the glue), (c) an injected closure
  (rejected, as `find_global_spec`'s was only for a one-off). The terminal background enters as `Option<Rgb>`: dropdown may not import `TerminalBackground`. Departure from the skill: its domain-services reference says a domain service has "No external dependencies" and gives "Uses ports" to use cases; `resolve_theme` is a domain service that calls a repository trait (the dependency direction is legal: domain trait, domain service, infrastructure adapter; plan section 4 lists `theme_resolver` under `domain/services`). Taken because the custom branch is the only reader of the theme file and option (b) would copy the same glue to three legacy call sites; Phase 9 may turn it into a use case when `terminal_session` owns the session (deviation 15).
- The holder is `Rc<dyn ThemeRepository>` (the session already holds `Rc<dyn CompletionEngine>`, `pty.rs:332`; unverified that nothing else needs `Send`, O5).

### R16 Two theme parsers, one mapper file, two entry points (theme 0.5, render G4, plan section 4 "one parser, not two")
Cannot be literal (D1); the table is verified on ~120 invocations:

| input | strict (`theme NAME`) | lenient (drawing) |
|---|---|---|
| unreadable / missing / a directory / non-UTF-8 | falls through to the built-in check, then "does not exist" | dark |
| no home | error before anything | dark |
| invalid JSON, empty, BOM, trailing text | `invalid theme file <p>: <serde text with line/column>` | dark |
| valid JSON, not an object (`[1]`, `"x"`, `null`, `5`) | `invalid type: .., expected struct ThemeFile at line L column C` | base16 filler (bg (24,24,24)), not dark |
| `{}` | Ok, no author | base16 filler |
| `author` a string / number / bool | `invalid type: string "Ann", expected struct ThemeAuthor at line 1 column 15` | ignored |
| `author` an ARRAY of exactly 3 strings or nulls | Ok, positional name, twitter, github | ignored |
| `author` an array of 0, 1, 2 or 4 items | `invalid length 1, expected struct ThemeAuthor with 3 elements ..` (4 items: `trailing characters ..`) | ignored |
| `author.name|twitter|github` non-string non-null | `invalid type: integer `5`, expected a string at line L column C` | ignored |
| `version` non-string non-null | the same text shape | the Fig format is not taken: base16 |
| `version` a string other than exactly `"1.0"` | Ok (never compared) | base16 |
| `theme` a string or array | `expected a map` | base16 |
| a top-level `shade*`/`accent*` key with a non-string, `null` included | `<key> must be a string`, first key in sorted order | only shade0,1,2,5,6,7 and accent6 are read; a non-string gives the filler for that index |
| unparsable colour strings (`"red"`) | Ok (never checked) | per-field fallback |
| duplicate `author` | ``duplicate field `author` `` | last key wins |
| a name `dark` `light` `system` `""` | the file is consulted FIRST | never reads a file; `""` is dark |

Taken: `infrastructure/adapters/theme_json_mapper.rs` holds both entry points: `parse_theme_definition(text: &str) -> Result<ThemeDefinition, ThemeFileError>` (strict: the two derived serde structs
`ThemeAuthor` and `ThemeFile` moved verbatim with their idents, `#[allow(dead_code)]` on `version`/`theme`, `from_str` on the raw text, the `shade*`/`accent*` loop; `bail!` becomes `ThemeFileError`, `pub(crate)`
(a private type in that signature raises `private_interfaces`, a 4th clippy warning) and in its own file `adapters/theme_file_error.rs` (plan section 1: one file per type; `atomic_file_error.rs` is the precedent), an enum of two text-carrying variants with the same `Display`, no `anyhow` in adapters and no serde type in the file) and `theme_palette_from_json(&Value) -> ThemePalette` (lenient: the `col` closure over `v.pointer`, the `"1.0"` plus object switch, then
the role values). The rules (fallbacks, filler, role map) are domain (`ThemePalette::theme()`), the serde is infrastructure. Strict-accepted is a subset of valid JSON, so the lenient parser never fails
where the strict one passes, but their results are not comparable.

### R17 The theme use cases, the handler and the module (theme G4, G5, G7, G10)
- `application/select_theme_use_case.rs`: `execute(&self, name: &str, announce: impl FnOnce(&ThemeSelection)) -> Result<(), DropdownError>`: `settings.load()?` FIRST (P8-T36), then `themes.find(name)?`
  (a file wins), else `ThemeChoice::built_in(name)`, else `Err(ThemeNotFound { name, folder })` (`themes.folder()?`); `announce(&selection)` BEFORE `settings.save(&settings.with(AUTOCOMPLETE_THEME,
  SettingValue::Text(name)))`; the saved value is the name as typed. DTO `ThemeSelection { Custom(ThemeDefinition), BuiltIn(ThemeChoice) }` (`application/theme_selection.rs`; it holds two domain value objects, not primitives: a DTO exception like `SettingValue` and Phase 7's `GhostTextSources`, because `announce` and the banner need the author, deviation 19).
- `application/show_theme_use_case.rs`: `execute(&self) -> Result<CurrentTheme, DropdownError>`; DTO `CurrentTheme { Named(String), NotText(SettingValue) }` (`SettingValue` is the DTO exception of section 1);
  explicit arms: `Some(v) if v.as_str().is_none() => NotText(v.clone())`, `Some(_) | None => Named(settings.string(KEY).unwrap_or_default().to_string())` (D8).
- `application/list_themes_use_case.rs`: `execute(&self) -> Vec<String>`: the three built-in names first, then `themes.list()` minus the built-in names (P8-T40). Where the built-in filter sits is a plan silence: the adapter's
  `list()` returns every stem sorted and deduped (determinism is a file-system concern), the use case drops and prepends (the built-ins are the domain's). The two halves of `theme_list_order` assert the union.
- `--folder` is no use case (a fourth for one `mkdir` would be a deviation): the handler calls `module.themes.folder()?`, `std::fs::create_dir_all(&folder)?`, `println!("{}", folder.display())`, the io error unannotated (P8-T36).
- `infrastructure/cli/theme_command.rs`: `ThemeArgs` (clap attributes, doc comments, field names and order verbatim, D4; `pub(crate)`, fields private like `SettingsArgs`), `run_theme_command(args, &DropdownModule) ->
  anyhow::Result<i32>` (the order of P8-T36 with `module.themes.folder()?` first), the private `theme_banner(name, &ThemeDefinition, color)` (the emoji and arrows copied by script, `bold` from
  `shared::infrastructure::terminal_style`, `stdout_tty()` read at the same point), and a private `SettingValue` to JSON fn (~25 lines: `u64`, then `i64`, then `f64`, as `setting_value_json.rs:61-84`; the
  duplication is accepted, rule 3 bars the settings one) guarded by a parity test over the same corpus against the settings fn (test code is exempt, D2).
- `infrastructure/dropdown_module.rs`: `DropdownModule { select_theme, list_themes, show_theme, pub(crate) themes: Rc<dyn ThemeRepository> }` and `build_dropdown_module(&SharedDependencies, Rc<dyn SettingsRepository>)`
  (mirrors `InlineCommandModule`); `bootstrap/cli.rs` adds `fn dropdown_module() -> DropdownModule` beside `inline_command_module()` (`Rc::new(JsonFileSettingsRepository::new(dependencies.data_directory.clone()))`), changes
  `Theme(cli::ThemeArgs)` to `Theme(ThemeArgs)` (variant order and the doc comment `/// Get or set theme` unchanged, D4) and the arm to `run_theme_command(a, &dropdown_module())`.
- `DropdownError` (`domain/dropdown_error.rs`, hand-written `Display`/`Error`, no `anyhow`): `Settings(SettingsError)` (transparent, `source()` continues, like `GhostTextError`), `DataDirectory(SharedError)` (transparent),
  `InvalidThemeFile { path: PathBuf, reason: String }` with `Display` `invalid theme file {path}: {reason}`, `ThemeNotFound { name: String, folder: PathBuf }` with `'{name}' does not exist in {folder}` (path via `.display()`),
  `From<SettingsError>`. `{err:#}` must print the old text exactly (the reason is pre-rendered, `source() = None`, as `ShellIntegrationError`'s chains, Phase 4 P3).
- `cli::settings_repository()` (`cli.rs:57-60`) has no caller after the move: deleted in 8.5c with the theme code.

### R18 `FileThemeRepository`
`infrastructure/adapters/file_theme_repository.rs` holds a `DataDirectory` (D3: the deferred no-home error stays inside; nothing in the process sets `TABSMITH_HOME`/`HOME`/`USERPROFILE`, the only `set_var`s are `ZDOTDIR` and
`TABSMITH_DEBUG_SHELL`, `cli.rs:390-393`, so building it once is unobservable), resolves paths per call, reads the file on every `find`/`find_palette` call (no cache: P8-T25), never logs, never creates the directory,
joins `format!("{name}.json")` unsanitised, lists with `read_dir` (errors give an empty list), exact extension `json`, `file_stem().to_str()`, `sort()`, `dedup()`. It is the only caller of `themes_dir()` after the move.

### R19 Placement of the pure render helpers (render G8, G11; renderer 23)
`color_parser.rs` (`parse_color`, `xterm256`, verbatim), `text_fitting.rs` (`clean`, `cw`, `width`, `fit`, `wrap`, verbatim, `pub(crate)`), `menu_side.rs` (`MenuSide`, its own file: a public enum, one file per type) and `menu_layout.rs` (`MenuLayout` with private fields,
`MenuLayoutParts` (the same six field names), accessors named as the fields; `side()` is `#[cfg(test)]`, no production reader: D7), `menu_layout_calculator.rs` (`layout`, the function name kept like Phase 6 kept function
names, its final `Some(Layout{..})` built through `MenuLayout::new(MenuLayoutParts{..})`), `menu_describer.rs` (`describe(&Menu) -> (String, String)`, the tuple kept: `desc_row` and `popout_rows` destructure it; D8 arms;
glossary row `MenuDescriber`). Left verbatim in the renderer (behaviour-safe, one caller each): the scroll-window clamp (`:414-418`), the pop-out side choice (`:430-435`, needs `POP_W`), the `autocomplete.width` read, and
`ghost_patch`'s clip loop.

### R20 Icons: ids in the domain, bytes in infrastructure (icons 4.1-4.12, renderer G4, cons K-I)
Taken literally the plan makes dropdown import terminal_session infrastructure (`png_slot` embeds `PNGS[i].1`, `badge_slot` calls `tinted` over `TEMPLATE_RGBA`). Resolution:
- `value_objects/icon_id.rs`: `IconId { Png(usize), Tinted(u8, u8, u8) }` (`Clone, Copy, Debug, PartialEq, Eq`), `value(self) -> u32` (`Png(i)` gives `i as u32 + 1`; `Tinted(r,g,b)` gives `0x0100_0000 | u32::from(r) << 16 | ..`, the text
  moved verbatim, P8-T32), `ICON_NAMES: &[&str]` (the 42 names of `icons.rs:19-60`, same order), `placeholders(id: u32)` and `color(id: u32)` as free fns on the raw number (verbatim; the renderer calls `color`).
- `value_objects/icon_slot.rs`: `IconSlot { text: String, image: Option<IconId>, badge: bool }`, private fields, `new(text, image, badge)`, `text() -> &str`, `image() -> Option<IconId>`, `is_badge() -> bool`.
- `value_objects/graphics_support.rs`: `GraphicsSupport` = `Graphics` renamed, derives and variants (`Off`, `Direct`, `Tmux`) and doc comments verbatim.
- `services/icon_resolver.rs`: `slot(&Suggestion, GraphicsSupport) -> IconSlot` and the private helpers verbatim (`wide_emoji`, `spec_image`, `type_image`, `ascii_slot`, `text`, `pad2`, `param`, `hex`); `png_slot` finds the index in
  `ICON_NAMES` (same `find`, same `.or_else(template)`, same `.expect`, the message pointing at the new list) and returns `IconId::Png(index)`; `badge_slot` returns `IconId::Tinted(r,g,b)` or `png_slot("template").image()`; no bytes.
- `terminal_session/infrastructure/adapters/icon_assets.rs`: `pngs!` (defined and invoked in this file, `include_bytes!` path `"../../../../assets/icons/"` + name, K10), `PNGS`, `TEMPLATE_RGBA`, **`tinted(r, g, b)` verbatim**
  (deviation from the plan's "tint maths in icon_resolver": it is pixel work, nothing in the domain needs pixels, and keeping it beside `TEMPLATE_RGBA` leaves `tint_multiplies_colour_into_template` byte-identical), and
  `pub(crate) fn kitty_image(icon: IconId) -> KittyImage` (`Png(i)` gives `KittyImage { id: icon.value(), src: KittyPixels::Png(PNGS[i].1) }`, `Tinted(r,g,b)` gives `KittyPixels::Rgba(tinted(r,g,b))`; explicit arms, no `_`).
- `terminal_session/infrastructure/adapters/kitty_graphics.rs`: `CHUNK`, `KittyImage { id, src }` and `KittyPixels { Png(&'static [u8]), Rgba(Vec<u8>) }` (was `Image`/`Source`; the field names `id`, `src` and the variant names
  stay, so `transmit_chunks` changes only its type names; the bare word `Source` is ghost-text vocabulary now; the two byte types share this file as the adapter's own parameter types, deviation 20), `transmit(&KittyImage, GraphicsSupport)`, `place(icon: IconId, p: usize)` (uses `icon.value()` where it used `img.id`),
  `CLEAR_PLACEMENTS`, `base64`, `tmux_graphics`, `detect() -> GraphicsSupport`. Rejected: keeping `Image`/`Source` (fewer CHANGED tests, but the glossary maps `icons::Image` to `IconId`).
- The renderer holds `Option<IconId>` in `Patch.img`, keeps `sent: HashSet<u32>` keyed on `icon.value()`, calls `icon_resolver::slot`, `icon_id::color(img.value())`, `kitty_graphics::{transmit, place, CLEAR_PLACEMENTS}` and
  `icon_assets::kitty_image(*img)`. Output is byte-identical: every number and string is produced by the same code.
- Two name lists (`ICON_NAMES` in the domain, `PNGS` in infrastructure) cannot be one: the macro needs string literals, `#[macro_export]`/`#[macro_use]` are banned (rule 8), a domain list cannot be const-iterated into
  `include_bytes!`. The guard `should_embed_the_png_assets_in_the_order_of_the_icon_ids` makes a drift a test failure; 8.4a adds a temporary form of it in `icons.rs` against the legacy `PNGS`.
- `icons::width` equals `render::width` for every input (the same per-char sum): 8.4b keeps a private verbatim copy in `icon_resolver.rs`, 8.3a deletes it in favour of `text_fitting::width` (one-line edit, provable by inspection).
- Rejected: S2 (a bridge `icons::image(IconId) -> Image` living between 8.3 and 8.5); `include_bytes!` inside dropdown's domain (embeds an asset in the domain); infra-assigned numeric ids with `IconId::Png(&'static str)` (moves `color`
  and `placeholders` out of the domain tests and splits two tests for no gain).

### R21 The overlay renderer (renderer G1-G6, G9, G12)
- `git mv src/render.rs src/terminal_session/infrastructure/adapters/overlay_renderer.rs` in 8.6, only after every pure item left (the diff of the move shows only paths and names). `Renderer` becomes `OverlayRenderer` (glossary row);
  `new`, `with_images(GraphicsSupport)`, `set_theme`, `take_clear`, `drop_clear`, `frame`, `full_repaint` are `pub(crate)`; `compose`, `menu_patches`, `rgb`, `pen`, `list_row`, `desc_row`, `popout_rows` stay private (the tests share the file).
- `FrameInput` stays in `overlay_renderer.rs` in Phase 8 (fields `pub(crate)`, no new field). Options: create `terminal_session/application/frame_input.rs` now (Phase 9.4's port file would only reference it) or leave it and move it
  in 9.4 (a second move, `pty.rs`'s import edited twice). Taken: leave it (a pure move; the move is one line in 9.4).
- `use crate::types::Model;` is the one legacy import of the new adapters (legal, `terminal_session` is unregistered); Phase 9 retargets it when `AlacrittyTerminalScreen` owns the `Term`; `types::Model` dies in 9.7.
  Rejected: a private `type Model = Term<VoidListener>` alias (an identical type under another name that Phase 9 must remove again).
- The test module stays inline. It was ~395 lines (136 helpers, ~250 tests) once the pure tests leave, but 8.0b's overlay hash test and its helpers add about 125, so 8.6 will hold about 530 to 555 lines, a little over section 1's "about 500" (measured at 8.6 and reported). Kept inline on purpose, recorded as deviation 24: 8.6 is a pure move with no restructuring, and Phase 9.4, which moves `FrameInput` and retargets `Model`, may slice it to `overlay_renderer/tests.rs`. The helper `input()` is unchanged (R7). `truncation_and_wrap_spinner` (the `SPINNER` half of
  `truncation_and_wrap`) lives with the renderer's tests.

### R22 Legacy code leaves with its last item; the shims
| Legacy code | Leaves in | How |
|---|---|---|
| `src/keys.rs` + `mod keys;` | 8.1b | its last items (decoder, `KeyEvent`, composite test) move; no shim existed (the importers were renamed in 8.1a) |
| `src/menu.rs` + `mod menu;` | 8.2b | `git mv` into the entity file; the plan's step 5 "delete `menu.rs`" is then already done |
| `src/icons.rs` + `mod icons;` | 8.4b | the atomic split |
| `impl Theme { from_settings, from_json }`, `SHADES`, `ACCENTS` in `render.rs` | 8.5d | the only temporary item of the phase (R13) |
| `cli.rs` theme section, `settings_repository()`, 2 tests | 8.5c | one commit with `bootstrap/cli.rs` |
| `src/render.rs` + `mod render;` | 8.6 | `git mv` into `overlay_renderer.rs` |
No `pub(crate) use` re-export anywhere in the phase (`grep -rn 'pub(crate) use' src/dropdown src/terminal_session src/pty.rs src/cli.rs` empty). `LEGACY_MODULES` keeps `icons keys menu render` (names stay after their files die).

### R23 Module root and `main.rs`
`src/dropdown/mod.rs` declares `domain` (8.1a), `application` (8.5a) and `infrastructure` (8.5b), children only (rule 6), alphabetical by convention. `main.rs` gains `mod dropdown;` in 8.1a (after `mod doctor;`) and loses `mod keys;` (8.1b), `mod menu;`
(8.2b), `mod icons;` (8.4b), `mod render;` (8.6). `terminal_session/domain/value_objects/mod.rs` gains `pub(crate) mod key_event;` (8.1b); `terminal_session/infrastructure/adapters/mod.rs` gains `stdin_key_decoder` (8.1b), `icon_assets` and `kitty_graphics`
(8.4b), `overlay_renderer` (8.6). `terminal_session/application/frame_input.rs` is NOT created (R21).

### R24 Registration and the fitness run
`"dropdown"` joins `REGISTERED_MODULES` in 8.7 (plan: at the end). Until then every step registers it **locally** (Phase 6 method) once `src/dropdown/` exists (8.1a; K13): add the name, `cargo test --locked architecture`
reports nothing, revert before committing (`git diff src/architecture_tests.rs` empty). Reviewers run the same check on a copy (`git archive <rev> | tar -x -C target/hexagonal-refactor/review-tmp/`). `terminal_session` stays unregistered until
Phase 9, so its new adapters and `KeyEvent` follow the layout but no rule reads them: hand greps stand in (R26). `LEGACY_MODULES` is unchanged.

### R25 New-test ownership
Every new name in "Test moves" has one owner step; each step runs `grep -rn "fn <name>(" src` (empty) before adding it (K21). Test-first for new seams (RED, GREEN, commit, REFACTOR, commit), `should_` names, AAA, no mocks (in-memory fakes and real
objects). Characterisation tests that must pass on the old code (8.0a, 8.0b) are written first and committed green.

### R26 Per-step rules (gate, lock, evidence)
Every step:
- **gate**: `scripts/hexagonal-refactor/gate.sh` prints `GATE PASS` before each commit; a lone `integrations_zsh_powerlevel10k_prompt_wrapped` failure is rerun alone (P7-T54);
- **test lock**: `python3 scripts/hexagonal-refactor/testlock.py . ef807c1 --diff` and `... . phase8-start --diff`: MISSING 0, DUPLICATE 0, every CHANGED in the step's list ("Test moves"); delete `scripts/hexagonal-refactor/__pycache__` afterwards;
- **diffs**: from 8.0a, `cargo build --locked`, then `theme-diff.sh` baseline vs `target/debug/tabsmith` prints nothing (mandatory at 8.5c and 8.5d, 8.7; cheap elsewhere); `integrations-diff.sh`, `precmd-diff.sh`, `inline-diff.sh` stay empty at the
  steps that touch `cli.rs`, `bootstrap/cli.rs` or `pty.rs` (8.1c, 8.2a, 8.5c, 8.5d, 8.6, 8.7);
- **moves**: by script or `git mv`; a block diff against `git show phase8-start:<file>` shows only paths, visibility, glossary names, accessor reads and the edits the step lists;
- **fitness**: the local registration of `dropdown` (R24), plus by hand `grep -rnE 'crate::(types|engine|parser|suggest|pty|menu|render|icons|doctor|cli|tracker|keys|shell_parser|js|spec|inline)\b' src/dropdown` (empty),
  `grep -rnE 'crate::(shell_integration|history|ghost_text|terminal_session|diagnostics|bootstrap)\b' src/dropdown` (empty), `grep -rnE 'dropdown::(application|infrastructure)' src/terminal_session` (empty outside `#[cfg(test)]`; `pty.rs`,
  legacy, may import `FileThemeRepository` as it imports `FileSessionContextRepository`), `grep -rnE 'completion::application::(command_line_parser|generator_runner|test_fixtures)' src --exclude-dir=completion` (empty), `grep -rn
  'crate::settings::infrastructure' src/dropdown` (empty outside tests), `grep -rn 'pub(crate) use' src/dropdown src/terminal_session` (empty);
- **cfg**: per touched file `grep -n 'cfg(windows)\|cfg(unix)\|cfg(not(\|cfg(target_os\|cfg(all(' ` returns the same lines as at `phase8-start` (the legacy areas have none); new chmod tests carry `#[cfg(unix)]`; `cargo check --locked --no-default-features --all-targets`;
- **clippy budget 3**: prune the imports a move leaves unused in the same commit; no new `new` over 7 parameters, no `Default` unit struct, no `match` with duplicated bodies;
- **commits**: `refactor(dropdown): <why>`, `test(dropdown): ...`, `chore(scripts): ...`, `docs: ...`, at most 72 characters, ending with the trailer `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (the model that writes the commit; agent-brief.md's Opus
  line is superseded), `git commit -F <file>` (the RTK hook rewrites words inside a quoted `-m`), measured length before committing; never rewrite history;
- **guard tests**, always green without edits to their assertions: the 12 menu tests, every `pty::tests::*` (they drive `Keymap`, `Menu` and the renderer through `Session`), the 11 render tests, `debug_shell_survives_closed_terminal` alone;
- **per run**: `cargo test --locked --release -- --ignored all_bundle_modules` prints `1 passed` at the end of each run.

### R27 Documentation only at the close
Glossary rows, the plan's section 4 layouts, section 5 status, section 8 smoke line, section 9 "Phase 8 notes", `README.md`, `handoff.md` and the stale comments are 8.7's work (as 7.10 did), except the comment repoints each step lists.
Stale comments: `settings_catalog.rs:295, 375, 441, 506, 519, 532` (`menu.rs`) are repointed in 8.2b to `dropdown/domain/entities/menu.rs`; `:305` (`render.rs: f64("autocomplete.width", 40.0)`) in 8.6 to `overlay_renderer.rs`'s `menu_patches`; `:545` (`render.rs:
string("autocomplete.theme")`) in 8.5d to `theme_resolver.rs`; `cli.rs:74` ("render.rs owns the colours") disappears with 8.5c's code; `pty.rs:978` in 8.6; `render.rs:2` and `menu.rs:2` die with their files; `docs/SPEC.md:499-501, 2121, 2262-2276, 2680` are Phase 11.

### R28 Hash guards are data
The byte-hash tests of 8.0b store 64-bit FNV-1a values generated by a throwaway `println!` run at `phase8-start` (the working tree equals the tag when 8.0b starts), pasted into the test, the print removed. They are new tests, not baseline
tests, so later steps may change HOW they build the scenario (new constructors, `now`) but never the hash values; a changed hash is a behaviour change.

### R29 No new crates; nothing else moves
No `Cargo.toml` edit. No change to `bootstrap/engine_*`, `doctor.rs`, `tracker.rs`, `types.rs` or `specs`/`ghost_text`/`history`/`shell_integration`. `pty.rs` changes only imports, renames, accessor reads, `Instant::now()` arguments, the theme resolution and
one new field (R15); `cli.rs` only loses code.

### R30 Open questions use defaults
See "Open questions": the defaults apply unless the user answers.

## Module map

Layers: **D** domain, **A** application, **I** infrastructure, **T** terminal_session. `pub(crate)` at most; domain fields private.

| File | Content | From (`7aafcef`) | Layer | Step |
|---|---|---|---|---|
| `src/dropdown/mod.rs`, `domain/mod.rs`, `domain/{value_objects,services}/mod.rs` | child declarations | new | | 8.1a (`entities` 8.2b, `repositories` and `dropdown_error` 8.5a, `application` 8.5a, `infrastructure` 8.5b) |
| `domain/value_objects/{key_code, modifiers, key, key_action, key_binding, keymap}.rs` | the key VOs, `Keymap` + `DEFAULTS` | `keys.rs:6-44, 268-327, 393-449` | D | 8.1a (private fields 8.1c) |
| `domain/services/keystroke_parser.rs` | `parse_keystroke`, `normalize` | `keys.rs:329-390` | D | 8.1a |
| `T domain/value_objects/key_event.rs` | `KeyEvent` | `keys.rs:46-51` | T | 8.1b (private fields 8.1c) |
| `T infrastructure/adapters/stdin_key_decoder.rs` | `StdinKeyDecoder`, `#[cfg(test)] decode`, the decode fns and consts | `keys.rs:53-266` | T | 8.1b |
| `domain/entities/menu.rs` (+ `menu/tests.rs`) | `Menu`, `max_rows`, `typing`, `same_item` | `menu.rs:47-468` | D | 8.2b |
| `domain/value_objects/{menu_visibility, menu_effect}.rs` | `MenuVisibility`, `MenuEffect` | `menu.rs:19-45` | D | 8.2b |
| `domain/value_objects/common_prefix_outcome.rs`, `domain/services/common_prefix.rs` | `CommonPrefixOutcome`, `decide`, `matching_type` | `menu.rs:83-89, 388-468` | D | 8.2c |
| `completion/domain/test_fixtures.rs` | + `item`, `comp`, `settings` | `menu.rs:479-537` | completion | 8.2a |
| `domain/value_objects/rgb.rs`, `domain/services/{color_parser, text_fitting}.rs` | `Rgb`, `parse_color`, `xterm256`, `clean cw width fit wrap` | `render.rs:22, 132-177, 825-888` | D | 8.3a |
| `domain/value_objects/theme.rs` | `Theme`, `ThemeParts`, `dark`, `light` | `render.rs:24-35, 46-71` | D | 8.3b |
| `domain/value_objects/{menu_side, menu_layout}.rs`, `domain/services/{menu_layout_calculator, menu_describer}.rs` | `MenuSide`; `MenuLayout`, `MenuLayoutParts`; `layout`; `describe` | `render.rs:599-676` | D | 8.3c |
| `domain/value_objects/{icon_id, icon_slot}.rs` | `IconId`, `ICON_NAMES`, `value`, `placeholders`, `color`, `IconSlot` | new + `icons.rs:164-178` | D | 8.4a (the moved fns 8.4b) |
| `domain/value_objects/graphics_support.rs`, `domain/services/icon_resolver.rs` | `GraphicsSupport`, `slot` and helpers | `icons.rs:89-99, 138-312` | D | 8.4b |
| `T infrastructure/adapters/{icon_assets, kitty_graphics}.rs` | `pngs!`, `PNGS`, `TEMPLATE_RGBA`, `tinted`, `kitty_image`; `CHUNK`, `KittyImage`, `KittyPixels`, `transmit`, `place`, `CLEAR_PLACEMENTS`, `base64`, `tmux_graphics`, `detect` | `icons.rs:11-136, 274-283, 314-374` | T | 8.4b |
| `domain/dropdown_error.rs` | `DropdownError` | new | D | 8.5a |
| `domain/value_objects/{theme_choice, theme_author, theme_definition}.rs` | R15, R14 | `cli.rs:65-84` | D | 8.5a |
| `domain/repositories/theme_repository.rs` | R14 + `InMemoryThemeRepository` | new | D | 8.5a (`find_palette` 8.5d) |
| `application/{select_theme, show_theme, list_themes}_use_case.rs`, `application/{theme_selection, current_theme}.rs` | R17 | `cli.rs:96-159` | A | 8.5a |
| `infrastructure/adapters/theme_json_mapper.rs` | strict entry (8.5b), lenient entry (8.5d) | `cli.rs:67-94`, `render.rs:96-128` | I | 8.5b, 8.5d |
| `infrastructure/adapters/theme_file_error.rs` | `ThemeFileError` (`Json`, `ColourNotText`; `Display`, `Debug`), what the strict entry returns | `cli.rs:67-94` (the `bail!` and the serde error) | I | 8.5b |
| `infrastructure/adapters/file_theme_repository.rs` | R18 | `cli.rs:137-159`, `render.rs:85-90` | I | 8.5b (`find_palette` 8.5d) |
| `infrastructure/cli/theme_command.rs`, `infrastructure/dropdown_module.rs` | R17 | `cli.rs:27-37, 96-175` | I | 8.5b |
| `domain/value_objects/theme_palette.rs`, `domain/services/theme_resolver.rs` | R15 | `render.rs:37-43, 73-128` | D | 8.5d |
| `T infrastructure/adapters/overlay_renderer.rs` | `OverlayRenderer`, `FrameInput`, the cell and SGR fns | `render.rs:179-597, 678-822` | T | 8.6 |

Allowed edges used: dropdown to shared (`DataDirectory`, `SharedDependencies`, `SharedError`, `terminal_style::{bold, stdout_tty}`), settings (`Settings`, `SettingValue`, `SettingsRepository`, `SettingsError`, the catalog keys), completion
(`Suggestion`, `CompletionResult`, `Insertion`, `InsertionParts`, `GeneratorKey`, `ArgumentHint`, `CompletionPhase`) and specs (`SuggestionKind`); terminal_session to dropdown domain items only. `anyhow`, `clap`, `serde`, `serde_json`, print macros only in
`infrastructure` (`anyhow` and `clap` only in `theme_command.rs` and the tests of `domain/dropdown_error.rs`, D2; `serde`/`serde_json` only in the mapper, `theme_command.rs` and tests); `unicode_width` is the only external crate in `domain` (`text_fitting`, `icon_resolver`).

Outside `src/dropdown/` and `src/terminal_session/`:

| File | Change | Step |
|---|---|---|
| `main.rs` | `+ mod dropdown;`; `- mod keys; mod menu; mod icons; mod render;` file by file | 8.1a, 8.1b, 8.2b, 8.4b, 8.6 |
| `pty.rs` (legacy) | imports, renames, accessors, `Instant::now()` arguments, `StdinKeyDecoder`, `OverlayRenderer`, `kitty_graphics::detect()`, `themes: Rc<dyn ThemeRepository>` + `resolve_theme` x3, the test `Session` literal | 8.1a-8.6 |
| `render.rs` (legacy) | imports, accessors, `IconId`/`GraphicsSupport`, the shim, then the move | 8.1a-8.6 |
| `completion/domain/test_fixtures.rs` | + `item`, `comp`, `settings` | 8.2a |
| `cli.rs` (legacy) | loses the theme section and 2 tests; `//!` drops `theme` | 8.5c |
| `bootstrap/cli.rs` | `Theme(ThemeArgs)`, `run_theme_command`, `dropdown_module()` | 8.5c |
| `settings_catalog.rs` | comment repoints (R27) | 8.2b, 8.5d, 8.6 |
| `architecture_tests.rs` | `"dropdown"` in `REGISTERED_MODULES` | 8.7 |
| `scripts/hexagonal-refactor/theme-diff.sh` | new | 8.0a |

## Deviations from the plan's letter (record them in plan section 9 at 8.7)

1. **Guards first.** `theme-diff.sh` (8.0a), three lenient-theme tests, two byte-hash tests (icons, overlay) and seven keymap/menu characterisation tests (8.0b) precede any move; the tag `phase8-start` is the lock baseline.
2. **Nineteen steps in three runs** instead of five steps: plan step 1 is 8.1a-8.1c, step 2 is 8.2a-8.2c, step 3 is 8.3a-8.3c and 8.4a-8.4b, step 4 is 8.5a-8.5d, step 5 is 8.6; `keys.rs`, `menu.rs`, `icons.rs` and `render.rs` are deleted file by file with
   their last item (8.1b, 8.2b, 8.4b, 8.6), not at the end.
3. **`KeyEvent`** lives in `terminal_session/domain/value_objects/key_event.rs`; `Decoder` is `StdinKeyDecoder`; `KeyCode` and `Modifiers` get their own files; `Mods`, `Action`, `Binding` take their glossary names.
4. **Private key fields** with `const fn` builders (8.1c), after the verbatim move.
5. **`keymap_defaults_and_overrides` is not D6-split**: it stays whole in the decoder's tests (R5); `Keymap` gets its tests from the 8.0b set.
6. **`KeystrokeParser` is two free functions**, not a struct; `normalize` lives there.
7. **`now`** is a parameter of `Menu::on_completion` and `Menu::loading`; the renderer and `Session::on_completion` still read `Instant::now()` (R7); `pty.rs:1242` passes `Instant::now()`, not the tick's `now`.
8. **`common_prefix`** is a service returning a new VO `CommonPrefixOutcome` (a plan silence); `Menu::common_prefix` applies it. D8 arms in `Menu::on_completion` and `handle`.
9. **`item`, `comp`, `settings`** go to `completion/domain/test_fixtures.rs`, `settings` with `#[cfg(test)]` on its `use` and fn.
10. **`max_rows`** stays in the `Menu` entity file; `Menu::vis()` is `#[cfg(test)]`.
11. **`Rgb` is a tuple alias**, the glossary row amended.
12. **`Theme` has private fields** with `ThemeParts` and eight accessors; `description_border` keeps its reader only in `ThemePalette::theme()`.
13. **A temporary shim** (`impl Theme { from_settings, from_json }` in `render.rs`) from 8.3b to 8.5d (R13).
14. **`ThemeRepository` has four methods** and two result types: strict `ThemeDefinition` (the author) and lenient `ThemePalette`; the glossary's `ThemeDefinition` row is amended; `theme_json_mapper` holds two entry points, not "one parser" (R14, R16, P8-T39).
15. **`ThemeChoice`** shares only the built-in NAME list with the CLI; `built_in("")` is `None`, `from_name("")` is `Dark`. **`resolve_theme`**, a domain service, takes the `ThemeRepository` trait object (the skill gives ports to use cases): the custom branch is the only reader of the theme file and three legacy call sites would otherwise carry the same glue (R15 b); a precedent for Phase 9, which may turn it into a use case.
16. **`describe`** becomes `MenuDescriber` (`menu_describer.rs`), a name the plan and glossary lack.
17. **Icons cross the layers as ids**: `IconId`, `IconSlot` and the 42-name `ICON_NAMES` in the domain; `PNGS`, `TEMPLATE_RGBA`, `kitty_image` and **`tinted`** in `icon_assets.rs` (the plan puts "tint maths" in `icon_resolver`); `placeholders` and `color` in `icon_id.rs`; the kitty
    byte types are `KittyImage`/`KittyPixels`; `tinted` now runs at transmit time, once per colour per session (performance only, P8-T35).
18. **`FrameInput` stays** in `overlay_renderer.rs`; no `now` field (R21).
19. **The theme CLI**: no use case for `--folder`; `ThemeSelection` and `CurrentTheme` DTOs (`ThemeSelection` holds `ThemeDefinition` and `ThemeChoice`, a DTO exception to "DTOs hold primitives" like `SettingValue` and Phase 7's `GhostTextSources`: the banner needs the author; `CurrentTheme` holds `SettingValue`); `DropdownError`; `SettingValue` to JSON copied (parity-tested); `announce` before the save.
20. **Files beyond section 4**: `key_code.rs`, `modifiers.rs`, `key_event.rs`, `common_prefix_outcome.rs`, `menu_side.rs`, `menu_describer.rs`, `theme_palette.rs`, `theme_author.rs`, `theme_selection.rs`, `current_theme.rs`, `dropdown_error.rs` content as listed, `entities/menu/tests.rs`, `infrastructure/adapters/theme_file_error.rs` (`ThemeFileError`, the strict entry's error, `pub(crate)` because the entry returns it). `KittyImage` and `KittyPixels` share `kitty_graphics.rs`, the adapter's own parameter types (precedent: `ShellArg`, `IntegrationPhaseArg` and `InitArgs` share `init_command.rs`; `JsArg` and `QuickJsSpecRuntime` share `quickjs_spec_runtime.rs`), not one file each.
21. **Stale comments repointed** in `settings_catalog.rs` (Phase 7 deviation 18 precedent).
22. **`dropdown` is registered at the end** (8.7); `LEGACY_MODULES` is unchanged.
23. **Section 5's step 5 text** ("delete `keys.rs`, `menu.rs`, `render.rs` and `icons.rs`") is edited in place to say they went with their last items.
24. **The renderer's test module stays inline** in `overlay_renderer.rs` at about 530 to 555 lines, over section 1's "about 500" for a sibling `tests.rs` (R21): 8.6 is a pure move; Phase 9.4 may slice it when `FrameInput` moves.
25. **A test-only `terminal_session` to `specs` edge**: the moved 8.0b hash test in `icon_assets.rs` (`icon_inputs`) imports `SuggestionKind` to build its rows, and the 8.6 renderer tests will do the same through `completion::domain::test_fixtures::item`. Plan section 2 does not list `specs` for `terminal_session`; the edge sits under `#[cfg(test)]` (D2), is production-clean, and nothing checks the section 2 graph. Phase 9, when `terminal_session` registers, either allows test-only edges explicitly or adds a section 2 edge check to `architecture_tests.rs`.

Plan text (8.7, in place): section 4 `dropdown` and `terminal_session` layouts as built (add `key_code`, `modifiers`, `key_event`, `common_prefix_outcome`, `menu_side`, `menu_describer`, `theme_palette`, `theme_author`, `icon_id` holding `ICON_NAMES`/`placeholders`/`color`, `kitty_graphics` holding
`CLEAR_PLACEMENTS`, `KittyImage` and `KittyPixels`, `icon_assets` holding `tinted` and `kitty_image`; `theme_json_mapper` "two entry points" with its error in `theme_file_error`); section 5 Phase 8 marked done with its research link; section 8 "Manual smoke" gains "after Phase 8: items 1-3, 5, 6 plus a custom theme and icons in a kitty,
ghostty and tmux pane" (the overlay is not covered by golden); section 9 "Phase 8 notes".

## Test moves

"path" means testlock's loose form shows no change (it strips `Type::` prefixes, so a rename confined to a type path is PATH-ONLY; a renamed struct-literal type such as `Mods { .. }` to `Modifiers { .. }` is not a path, so it is CHANGED, not PATH-ONLY; a body that is byte-identical after a move is neither); "CHANGED" is a justified restructuring: the same scenario and the same assertions. Judge by reading the diff.
Find tests by name; line numbers are `7aafcef` `#[test]` lines. Helpers outside `#[test]` fns are not locked.

| Test (baseline) | Now | Destination | Step | Kind |
|---|---|---|---|---|
| `action_ids` | `keys.rs:648` | `dropdown/domain/value_objects/key_action.rs` | 8.1a | path (`Action` to `KeyAction`) |
| `keystroke_grammar` | `keys.rs:626` | `dropdown/domain/services/keystroke_parser.rs` | 8.1a moves, CHANGED from 8.1a; 8.1c CHANGED again | CHANGED from 8.1a: `Mods { meta: true, ..NONE }` (`:641`) becomes `Modifiers { meta: true, ..NONE }` (not a path); CHANGED in 8.1c: it becomes `NONE.with_meta(true)`; copies `NONE/CTRL/ALT/SHIFT/k` |
| `keymap_defaults_and_overrides` | `keys.rs:673` | stays in `keys.rs` tests (8.1a, importing the new paths), then `stdin_key_decoder.rs` tests (8.1b) | 8.1b | path (`Binding`/`Action` to `KeyBinding`/`KeyAction`, `Keymap`/`Settings` by path); D6 deviation (R5); helper `settings(json)` copied |
| `decode_table` | `keys.rs:480` | `terminal_session/infrastructure/adapters/stdin_key_decoder.rs` | 8.1a renames in place (CHANGED from 8.1a), 8.1b moves; 8.1c CHANGED again | CHANGED from 8.1a: the two `Mods { ctrl: true, alt: true, ..NONE }` (`:501-506, :521-529`) become `Modifiers { .. }`; CHANGED in 8.1c: they become `CTRL.with_alt(true)`; helpers `NONE/CTRL/ALT/SHIFT/k/one` copied |
| `decode_multi_key_chunk`, `esc_not_last_and_double_esc` | `keys.rs:568, 584` | same | 8.1b; 8.1c CHANGED | CHANGED in 8.1c: `e.key.code` becomes `e.key().code()`, `e.bytes.clone()` becomes `e.bytes()`, `.bytes.end` becomes `.bytes().end` |
| `paste_across_chunks` | `keys.rs:605` | same | 8.1b (`Decoder::default()` to `StdinKeyDecoder::default()`); 8.1c CHANGED | CHANGED in 8.1c: accessors as above |
| `ghost_keys_and_words_keys` | `pty.rs:1679` | stays | CHANGED from 8.1a (its `Mods { .. }` literals become `Modifiers { .. }`), again in 8.1c | 8.1c: `Key { code, mods }` to `Key::new(..)`, `Mods { ctrl: true, ..Mods::default() }` to `Modifiers::none().with_ctrl(true)`; the same five `ghost_key` asserts |
| `enter_records_the_submitted_line` | `pty.rs:2230` | stays | 8.1a | path (`Action::InsertSelectedAndExecute` to `KeyAction`), `Effect::Insert` to `MenuEffect` in 8.2b |
| `typing_and_space`, `full_insertion_reopens_only_for_new_arg_or_retrigger`, `esc_hides_for_the_line`, `backspace_into_previous_token_and_large_change`, `hidden_until_keypress_refreshes_items`, `large_change_counts_skipped_requests`, `error_reset_and_loading`, `only_show_on_tab`, `sticky_selection_across_recompute`, `common_prefix`, `execute_and_record` | `menu.rs:558-1024` | `dropdown/domain/entities/menu/tests.rs` | 8.2a CHANGED in place; 8.2b moves | CHANGED in 8.2a: the new `now: Instant` argument of `on_completion` (48 call sites in the module, 5 inside `typed`, 1 inside `with`) and of `loading` (3 sites); `error_reset_and_loading` keeps its private `loading_since` write (R8). In 8.2b `Action::` to `KeyAction::`, `Vis::` to `MenuVisibility::`, `Effect::` to `MenuEffect::` (path) |
| `navigate_wrap_hide_history` | `menu.rs:768` | same | 8.2b | path (it calls `typed(&s)` and `handle` only) |
| helpers `item`, `comp`, `settings` | `menu.rs:479-537` | `completion/domain/test_fixtures.rs` | 8.2a | bodies byte-identical; `subs`, `typed`, `prefix_fx`, `with` stay private in `menu/tests.rs` |
| `colours_and_themes` | `render.rs:1375` | split in three (layers of one test): A `colours_and_themes` (`parse_color`, `xterm256`, L1376-1396) in `color_parser.rs`; B `colours_and_themes_theme_json` (L1398-1420, `Theme::from_json`) in `render.rs` (8.3a) then `theme_json_mapper.rs` tests (8.5d); C `colours_and_themes_theme_choice` (L1422-1435, `Theme::from_settings` over `settings(..)`) in `render.rs` (8.3a) then `theme_resolver.rs` tests (8.5d) | 8.3a, 8.5d | A CHANGED (loses B and C); B, C NEW names, every assert kept; B reads accessors from 8.3b, builds through the mapper and `ThemePalette::theme()` from 8.5d; C calls `resolve_theme(&settings, bg, &InMemoryThemeRepository::default())` from 8.5d |
| `truncation_and_wrap` | `render.rs:1272` | D6 split: `cw('…') == 1` and the rest keep the name in `text_fitting.rs`; `SPINNER.iter().all(\|&c\| cw(c) == 1)` is the new `truncation_and_wrap_spinner` in `render.rs`, then `overlay_renderer.rs` tests | 8.3a, 8.6 | CHANGED (the last assert `L1282` is two facts in two modules) |
| `layout_decisions` | `render.rs:1199` | `dropdown/domain/services/menu_layout_calculator.rs` tests | 8.3c | CHANGED: `Layout {..}` x4 (`:1204, 1215, 1228, 1259`) becomes `MenuLayout::new(MenuLayoutParts {..})` with the same values, `Side` to `MenuSide`, `l.n/list_top/desc_row/x0/side` to accessors (`side()` is `#[cfg(test)]`) |
| `round_trip_below_wide_edges_truecolor`, `round_trip_above_ghost_popout`, `dedupe_guards_and_clear`, `full_repaint_round_trip`, `row_contents`, `images_fill_the_icon_slot`, `tmux_images_are_placeholders` | `render.rs:1036-1357` | `overlay_renderer.rs` tests | 8.6 (edits earlier: 8.1a `KeyAction`, 8.4b `GraphicsSupport` and `kitty_graphics::CLEAR_PLACEMENTS`) | path; `row_contents` needs `Rgb` a tuple (R11) |
| `round_trip_arg_hint_spinner_and_wide_ghost` | `render.rs:1097` | `overlay_renderer.rs` tests | 8.2a CHANGED in place; 8.6 moves | CHANGED: `m.on_completion(c, &s, Instant::now())` (`:1102`); the "no spinner before 200 ms" assertion is unchanged |
| `base64_matches_rfc4648` | `icons.rs:381` | `kitty_graphics.rs` tests | 8.4b | unchanged (byte-identical body; neither CHANGED nor PATH-ONLY) |
| `transmit_chunks` | `icons.rs:389` | `kitty_graphics.rs` tests | 8.4b | CHANGED (type names): `Image { id, src: Source::Png/Rgba(..) }` becomes `KittyImage { id, src: KittyPixels::.. }`, `Graphics::Direct` becomes `GraphicsSupport::Direct`; same bytes `b"Man"`, `vec![0; 4000]`, same assertions |
| `tint_multiplies_colour_into_template`, `assets_are_pngs` | `icons.rs:417, 592` | `icon_assets.rs` tests | 8.4b | unchanged: byte-identical bodies (`tinted` stays verbatim beside `TEMPLATE_RGBA`, R20), so testlock lists them as neither CHANGED nor PATH-ONLY |
| `resolves_like_q` | `icons.rs:438` | `icon_resolver.rs` tests | 8.4b | CHANGED: `.text` to `.text()`, `.image` to `.image()`, `.badge` to `.is_badge()`; `PNGS.iter().position(\|(n, _)\| *n == "git").map(\|i\| i as u32 + 1)` becomes the same over `ICON_NAMES` (`.position(\|n\| *n == "git")`); `s.image.as_ref().map(\|i\| i.id)` becomes `s.image().map(IconId::value)`; `matches!(s.image, Some(Image { src: Source::Rgba(_), .. }))` becomes `matches!(s.image(), Some(IconId::Tinted(..)))`; `width` from `text_fitting` or the private copy; fixture `item` from `completion::domain::test_fixtures` |
| `tmux_uses_placeholders` | `icons.rs:540` | D6 split: L542-562 (slot in Tmux, `id < 256`, 2 placeholder cells, `width == 2`, `color(id)`, tinted exact text, badge stays text) keep the name in `icon_resolver.rs`; L563-589 (Tmux `transmit` of `b"Man"` with the exact wrapped string, the six `tmux_graphics` cases) are the new `tmux_uses_placeholders_passthrough` in `kitty_graphics.rs` | 8.4b | CHANGED (both halves): accessors, `color` from `icon_id`, type renames |
| `theme_list_order` | `cli.rs:464` | D6 split: `file_theme_repository.rs::theme_list_order` keeps the name (the same files under `<temp>/themes` through `DataDirectory::new(Some(temp), None)`, asserts `list()` = `[alpha, dark, zed]`, non-json out, sorted; the missing-folder half asserts an empty list; the `remove_dir_all` line stays); `list_themes_use_case.rs::theme_list_order_built_ins` over `InMemoryThemeRepository` with `[alpha, dark, zed]` and with none asserts `[dark light system alpha zed]` and the three built-ins | 8.5c | CHANGED (the union of assertions is the original's) |
| `theme_file_parsing` | `cli.rs:477` | D6 split: `theme_json_mapper.rs::theme_file_parsing` keeps the name (parse OK, `{"author":"Ann"}`, `{"shade3":5}`, `not json` are errors); `theme_command.rs::theme_file_parsing_banner` holds the two banner asserts, arranging the definition through `parse_theme_definition` from the same two JSON texts, emoji bytes copied by script | 8.5c | CHANGED |

New tests (each has one owner step; `grep -rn "fn <name>(" src` empty first, K21; test-first, `should_` names, AAA):

| Step | File | Tests |
|---|---|---|
| 8.0a | `render.rs` (characterise `Theme::from_json`; they pass on the old code, then move in 8.5d) | `should_draw_a_json_value_that_is_not_a_fig_theme_with_the_base16_filler` (`{}`, `[1]`, `"x"`, `null`, `5`, `{"version":"1.0"}`, `{"version":1,"theme":{}}`, `{"version":"1.0","theme":"x"}`: all give the filler bg (24,24,24)), `should_fall_back_per_colour_for_a_fig_theme_with_missing_or_unparsable_colours`, `should_fall_back_per_colour_for_a_base16_theme_with_missing_or_unparsable_colours` |
| 8.0b | `icons.rs` | `should_render_every_icon_exactly_as_at_the_phase_8_start` (R28; see the guard section) |
| 8.0b | `render.rs` | `should_draw_the_overlay_bytes_it_drew_at_the_phase_8_start` |
| 8.0b | `keys.rs` (they move with their code in 8.1a) | `should_let_the_lexicographically_last_user_binding_win`, `should_parse_the_plus_key_and_reject_a_keystroke_without_a_key`, `should_ignore_an_unknown_modifier_and_bind_the_plain_key`, `should_read_the_select_suggestion_number_as_a_u8`, `should_lowercase_only_ctrl_and_alt_letters_when_binding_and_looking_up` (also `autocomplete.ignore` is `Unknown`, `ctrl+shift+k` is `k`+ctrl, a non-string value is skipped) |
| 8.0b | `menu.rs` (move with `Menu`) | `should_insert_a_lone_special_row_in_full`, `should_not_insert_an_auto_execute_row_in_full_on_a_shared_prefix` |
| 8.1c | `modifiers.rs`, `key.rs`, `key_event.rs` | `should_set_one_modifier_without_touching_the_others`, `should_keep_the_modifiers_of_a_key_when_adding_alt`, `should_hold_the_key_and_the_byte_range_of_its_chunk` |
| 8.2a | `menu.rs` (then `menu/tests.rs`) | `should_not_report_loading_at_exactly_200_ms_and_report_it_after`, `should_keep_the_first_loading_start_across_consecutive_loading_replies`, `should_stop_loading_on_the_first_reply_that_is_not_loading` (a `t0` and `t0 + 200 ms`, no real time) |
| 8.2c | `common_prefix.rs` | `should_decide_a_full_insertion_for_a_lone_row_whatever_its_kind`, `should_fail_for_an_auto_execute_row_without_an_original_kind`, `should_keep_the_original_case_of_the_selected_name_in_the_shared_prefix`, `should_escape_every_whitespace_character_of_the_shared_prefix`, `should_fail_when_the_shared_prefix_needs_quoting` |
| 8.3a | `render.rs` (the D6 and layer splits) | `truncation_and_wrap_spinner`, `colours_and_themes_theme_json`, `colours_and_themes_theme_choice` |
| 8.3b | `theme.rs` | `should_read_every_colour_of_a_theme_through_its_accessor` (dark and light literals of P8-T26) |
| 8.3c | `menu_describer.rs` | `should_label_nothing_for_the_own_description_of_the_selected_suggestion`, `should_treat_a_blank_description_as_missing_and_name_a_file_or_folder`, `should_describe_the_argument_hint_when_nothing_is_selected` |
| 8.4a | `icon_id.rs`, `icon_slot.rs`, `icons.rs` | `should_number_png_icons_from_one_and_tinted_icons_with_the_top_byte_set` (`Png(0)` 1, `Png(38)` 39 template, `Png(33)` 34 option, `Png(22)` 23 git, `Tinted(0x2e,0xcc,0x71)` `0x012e_cc71`), `should_hold_the_text_image_and_badge_of_an_icon_slot`, `should_list_the_icon_names_in_the_order_of_the_png_assets` (temporary, against legacy `PNGS`; 8.4b folds it into the next row) |
| 8.4b | `icon_assets.rs`, `kitty_graphics.rs` | `should_embed_the_png_assets_in_the_order_of_the_icon_ids` (`PNGS[i].0 == ICON_NAMES[i]`, equal lengths, bytes equal `assets/icons/<name>.png`), `should_transmit_a_png_icon_with_its_embedded_bytes_under_its_id`, `should_transmit_a_tinted_icon_as_32_by_32_rgba` (id computed through `IconId::value`), plus the split `tmux_uses_placeholders_passthrough` |
| 8.5a | `dropdown_error.rs` | `should_print_a_theme_settings_failure_exactly_as_the_settings_error`, `should_continue_the_source_chain_of_a_settings_failure`, `should_print_an_invalid_theme_file_and_a_missing_theme_as_the_cli_did`, `should_print_a_missing_home_exactly_as_the_shared_error` (the `DataDirectory` arm) |
| 8.5a | `theme_author.rs`, `theme_definition.rs` | `should_hold_the_optional_parts_of_a_theme_author`, `should_hold_the_author_of_a_theme_file_when_it_names_one` (every accessor is born from a test) |
| 8.5a | `theme_choice.rs` | `should_list_the_built_in_theme_names_in_order`, `should_read_an_empty_name_as_dark_for_the_renderer`, `should_not_take_an_empty_name_as_a_built_in_for_the_cli` |
| 8.5a | `theme_repository.rs` | `should_answer_the_definition_it_was_given_for_a_name`, `should_list_the_names_it_was_given_in_sorted_order` |
| 8.5a | `select_theme_use_case.rs` | `should_select_a_built_in_theme_and_keep_the_other_settings`, `should_select_a_custom_theme_before_a_built_in_of_the_same_name`, `should_not_take_an_empty_name_as_a_built_in`, `should_refuse_an_unknown_theme_naming_the_folder`, `should_refuse_an_invalid_theme_file_and_save_nothing`, `should_load_the_settings_before_it_looks_for_the_theme` (`UnreadableSettingsRepository` + unknown name gives the settings error, `save_attempted()` false), `should_announce_the_selection_before_it_saves` (inspect `load()` inside the closure), `should_save_the_name_as_typed`, `should_report_a_save_failure_after_announcing` |
| 8.5a | `show_theme_use_case.rs` | `should_show_the_default_theme_when_none_is_set`, `should_show_a_text_theme_raw_including_an_empty_one`, `should_show_a_value_that_is_not_text_unchanged` (`null`, number, list, object), `should_fail_to_show_over_settings_that_cannot_be_loaded` |
| 8.5a | `list_themes_use_case.rs` | `should_list_the_built_in_themes_first_then_the_custom_ones`, `should_not_list_a_custom_theme_named_like_a_built_in_twice`, `should_list_only_the_built_in_themes_when_there_are_no_custom_ones` |
| 8.5b | `theme_json_mapper.rs` (strict; the serde texts come from running the baseline binary on the D group of `theme-diff.sh`, never retyped) | `should_parse_a_full_author_and_a_name_only_author`, `should_read_a_three_item_author_array_positionally`, `should_report_the_serde_text_of_a_theme_file_that_is_not_an_object`, `should_report_the_serde_text_of_a_bad_author`, `should_report_the_serde_text_of_a_version_a_theme_or_a_duplicate_author`, `should_report_the_first_colour_key_that_is_not_a_string` (`accent0` before `shade9`, `shadeX`, `null`), `should_report_broken_json_like_serde_does` (empty, `not json`, trailing text, BOM), `should_accept_unknown_keys_and_unparsable_colours` |
| 8.5b | `file_theme_repository.rs` | `should_find_a_valid_theme_file`, `should_find_nothing_for_a_missing_file_or_a_directory_named_like_a_theme`, `should_find_nothing_for_a_file_that_is_not_utf8`, `should_report_an_invalid_theme_file_with_its_path`, `should_find_nothing_without_a_home`, `should_fail_to_name_the_folder_without_a_home`, `should_list_the_json_stems_sorted_and_deduped`, `should_not_create_the_themes_directory`, `should_join_the_name_unsanitised` (and `#[cfg(unix)]` `should_find_nothing_for_an_unreadable_file`) |
| 8.5b | `theme_command.rs` | `should_print_a_non_text_setting_value_like_the_settings_command` (literal `5`, `5.0`, `-3`, `null`, pretty list and object), `should_convert_every_setting_value_like_the_settings_json_adapter` (parity corpus), `should_write_the_author_lines_of_the_banner`, `should_write_only_the_theme_name_when_the_theme_has_no_author`, `should_bold_the_theme_name_and_the_author_name_on_a_terminal` |
| 8.5b | `dropdown_module.rs` | `should_select_a_custom_theme_from_the_data_directory_it_was_built_with` (a temp data directory, an in-memory settings repository kept by the test) |
| 8.5c | the two D6 halves | `theme_list_order_built_ins`, `theme_file_parsing_banner` |
| 8.5d | `theme_palette.rs`, `theme_resolver.rs`, `theme_json_mapper.rs`, `file_theme_repository.rs`, `theme_repository.rs` | `should_fall_back_per_role_of_a_fig_palette_to_the_dark_theme`, `should_fall_back_per_key_of_a_base16_palette_to_the_filler`, `should_build_the_dark_filler_theme_from_an_empty_palette` (the (16,16,16), (40,40,40), (18,52,86), (232,232,232) asserts of P8-T26); resolver: `should_resolve_the_built_in_names_without_asking_the_repository` (a recording `InMemoryThemeRepository`), `should_resolve_an_empty_name_to_dark`, `should_resolve_system_by_the_luma_of_the_terminal_background` (inclusive 0.5, none is dark), `should_resolve_a_custom_theme_through_its_palette`, `should_resolve_a_missing_custom_theme_to_dark`, `should_resolve_a_custom_theme_named_like_a_built_in_to_the_built_in`; mapper lenient: `should_map_a_fig_theme_to_its_roles` and the two characterisation tests of 8.0a moved path-only; `theme_repository.rs`: `should_answer_the_palette_it_was_given_for_a_name`; repository: `should_find_a_palette_for_valid_json_that_is_not_a_theme`, `should_find_no_palette_for_invalid_json`, `should_find_no_palette_without_a_home`, `should_read_the_file_again_on_every_call` |

About 104 NEW names in all (the sum of the new-test column of "Per-step guards" below, which this table lists name by name; a brief's own list wins where they differ), against `phase8-start`. The pass count only grows: 986 plus the NEW names, minus none.

Expected CHANGED against `phase8-start` over the phase (each step's list is the table above): `keystroke_grammar`, `decode_table`, `ghost_keys_and_words_keys` (first CHANGED at 8.1a by the `Mods {..}` to `Modifiers {..}` literal rename, edited again by 8.1c), `decode_multi_key_chunk`, `esc_not_last_and_double_esc`, `paste_across_chunks` (8.1c); the 11 menu tests with `now` (8.2a);
`round_trip_arg_hint_spinner_and_wide_ghost` (8.2a); `colours_and_themes`, `truncation_and_wrap` (8.3a); `layout_decisions` (8.3c); `resolves_like_q`, `tmux_uses_placeholders`, `transmit_chunks` (8.4b); `theme_list_order`, `theme_file_parsing` (8.5c) = 26, each a restructuring.
MISSING 0 and DUPLICATE 0 throughout; `action_ids`, `keymap_defaults_and_overrides`, `navigate_wrap_hide_history`, the seven renderer tests are path-only (those already CHANGED against `ef807c1`, such as `row_contents`, stay CHANGED there); `tint_multiplies_colour_into_template` and `assets_are_pngs` move with identical bodies (testlock lists them as neither CHANGED nor PATH-ONLY). Against `ef807c1` the CHANGED count of Phase 7 (154) grows by these, except tests already CHANGED there.

Guards that must stay green without assertion edits: every `pty::tests::*`, notably `ghost_keys_and_words_keys`, `split_paste_is_never_intercepted`, `disabled_menu_lets_tab_through`, `insertions_build_on_keys_not_yet_echoed`,
`keys_after_an_unmodelled_key_wait_for_its_echo`, `unbracketed_paste_is_not_intercepted`, `panic_while_handling_keys_still_forwards_them`, `enter_records_the_submitted_line`, `probe_replies`; `debug_shell_survives_closed_terminal` alone;
the settings catalog's per-key default tests for the menu, width and theme keys (`settings_catalog.rs:296, 376, 442, 507, 520, 533`).

## Output-equivalence guard

The golden capture covers `theme --help`, a few `theme` forms and no overlay at all; keys, menu, renderer and icons have no CLI surface. The nets are: **`theme-diff.sh`** (the theme command), the **byte-hash tests** (icons, overlay), the **characterisation tests**, the
existing pty and render round-trip tests, and a manual smoke at the end. Per step, once 8.0a has committed the script:
```
cargo build --locked
diff <(scripts/hexagonal-refactor/theme-diff.sh target/hexagonal-refactor/baseline-tree/target/debug/tabsmith) \
     <(scripts/hexagonal-refactor/theme-diff.sh target/debug/tabsmith)        # prints nothing
scripts/hexagonal-refactor/gate.sh                                              # GATE PASS (golden included)
python3 scripts/hexagonal-refactor/testlock.py . ef807c1 --diff
python3 scripts/hexagonal-refactor/testlock.py . phase8-start --diff
```
(also `integrations-diff.sh`, `precmd-diff.sh`, `inline-diff.sh` at the steps listed in R26). The baseline binary is `target/hexagonal-refactor/baseline-tree/target/debug/tabsmith` (`af1b304`). 8.0a also proves the script deterministic (two runs on the baseline binary
diff empty) and that the baseline and the `phase8-start` build agree (diff empty).

### `scripts/hexagonal-refactor/theme-diff.sh <binary>` (8.0a)

Style of `inline-diff.sh` and `integrations-diff.sh`: bash, macOS, `fresh` sandbox per group (`mktemp -d`, `pwd -P`, `mkdir -p $SB/home/.tabsmith`, `cd $SB`), `env -i PATH=$PATH HOME=$SB/home TABSMITH_HOME=$SB/home/.tabsmith USERPROFILE=$SB/home` plus the
group's overrides, stdin from `/dev/null`, `trap` cleanup (`chmod -R u+rwx` before `rm -rf`, so the chmod scenarios do not leak). Helpers: `put` (settings.json holds exactly these bytes), `theme_file <name> <bytes>` (writes `$SB/home/.tabsmith/themes/<name>.json`),
`run [VAR=value ...] -- <args>` printing `### <cmd>`, stdout, `--- stderr`, stderr, `--- exit N`, then `--- settings.json` (bytes, a `(no newline at end)` marker, `stat -f %Lp` mode, or `(none)`) and `--- ls -A` of the data directory (sorted; proves nothing is created lazily).
Scrubbing: `$SB` to `$SANDBOX`, the binary path to `$BIN`, its directory to `$BINDIR` (so the relative `TABSMITH_HOME` form prints `rel/themes`, and clap usage lines name `tabsmith` whatever the path). No clock or pid appears in any output. Groups (each scenario of the researcher's A-I list):
- **A list**: the folder missing / empty / holding `zed alpha dark light system notes.txt UP.JSON x.json.bak .hidden.json "a b.json" a.b.json é.json 日本.json dirtheme.json/` (a directory) `noext` and a non-UTF-8 name (`printf '\xff.json'`) / `themes` is a regular file / a symlink
  to a directory / `chmod 000` (skipped as root: `[ "$(id -u)" != 0 ]`) / a relative `TABSMITH_HOME` / over a corrupted settings.json / no home (`HOME="" TABSMITH_HOME="" USERPROFILE=""`).
- **B folder**: first run (creates, prints), second run, `themes` is a regular file (`File exists (os error 17)`), relative home, no home, over `{oops`.
- **C custom valid**: full author, the 3-item array author `["Ann","@t","gh"]`, name only, twitter only, github only, `author:{}`, `author:null`, no author, `version:null,theme:null`, `{}`, the version/theme/shade mix, with other settings present (kept), `sub/x`, `../up`, `日本`,
  `""` with a `.json` file; each followed by the saved settings.json bytes and mode.
- **D custom invalid**, each error text and settings.json untouched, no banner: `not json`, an empty file, `{} x`, a BOM, `[1]`, `"x"`, `null`, `5`, `{"author":"Ann"}`, `{"author":{"name":5}}`, `{"author":{"twitter":true}}`, `{"author":5}`, `{"author":["Ann"]}`, `{"author":[]}`, a 4-item author
  array, `{"version":1}`, `{"version":["x"]}`, `{"theme":"x"}`, `{"theme":[1]}`, a duplicate `author`, `{"shade3":5}`, `{"shade9":null,"accent1":1,"accent0":2}` (reports `accent0`), `{"shadeX":true}`, `{"accent":1}`.
- **E built-ins and collisions**: `dark light system` with no file, then with a valid, an invalid, an unreadable (`chmod 000`) and a directory-named `dark.json`; `Dark`, `dark ` (trailing space), an unknown name, `""` without and with a `.json` file.
- **F settings**: corrupted, `[1]`, a directory, missing, blank, each for NAME valid / built-in / unknown / `--list` / `--folder`; an unwritable data directory (banner on stdout, then `error: Permission denied (os error 13)`, built-in and custom); `TABSMITH_SETTINGS_PATH` with a home (themes still
  under the home) and alone (no home).
- **G show**: unset, `"light"`, `""`, `"a\nb"`, `null`, `5`, `5.0`, `-3`, `true`, `["a",1]`, `{"b":1,"a":[1,2]}`, `{}`, `[]`, a big float, corrupted, no home.
- **H tty** (a python pty driver as in `inline-diff.sh`, stdout on the pty; output piped through `cat -v`): a custom theme with a full author, a built-in, `--list`, show; the bold escapes wrap the name and author name only; the error text is unchanged.
- **I clap**: `--list --folder`, `--list X`, `--folder X`, `--list --list`, `a b`, `theme -x`.
`docs/hexagonal-refactor/README.md` ("Tools") gains `theme-diff.sh` in 8.7.

### Unit-test guards added BEFORE the move (8.0a, 8.0b; R28)
- **Lenient theme** (8.0a): the three `Theme::from_json` tests of "Test moves", over `serde_json::json!` values (the strict side has no unit test before the move: the D group above is its guard, and 8.5b writes its tests against the new mapper with texts copied from the script's output).
- **Icons** (8.0b), `should_render_every_icon_exactly_as_at_the_phase_8_start`: a local `fnv1a(&[u8]) -> u64`; for each of the 42 `PNGS` names and four templates (the `Special` and `Shortcut` type defaults, tinted `(0x62,0x8d,0xad)` and `(0x34,0x98,0xdb)` with a badge; `fig://template?color=2ecc71`, a tint with no badge; `fig://template?badge=ab`, an untinted template with a badge), build `item("x", kind, "").with_icon(Some(..))` (`fig://icon?type=<name>`, `fig://template?color=<hex>&badge=..`), for `Graphics::Direct` and
  `Graphics::Tmux` call `slot(..)` and hash `format!("{:?}|{}|{}", text, badge, image id)` followed by the `transmit(&image, mode)` string (46 inputs, 92 rows with both modes; correction C1 of 8.0b added the last two templates so that `placeholders`' top-byte diacritic and `badge_slot(None)` are guarded, the researcher's design had 90 rows and three badge tints); compare the 92 values with a `const EXPECTED: &[(&str, &str, u64)]` of 92 rows. Generate the table once with a throwaway `println!` at the tag, paste, remove the print. 8.4b
  rewrites only HOW each value is produced (`slot` returns `IconId`; bytes from `icon_assets::kitty_image`), never a value.
- **Overlay** (8.0b), `should_draw_the_overlay_bytes_it_drew_at_the_phase_8_start`, in `render.rs` tests, with the existing helpers (`model`, `menu`, `subs`, `input`): FNV-1a of `take_clear() ++ frame(..)` for about nine scenarios: below, truecolor, dark, selection 0; selection 2 with `autocomplete.height: 2` (scrolled); above with ghost and pop-out; arg hint only (no items); the light theme without
  truecolor; a base16 custom theme from `Theme::from_json`; a Direct icon slot and a Tmux placeholder slot; a wide-character edge; a description row with a long description (wrap and `…`); no scenario depends on time (no spinner). It is the only test that reads every theme colour through its accessor (8.3b), every layout decision (8.3c) and the clear bytes (8.6).
- **Keymap and menu** (8.0b): the seven characterisation tests of "Test moves".

### Manual smoke (the user's, after 8.7; plan section 8)
`TABSMITH_HOME=$(mktemp -d) cargo run -- debug shell zsh` (and bash, fish): (1) `git ch` shows the menu; (2) Tab inserts and branches appear, a shared prefix inserts; (3) Esc hides it; (5) a prompt on the bottom row draws the menu above; (6) a resize leaves no artefacts; icons in kitty or ghostty
(direct placements) and in tmux with `allow-passthrough on` (placeholders); `tabsmith theme light`, a custom theme file under `$TABSMITH_HOME/themes/`, and the `system` theme in a light terminal show the right colours; `theme` with no argument prints the saved value; and two checks of this phase that are not plan items (the plan's own 7 and 8 are `vim` and `exit`, which Phase 8 does not touch): (P8-S1) the spinner: a slow generator (a spec whose generator runs `sleep 1`, saved as `$TABSMITH_HOME/specs/user/<name>.js`, SPEC section 6.3) shows the spinner on the description bar after about 200 ms, and it stops when the reply arrives (the `loading(now)` and `on_completion` call sites of 8.2a have no automated test beyond the `Menu` predicate); (P8-S2) the two theme moments no test reaches: with a session running, `TABSMITH_HOME=<the same dir> tabsmith theme light` and back to `dark` from a second terminal recolours the menu (the settings reload, `poll_files`); with `autocomplete.theme` set to `system`, in a terminal whose OSC 11 background reply arrives after the startup timeout (a slow ssh session), the theme follows the late reply (`on_input`'s late-probe arm).

## Hand-off notes for Phase 9

What Phase 8 leaves in the legacy files and for the `terminal_session` adapters.
- **`pty.rs` after 8.7.** Imports: `dropdown::domain::value_objects::{key::Key, key_action::KeyAction, key_binding::KeyBinding, key_code::KeyCode, keymap::Keymap, modifiers::Modifiers, menu_effect::MenuEffect, theme::Theme}`, `dropdown::domain::entities::menu::Menu`,
  `dropdown::domain::repositories::theme_repository::ThemeRepository`, `dropdown::domain::services::theme_resolver::resolve_theme`, `dropdown::infrastructure::adapters::file_theme_repository::FileThemeRepository` (legacy may import infrastructure),
  `terminal_session::infrastructure::adapters::{stdin_key_decoder::StdinKeyDecoder, overlay_renderer::{FrameInput, OverlayRenderer}, kitty_graphics::detect}`. Fields: `keymap: Keymap`, `decoder: StdinKeyDecoder`, `renderer: OverlayRenderer`, `themes: Rc<dyn ThemeRepository>` (built in `Session::start` from
  `data_directory_from_env()` before the renderer, `InMemoryThemeRepository` in the test `Session` literal). Untouched logic: `ghost_key` (`:186`), `is_ctrl` (`:244`) and `note_forwarded` (`:938`) read `key.code()`/`key.modifiers()`; `on_keys` (`:1035-1111`) still decodes once per call, holds raw bytes, resets the decoder by assignment (P8-T9);
  the effect loop (`:1117-1135`) matches `MenuEffect`; `menu.on_completion(c, &settings, Instant::now())` (`:891`); `menu.loading(Instant::now())` (`:1242`); `frame()` builds `FrameInput` (`:994-1000`, no clock field); `resolve_theme(&settings, terminal_bg, &*self.themes)` at the three moments (`:451`, `:1021`, `:1284`); `drawable()`'s comment (`:978`) names `overlay_renderer.rs`.
- **`KeyRouter`** (a terminal_session domain service, allowed to use dropdown and ghost_text) takes `ghost_key`, `is_ctrl`, `note_forwarded`'s key rule and the `on_keys` arms: D8 explicit arms over `Option<KeyBinding>` keeping P8-T10's guard order (`Action if intercepting`, `Unknown if intercepting`, `_ if tab_show`, `Show/Toggle if global`, then the rest; `Ignore`
  has no arm of its own). `typed` counts only plain `Char` (`KeyCode::{Paste, Unknown, CursorReport}` stay variants of one `KeyCode`).
- **The decoder** (keys PG10): `StdinKeyDecoder` (`decode(&mut self, &[u8]) -> Vec<KeyEvent>`, `Default`) is pure std; `KeyEvent` is already a domain VO. Phase 9 picks one: a `KeyDecoder` port in `terminal_session/application/ports/` implemented by `StdinKeyDecoder` (plus a reset to default), or a domain service (one `git mv`). Either way `TerminalSession`
  owns the decoder and the reset replaces `self.decoder = StdinKeyDecoder::default()`. Three non-stdin use cases replay held raw bytes through `on_keys` (`release_held` from `on_output`, `on_completion`, `on_timers`), so decoding once per `Stdin` chunk in the driver would double-decode held bytes and desync the paste state. `#[cfg(test)] tests::maybe_panic()` stays at the
  driver's call site, first in the per-event loop.
- **`HandleKeystrokesUseCase`** interprets `MenuEffect`: `Insert { ins, full, record }` (`full` is read only by tests), `Write` (also sets `tracker.submitted` for `\r`), `Bell`, `Rerequest`, `ToggleFuzzy`, in the order of `pty.rs:1117-1135`; it imports `MenuEffect` from dropdown's domain.
- **Clocks.** `Menu` takes `now`; Phase 9's use cases get it from `shared::application::ports::clock::Clock::now_instant()` (one `now` per tick). `OverlayRenderer::menu_patches` and `Session::on_completion` still read `Instant::now()`: when the screen adapter gets a `Clock`, add `now: Instant` to `FrameInput` (the render test helper `input()` and the pty call site change) and pass it to `menu.loading(input.now)`.
- **`TerminalScreen` port** (plan 9.4) lists `draw_overlay`, `take_clear`, `full_repaint`; `pty.rs` also needs `set_theme` (`:1021`, `:1284`), `drop_clear` (`:1199`) and `with_images(kitty_graphics::detect())` at construction (`:451-452`): add them. `AlacrittyTerminalScreen` owns the `Term` and ONE `OverlayRenderer` built once per session (`sent` must survive theme changes and resizes; `set_theme` must stay
  unconditional, P8-T25); `overlay_renderer.rs`'s only legacy import is `crate::types::Model`, retargeted then (`types::Model` dies in 9.7). `FrameInput` moves to `terminal_session/application/` when the port references it. `RenderGuard` must equal `compose`'s guards (`overlay_renderer.rs`) and `pty.rs` `drawable` (`:979-990`) exactly.
- **Theme injection.** `DropdownModule::themes: Rc<dyn ThemeRepository>` is what `terminal_session` receives from bootstrap (plan section 2's bridge row); the three `resolve_theme` call sites become `ApplyTerminalBackground...`/settings-reload use cases. `Rc` is enough only while the session stays on one thread (it already holds `Rc<dyn CompletionEngine>`; O5). The resolver takes the terminal
  background as `Option<Rgb>` (`pty.rs:248`'s own `type Rgb` and `parse_osc11`/`probe_replies` stay tuples; `TerminalBackground` stays terminal_session's).
- **Graphics.** `kitty_graphics::detect()` is called once at session start inside the "everything that can fail goes before the child exists" block (`pty.rs:446-452`): `session_runner` calls it where `pty.rs` does; no timeout, no `which tmux`, no port (P8-T34).
- **`tracker.rs` and `types.rs`** are untouched by Phase 8 (`types::Model` stays for the renderer; `Msg` for the driver). `cli.rs` keeps `diagnostic`, `debug`, the key tester and their tests; `debug_shell_survives_closed_terminal` (`cli.rs:593`) names its own path (D9): update the string when it moves.
- **Phase 11 watch list** (D7): `Menu::vis()` (now `#[cfg(test)]`), `show_on_tab`'s unused `settings` parameter, `MenuEffect::Insert.full`, `ThemeDefinition`'s two dead-code serde fields (`version`, `theme` in the mapper's private `ThemeFile`; they validate), `Theme::description_border()` (read only by `ThemePalette::theme()`), `KeyCode::F(0)`, the `Hash` derives, `MenuLayout::side()`
  (`#[cfg(test)]`), `IconId::Png(usize)` unvalidated. `LEGACY_MODULES` keeps `icons keys menu render`.

## Glossary additions (proposed for `docs/ubiquitous-language.md`; 8.7 writes them)

dropdown:

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `KeystrokeParser` | Domain service | Parses `ctrl+k`, `shift+tab`, `+`, `f12` into a `Key`; `normalize` makes ctrl and alt letters case-insensitive. | `parse_keystroke`, `normalize` |
| `CommonPrefix` / `CommonPrefixOutcome` | Domain service / VO | The longest prefix the visible suggestions share and what inserting it does (fail, insert the selected row, insert a prefix). | `Menu::common_prefix` decision, `matching_type` |
| `Rgb` (edit) | VO (alias of a tuple) | `(u8, u8, u8)`. | `render::Rgb` |
| `ThemeDefinition` (edit) / `ThemeAuthor` | VO | The strictly parsed custom theme file: its author, for the `theme NAME` banner. | `cli::{ThemeFile, ThemeAuthor}` |
| `ThemePalette` | VO | The colour roles a custom theme file gives (Fig `version "1.0"` or base16), each possibly missing; `theme()` fills the gaps from dark or the dark filler. | `Theme::from_json` rules |
| `ThemeChoice` (edit) | VO | dark, light, system or a custom name; the three built-in names. | `BUILTIN_THEMES`, the `from_settings` match |
| `ThemeRepository` (`FileThemeRepository`) | Repository | The themes folder: its path, the strict read of a theme file, the lenient read for drawing, the listing. Every read failure of the lenient path is "no theme". | `data_directory_from_env().themes_dir()` + `read_to_string` in `render` and `cli` |
| `ThemeResolver` | Domain service | Which `Theme` `autocomplete.theme` and the terminal background give (the repository is asked only for a custom name). | `Theme::from_settings` |
| `ThemeSelection` / `CurrentTheme` | DTO | What `theme NAME` announces (a custom definition or a built-in) / what `theme` prints (a name or a non-text value). | the `theme` command's locals |
| `ColorParser` / `TextFitting` | Domain service | `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()` and the xterm-256 downgrade / terminal-width `clean`, `fit`, `wrap`. | `parse_color`, `xterm256` / `clean`, `cw`, `width`, `fit`, `wrap` |
| `MenuLayoutCalculator` / `MenuDescriber` | Domain service | Above or below, rows, x0 and width of the menu / the bold label and description the description bar and pop-out show. | `render::layout` / `render::describe` |
| `IconResolver` | Domain service | The icon cell (two columns for every row kind; a spec badge is stored as given, P8-T33) and icon id for a suggestion under a graphics mode (emoji, `fig://` icons, type defaults). | `icons::{slot, spec_image, type_image, ascii_slot, wide_emoji}` |
| `DropdownError` | Domain error | Settings (printed as itself), data directory, invalid theme file, theme not found. | the `bail!`s of `cli::theme` |

terminal_session:

| Term | Kind | Meaning | Replaces |
|---|---|---|---|
| `KeyEvent` | VO | A decoded `Key` with the byte range of the stdin chunk it came from; every byte of a chunk belongs to exactly one event. | `keys::KeyEvent` |
| `StdinKeyDecoder` | Adapter | Decodes stdin bytes (UTF-8, `ESC`, SS3, CSI, bracketed paste) into `KeyEvent`s; keeps only the paste state between chunks. | `keys::Decoder`, `decode*` |
| `OverlayRenderer` / `FrameInput` | Adapter / DTO | Draws the menu, description and ghost over the alacritty model as bytes, and the clear that restores the cells; the per-frame inputs. | `render::{Renderer, FrameInput}` |
| `KittyImage` / `KittyPixels` | Adapter type | The id and the PNG or RGBA bytes of one icon, as the kitty graphics protocol transmits them. | `icons::{Image, Source}` |

No row: `*Parts` structs, `InMemoryThemeRepository`, `DropdownModule`, `ThemeFileError` (an adapter error, as `AtomicFileError` has none), `icon_assets`, `kitty_graphics` (named in prose). `docs/ubiquitous-language.md`'s `IconSlot / IconId / GraphicsSupport` row stays but for its first meaning, `Two-column icon cell`, which becomes `Icon cell (two columns for every row kind; a spec badge is stored as given)` (`IconId` is an id, no bytes).

"Words with more than one meaning" gains ONE row, because R14 had to settle it: `theme file` | `ThemeDefinition` (the strict read: its author, for the `theme NAME` banner) · `ThemePalette` (the lenient read: its colour roles, for drawing). No `key` row: `Key`, `SettingKey` and `GeneratorKey` predate Phase 8 and each already has a glossary row.

## Steps

Each implementer reads `agent-brief.md`, then this file and the research notes named in its prompt, and loads the skills `xp-tdd`, `testing-standard`, `coding-standards` and `hexagonal-architecture` (AGENTS.md). Every step ends with the full gate green and everything committed
(R26), the test lock clean (MISSING 0, DUPLICATE 0, exactly the step's CHANGED tests), the local fitness run with `dropdown` registered and reverted, the cfg check, the clippy budget and the `should_` names grepped first. Each step compiles on its own and leaves no shim across its end
except the one named in R13 (8.3b to 8.5d).

### Run 1: guards, keys, menu (8.0a-8.2c, 8.4a)
Ends with `keys.rs` and `menu.rs` gone, `src/dropdown/` holding the key and menu domain, `IconId` and `IconSlot` in place; `all_bundle_modules` `1 passed`.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 8.0a | Guards: `theme-diff.sh` and the lenient theme tests | (section 8) | `chore(scripts)`: the script per the guard section, proven deterministic and equal on baseline and `phase8-start`; `test(dropdown)`: the three `Theme::from_json` tests in `render.rs` (pass on the old code). No production change |
| 8.0b | Guards: byte hashes and characterisation tests | (section 8) | the icons hash test, the overlay hash test, five keymap/parser tests in `keys.rs`, two common-prefix tests in `menu.rs` (R28; pass on the old code; tables generated at the tag) |
| 8.1a | The key domain | 1 | scaffold `src/dropdown/{mod, domain/{mod, value_objects/mod, services/mod}}.rs` + `mod dropdown;` (R23); move `KeyCode`, `Mods` as `Modifiers` and `Key` (fields still `pub(crate)`), `Action` as `KeyAction` (+ `action_ids`), `Binding` as `KeyBinding`, `parse_keystroke`/`normalize` as `keystroke_parser.rs` (+ `keystroke_grammar`, the 8.0b parser tests), `DEFAULTS` + `Keymap` as `keymap.rs` (+ the 8.0b keymap tests); `keys.rs` keeps `KeyEvent`, `Decoder`, `decode` and the composite test, importing the new paths; `pty.rs`, `menu.rs` and `render.rs:1065` switch their imports and names in the same commit (no shim); the local fitness registration starts |
| 8.1b | The stdin decoder leaves `keys.rs` | 1 | `KeyEvent` to `terminal_session/domain/value_objects/key_event.rs`, `Decoder` to `StdinKeyDecoder` in `adapters/stdin_key_decoder.rs` (`#[cfg(test)] decode`, the consts, `decode_one/esc/csi`), with `decode_table`, `decode_multi_key_chunk`, `esc_not_last_and_double_esc`, `paste_across_chunks` and `keymap_defaults_and_overrides` + helpers; `src/keys.rs` and `mod keys;` deleted; `pty.rs` imports the decoder (R4, R5; P8-T5-T9) |
| 8.1c | Private key fields | 1 | `Modifiers`, `Key`, `KeyEvent` get private fields, `const fn` builders and accessors; the construction sites in `keystroke_parser.rs`, `stdin_key_decoder.rs` (about 10), `pty.rs` (`ghost_key`, `is_ctrl`, `note_forwarded`, `on_keys`, `ghost_keys_and_words_keys`) and the tests change; three new builder tests (R3); `integrations-diff`/`precmd-diff`/`inline-diff` empty |
| 8.2a | Menu seams: fixtures and clock | 2 | commit 1 `test(completion)`: `item`, `comp`, `settings` to `test_fixtures.rs`, importers (`menu.rs`, `pty.rs:1758`, `render.rs:893`, `icons.rs:379`) switch, no shim; commit 2 RED/GREEN `refactor(dropdown)`: `on_completion(c, s, now)`, `loading(now)`, the callers (R7), the 11 tests and the spinner test get `now`, three new boundary tests |
| 8.2b | `Menu` moves into the dropdown domain | 2 | `git mv src/menu.rs src/dropdown/domain/entities/menu.rs`; `MenuVisibility` and `MenuEffect` sliced out (`pub` to `pub(crate)`, `vis()` `#[cfg(test)]`); `pty.rs`, `render.rs` import paths; tests sliced to `entities/menu/tests.rs`; `mod menu;` deleted; the six `settings_catalog.rs` comments repointed (R9, R27) |
| 8.2c | `CommonPrefix` and the D8 arms | 2 | RED/GREEN: `common_prefix_outcome.rs`, `services/common_prefix.rs` (`decide`, `matching_type`) with five service tests; `Menu::common_prefix` as the apply step; the explicit arms for `v => v` and the description actions (R9); the `common_prefix` test stays whole |
| 8.4a | Icon seams (new, no caller) | 3 | `icon_id.rs` (`IconId`, `ICON_NAMES`, `value`), `icon_slot.rs` (`IconSlot`), three tests (one temporary guard against the legacy `PNGS`); nothing in legacy changes except that test (R20) |

### Run 2: the riskiest step, alone (8.4b)
Ends with `icons.rs` gone; `grep -rn 'crate::icons' src` empty; the byte-hash test unchanged in value.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 8.4b | Icons split across the layers | 3, 5 | in ONE commit (it cannot build in halves: `slot()` stops returning bytes): `graphics_support.rs`; `icon_resolver.rs` (`slot` and helpers, ids only, private `width` copy); `icon_id.rs` gains the moved `placeholders`/`color`; `icon_assets.rs` (`pngs!`, `PNGS`, `TEMPLATE_RGBA`, `tinted`, `kitty_image`) and `kitty_graphics.rs` (`KittyImage`, `KittyPixels`, `transmit`, `place`, `CLEAR_PLACEMENTS`, `base64`, `tmux_graphics`, `detect`); `render.rs` adapts (`Patch.img: Option<IconId>`, `list_row`, `compose`, `GraphicsSupport`, `sent` keyed by `value()`); `pty.rs:452` uses `kitty_graphics::detect()`; `icons.rs` and `mod icons;` deleted; the six tests move (two D6 halves, R20); the 8.4a temporary guard becomes `should_embed_the_png_assets_in_the_order_of_the_icon_ids`; the byte-hash test still passes with the same values; `adapters/mod.rs` gains two lines |

### Run 3: render, theme, renderer, close (8.3a-8.3c, 8.5a-8.5d, 8.6, 8.7)
Ends with `render.rs` gone, `cli.rs` free of `theme`, `dropdown` registered, Phase 8 recorded.

| Id | Title | Plan step | What it does |
|---|---|---|---|
| 8.3a | Colour and text helpers | 3 | `rgb.rs`, `color_parser.rs` (`parse_color`, `xterm256`, test A), `text_fitting.rs` (`clean cw width fit wrap`, `truncation_and_wrap` D6-split); `colours_and_themes` split in place (A moves, B and C keep their final names in `render.rs`); `icon_resolver`'s private `width` replaced by `text_fitting::width`; `render.rs` imports (R11, R19; P8-T22-T24, T29) |
| 8.3b | The `Theme` value object | 3 | `theme.rs` (`Theme`, `ThemeParts`, accessors, `dark`, `light`), one new test; `render.rs` keeps the shim `impl Theme { from_settings, from_json }`, `SHADES`, `ACCENTS` (R13) and reads through accessors; `pty.rs` changes its `use` line; B and C read accessors (R12; P8-T25, T26) |
| 8.3c | Layout and describer | 3 | `menu_side.rs`, `menu_layout.rs`, `menu_layout_calculator.rs` (`layout`, `layout_decisions` CHANGED), `menu_describer.rs` (`describe`, D8 arms, three new tests); `menu_patches` and `desc_row`/`popout_rows` use them (R19; P8-T27, T28) |
| 8.5a | Theme domain and use cases (new, no caller) | 4 | `dropdown_error.rs`, `theme_choice.rs`, `theme_author.rs`, `theme_definition.rs`, `repositories/theme_repository.rs` (`folder`, `find`, `list` + `InMemoryThemeRepository`), the three use cases and DTOs, `application/mod.rs`; about 27 tests, test-first (R14, R15, R17; P8-T36-T41) |
| 8.5b | Theme infrastructure and command (new, no caller) | 4 | `theme_json_mapper.rs` (strict entry), `theme_file_error.rs`, `file_theme_repository.rs`, `dropdown_module.rs`, `cli/theme_command.rs` (`ThemeArgs` copied verbatim, handler, banner, `SettingValue` JSON fn, parity test), the `mod.rs` lines; about 24 tests; legacy `cli.rs` is still the only caller (the duplication of the serde structs lasts until 8.5c) |
| 8.5c | The `theme` command switches | 4 | ONE commit: `bootstrap/cli.rs` (`Theme(ThemeArgs)`, `run_theme_command`, `dropdown_module()`); `cli.rs` loses `ThemeArgs`, `settings_repository`, `BUILTIN_THEMES`, `ThemeAuthor`, `ThemeFile`, `parse_theme`, `theme`, `theme_list`, `theme_banner`, the unused imports and the `//!` word; `theme_list_order` and `theme_file_parsing` split (R17); `theme-diff.sh` baseline vs new empty, golden identical |
| 8.5d | The live theme: palette, resolver, repository | 4 | RED/GREEN: `theme_palette.rs` (+ `SHADES`/`ACCENTS`), `find_palette` on the trait, the in-memory double and `FileThemeRepository`, the lenient entry of `theme_json_mapper.rs`, `theme_resolver.rs`; `pty.rs`: `themes` field, three call sites, test literal; the shim, `SHADES`, `ACCENTS`, `serde_json::Value` and `data_directory_from_env` leave `render.rs`; B and C and the 8.0a tests move; `settings_catalog.rs:545` repointed (R13, R15, R16; P8-T24-T26, T38) |
| 8.6 | The renderer moves to terminal_session | 5 | `git mv src/render.rs src/terminal_session/infrastructure/adapters/overlay_renderer.rs`; `Renderer` becomes `OverlayRenderer`; imports; `FrameInput` stays; tests inline with `truncation_and_wrap_spinner`; `pty.rs` lines 8, 342, 451, 978 comment, 1793; `mod render;` deleted; `settings_catalog.rs:305` repointed; the overlay byte-hash test unchanged in value (R21; P8-T44-T47) |
| 8.7 | Register `dropdown` and record the phase | 5 | R24; the greps below; plan sections 4, 5, 8, 9 ("Phase 8 notes", the deviation list), glossary rows, `README.md` (status, `theme-diff.sh`), `handoff.md`; `cargo test --locked --release -- --ignored all_bundle_modules`; `cargo check --locked --no-default-features --all-targets`; the orchestrator asks the user for the manual smoke (O8) |

Per-step guards and test-lock expectations ("CHANGED" = changed for the first time against `phase8-start`):

| Id | Guards (on top of every test) | New tests | CHANGED |
|---|---|---|---|
| 8.0a | theme-diff determinism and baseline equality; `scripts` only | 3 | none |
| 8.0b | the hash tests pass on the old code at the tag; `all_bundle_modules` | 9 | none |
| 8.1a | all pty tests, `keymap_defaults_and_overrides`, the 8.0b keymap tests | none | `decode_table`, `keystroke_grammar`, `ghost_keys_and_words_keys` (only the `Mods {..}` to `Modifiers {..}` literal type name; path-only: `action_ids`, `enter_records_the_submitted_line`) |
| 8.1b | the pty tests that drive keys (list in "Test moves"), `debug_shell_survives_closed_terminal` | none | none new: the three inherited from 8.1a stay as they are (path-only: `paste_across_chunks`) |
| 8.1c | `ghost_keys_and_words_keys`, the key tests, precmd/inline/integrations diffs | 3 | the 6 of "Test moves" (3 of them newly: `decode_multi_key_chunk`, `esc_not_last_and_double_esc`, `paste_across_chunks`; the other 3 inherited from 8.1a) |
| 8.2a | the 12 menu tests, `round_trip_arg_hint_spinner_and_wide_ghost`, all pty tests 20x, the overlay hash | 3 | 11 menu tests + the spinner test |
| 8.2b | the menu tests, `settings_catalog` default tests | none | none (path-only) |
| 8.2c | `common_prefix`, `execute_and_record`, the two 8.0b menu tests, all pty tests | 5 | none |
| 8.4a | the icon hash test (still the old `slot`); `all_bundle_modules` (the end of run 1) | 3 | none |
| 8.4b | the icon hash test (same values), the overlay hash, all render and pty tests 20x, `debug_shell_survives_closed_terminal` alone, `cargo check --no-default-features` | 3 + the split half | `resolves_like_q`, `tmux_uses_placeholders`, `transmit_chunks` |
| 8.3a | the overlay hash, `row_contents`, `truncation_and_wrap` halves | 3 (splits) | `colours_and_themes`, `truncation_and_wrap` |
| 8.3b | the overlay hash (every colour), `row_contents`, B and C | 1 | none |
| 8.3c | the overlay hash (layout), `layout_decisions` | 3 | `layout_decisions` |
| 8.5a | none beyond the new tests (no caller) | about 27 | none |
| 8.5b | none beyond the new tests (no caller) | about 24 | none |
| 8.5c | theme-diff baseline vs new, golden, precmd/inline/integrations diffs, `completion bash/zsh/fish` | 2 (halves) | `theme_list_order`, `theme_file_parsing` |
| 8.5d | the overlay hash (custom theme scenario), the 8.0a tests, theme-diff (unchanged), pty tests, `debug_shell_survives_closed_terminal` alone | about 15 | none |
| 8.6 | all render-derived tests by their new path, the overlay hash, `pty::tests::*` | none | none (path-only: +3 against `phase8-start`, +2 against `ef807c1`, where `row_contents` is already CHANGED) |
| 8.7 | full gate; testlock vs `ef807c1` and `phase8-start`; every diff script; `all_bundle_modules` | none | none |

**8.7 greps** (empty outside `#[cfg(test)]` unless said):
- `grep -rnE 'crate::(types|engine|parser|suggest|pty|menu|render|icons|doctor|cli|tracker|keys|shell_parser|js|spec|inline)\b' src/dropdown`;
- `grep -rnE 'crate::(shell_integration|history|ghost_text|terminal_session|diagnostics|bootstrap)\b' src/dropdown`;
- `grep -rn 'use crate::{' src/dropdown` reviewed by hand; `grep -rn 'pub(crate) use' src/dropdown src/terminal_session src/pty.rs src/cli.rs` empty;
- `grep -rnE '\b(anyhow|clap)\b' src/dropdown`: only `infrastructure/cli/theme_command.rs` and, inside its `#[cfg(test)]` module, `domain/dropdown_error.rs` (D2, as the tests of `ghost_text_error.rs` in Phase 7); `grep -rnE '\b(serde|serde_json)\b|json!' src/dropdown`: only `infrastructure/adapters/theme_json_mapper.rs`, `infrastructure/cli/theme_command.rs` and test modules;
- `grep -rnE '\b(e?print(ln)?|dbg)!' src/dropdown`: only `infrastructure/cli/theme_command.rs`; `grep -rn 'logger' src/dropdown` empty; `grep -rn 'alacritty_terminal' src/dropdown` empty;
- `grep -rn 'crate::settings::infrastructure' src/dropdown` empty outside tests; `grep -rnE 'dropdown::(application|infrastructure)' src/terminal_session` empty outside tests;
- `grep -rnE '\bunicode_width\b' src/dropdown`: only `text_fitting.rs` and `icon_resolver.rs` (if `width` is not deduped, only there);
- `grep -rnE 'pub(\(crate\))? [a-z_]+:' src/dropdown/domain` empty outside the `*Parts` structs;
- `grep -rnE '\b(Mods|Action|Binding|Vis|Effect|Graphics|Image|Source|Renderer|Slot)\b' src/dropdown src/terminal_session` shows no legacy name (`Action::`/`Effect::` of other enums excepted by hand);
- `grep -rnE 'completion::application::(command_line_parser|generator_runner|test_fixtures)' src --exclude-dir=completion` empty; `grep -rn 'cfg!(' src/dropdown` empty;
- `grep -n 'cfg(' src/dropdown -r` only `#[cfg(test)]` and `#[cfg(unix)]` on the chmod tests; `grep -rn 'Rc<dyn ThemeRepository>' src` shows the module, the resolver's callers and `pty.rs`.

## Open questions (for the plan owner; the default applies if nobody answers)

1. **O1 Private key fields (R3)** or `pub(crate)` fields on `Key`/`Modifiers`/`KeyEvent` (fewer CHANGED tests, an exception to the domain rule)? *Default: private, in their own step.*
2. **O2 The renderer reads `Instant::now()` itself (R7 B)** or `FrameInput` gets `now: Instant` now? *Default: the renderer reads it; Phase 9 adds the field.*
3. **O3 `tinted` stays in `icon_assets.rs` (R20)** or moves to `icon_resolver.rs` with a template parameter as plan step 3 says? *Default: infrastructure, verbatim (the test body is untouched).*
4. **O4 `keymap_defaults_and_overrides` kept whole (R5)** or the strict D6 split with six new decode rows? *Default: whole, a recorded deviation.*
5. **O5 `Rc<dyn ThemeRepository>` in the session (R15).** Is `Session` single-threaded for the rest of Phase 9? *Default: `Rc`; switch to `Arc<dyn .. + Send + Sync>` if Phase 9 finds a thread boundary (a `DataDirectory` is `Send + Sync`).*
6. **O6 Rename `Image`/`Source` to `KittyImage`/`KittyPixels` (R20)** or keep the legacy names inside `kitty_graphics.rs`? *Default: rename (the field names stay, `transmit_chunks` is CHANGED by type names only).*
7. **O7 `FrameInput` stays in `overlay_renderer.rs` (R21)** or moves to `terminal_session/application/frame_input.rs` now? *Default: stays; 9.4 moves it.*
8. **O8 The manual smoke** (kitty/ghostty direct icons, tmux placeholders, a custom and a `system` theme, plan section 8): once, after 8.7? *Default: once, after 8.7, because the overlay has no golden coverage; the byte-hash tests and the round-trip tests are the automated nets.*

## Launch

From the repo root on the new Phase 8/9 branch (created off `refactor/hexagonal-dropdown-terminal-session` by the orchestrator), with the gate green at `7aafcef`:
1. Re-check the DEFERRED items (K1, K21) and the working tree (K20): commit `phase-workflow.js` (the `args.branch` option) with the hand-over docs; `git status` must be clean for 8.0a.
2. Commit this file as `docs: hand Phase 8 over with its research`. The tag `phase8-start` stays at `7aafcef` (the testlock baseline of every step; the docs commit changes no code).
3. Run the workflow once per run, each time with the run's steps only and `branch` set to the Phase 8/9 branch: run 1 = `8.0a 8.0b 8.1a 8.1b 8.1c 8.2a 8.2b 8.2c 8.4a`; run 2 = `8.4b`; run 3 = `8.3a 8.3b 8.3c 8.5a 8.5b 8.5c 8.5d 8.6 8.7`. Before runs 2 and 3 the orchestrator writes a binding
   "SINCE" preamble into each step prompt (what the earlier runs left: layout, signatures as built, gate/testlock/theme-diff numbers, accepted deviations) and a "SCOPE OF THIS RUN" preamble for the reviewers (judge only this run's steps; do not demand later runs' work).

```js
Workflow({
  scriptPath: "docs/hexagonal-refactor/phase-workflow.js",
  args: /* { phase: "Phase 8 (dropdown)", startRev: <HEAD, then the previous run's end>, branch: <the Phase 8/9 branch>,
              reviewNotes: <this file's reviewer contract>, steps: <the run's steps, each prompt prefixed with its SINCE preamble> } */
})
```
