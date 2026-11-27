//! Row icons: which icon a row gets, as an id and a two-column slot (`assets/icons` has the PNGs, origin and licence
//! in its README). The pixels and the kitty protocol live in `terminal_session`.

use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
use crate::modules::dropdown::domain::services::brand_catalog::brand_icon;
use crate::modules::dropdown::domain::services::icon_palette::{tile_color, HISTORY_TILE};
use crate::modules::dropdown::domain::services::text_fitting::width;
use crate::modules::dropdown::domain::value_objects::graphics_support::GraphicsSupport;
use crate::modules::dropdown::domain::value_objects::icon_id::{placeholders, IconId, ICON_NAMES};
use crate::modules::dropdown::domain::value_objects::icon_slot::IconSlot;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// Slot of a row: the spec's `icon` first (emoji text, then `fig://` images), else the row type's icon. Subcommand and
/// argument rows of a `command` with a brand show that brand instead (SPEC §7.4.2).
pub(crate) fn slot(it: &Suggestion, command: Option<&str>, g: GraphicsSupport) -> IconSlot {
    let spec = it.icon();
    if let Some(emoji) = spec.and_then(wide_emoji) {
        return text(&emoji);
    }
    let s = spec
        .and_then(spec_image)
        .unwrap_or_else(|| type_image(it.kind(), command.and_then(brand_icon)));
    if g == GraphicsSupport::Off {
        return tile_slot(&s, it.kind());
    }
    if g != GraphicsSupport::Tmux {
        return s;
    }
    // A placeholder takes its whole cell, so nothing draws over it: a badge stays text alone.
    match s.image() {
        Some(img) if !s.is_badge() => IconSlot::new(placeholders(img.value()), Some(img), false),
        _ => text(s.text()),
    }
}

/// A short icon string is drawn as text. Only single wide emoji are kept: narrow ones (`⚙️`) have a width that
/// terminals disagree on, which would tear the frame.
fn wide_emoji(s: &str) -> Option<String> {
    let e: String = s.chars().filter(|&c| c != '\u{fe0f}').collect();
    (e.chars().count() == 1 && width(&e) == 2).then_some(e)
}

/// `fig://icon?type=<name>` → that PNG, or the plain template when there is no such icon (`DEFAULT_ICON`).
/// `fig://template?color=<hex>&badge=<text>` → the tinted template with the badge.
fn spec_image(s: &str) -> Option<IconSlot> {
    if let Some(q) = s.strip_prefix("fig://icon?") {
        let name = param(q, "type").unwrap_or("template");
        return Some(png_slot(name));
    }
    let q = s.strip_prefix("fig://template?")?;
    let color = param(q, "color").and_then(hex);
    Some(badge_slot(color, param(q, "badge").unwrap_or("")))
}

/// Default icon per row type, the `brand` for subcommands and arguments. History rows keep their 📚 from the history
/// source.
fn type_image(kind: SuggestionKind, brand: Option<&str>) -> IconSlot {
    match kind {
        SuggestionKind::Subcommand => png_slot(brand.unwrap_or("command")),
        SuggestionKind::Option => png_slot("option"),
        SuggestionKind::Arg => png_slot(brand.unwrap_or("box")),
        SuggestionKind::Special => badge_slot(Some((0x62, 0x8d, 0xad)), "➡️"),
        SuggestionKind::Shortcut => badge_slot(Some((0x34, 0x98, 0xdb)), "💡"),
        SuggestionKind::AutoExecute => png_slot("carrot"),
        SuggestionKind::File => png_slot("file"),
        SuggestionKind::Folder => png_slot("folder"),
        SuggestionKind::History => text("📚"),
    }
}

/// Without images: the row type's character, right-aligned in the two-column slot, on a tile of the colour of the icon
/// `shown` would draw (SPEC §7.4.2).
fn tile_slot(shown: &IconSlot, kind: SuggestionKind) -> IconSlot {
    let mark = format!(" {}", kind_mark(kind));
    match shown.image() {
        Some(icon) => IconSlot::tile(mark, tile_color(icon)),
        // Only history rows draw no image (their 📚 is text).
        None => IconSlot::tile(mark, HISTORY_TILE),
    }
}

fn kind_mark(kind: SuggestionKind) -> char {
    match kind {
        SuggestionKind::Subcommand => '>',
        SuggestionKind::Option => '-',
        SuggestionKind::Arg | SuggestionKind::Special => '*',
        SuggestionKind::File => 'f',
        SuggestionKind::Folder => 'd',
        SuggestionKind::History => 'h',
        SuggestionKind::Shortcut => '@',
        SuggestionKind::AutoExecute => '↪',
    }
}

fn text(s: &str) -> IconSlot {
    IconSlot::new(pad2(s), None, false)
}

/// The PNG for `name`; a name with no PNG is the plain template.
fn png_slot(name: &str) -> IconSlot {
    let find = |n: &str| ICON_NAMES.iter().position(|p| *p == n);
    let index = find(name)
        .or_else(|| find("template"))
        .expect("template is in ICON_NAMES");
    IconSlot::new("  ".to_string(), Some(IconId::Png(index)), false)
}

/// Template square (tinted when `color` is given) with `badge` text on top. Without a colour, the plain template PNG.
fn badge_slot(color: Option<(u8, u8, u8)>, badge: &str) -> IconSlot {
    let image = match color {
        Some((r, g, b)) => IconId::Tinted(r, g, b),
        None => png_slot("template")
            .image()
            .expect("template is in ICON_NAMES"),
    };
    let badge = badge.replace('\u{fe0f}', "");
    IconSlot::new(
        if badge.is_empty() {
            "  ".to_string()
        } else {
            pad2(&badge)
        },
        Some(image),
        !badge.is_empty(),
    )
}

/// Exactly two columns: a one-column glyph gets a space.
fn pad2(s: &str) -> String {
    if width(s) == 1 {
        format!("{s} ")
    } else {
        s.to_string()
    }
}

fn param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|kv| {
        kv.split_once('=')
            .filter(|(k, _)| *k == key)
            .map(|(_, v)| v)
    })
}

fn hex(s: &str) -> Option<(u8, u8, u8)> {
    if s.len() != 6 {
        return None;
    }
    let c = |i: usize| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
    Some((c(0)?, c(2)?, c(4)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::test_fixtures::item;
    use crate::modules::dropdown::domain::value_objects::icon_id::color;

    fn png(name: &str) -> Option<IconId> {
        ICON_NAMES.iter().position(|n| *n == name).map(IconId::Png)
    }

    #[test]
    fn should_show_the_command_brand_on_a_subcommand_row() {
        let checkout = item("checkout", SuggestionKind::Subcommand, "ch");

        let icon = slot(&checkout, Some("git"), GraphicsSupport::Direct).image();

        assert_eq!(icon, png("git"));
    }

    #[test]
    fn should_show_the_command_brand_on_an_argument_row() {
        let pods = item("pods", SuggestionKind::Arg, "");

        let icon = slot(&pods, Some("kubectl"), GraphicsSupport::Direct).image();

        assert_eq!(icon, png("kubernetes"));
    }

    #[test]
    fn should_keep_a_spec_emoji_over_the_brand() {
        let branch = item("main", SuggestionKind::Arg, "").with_icon(Some("\u{2b50}\u{fe0f}".into()));

        let shown = slot(&branch, Some("git"), GraphicsSupport::Direct);

        assert_eq!((shown.text(), shown.image()), ("⭐", None));
    }

    #[test]
    fn should_keep_a_named_spec_icon_over_the_brand() {
        let warning = item("x", SuggestionKind::Subcommand, "").with_icon(Some("fig://icon?type=alert".into()));

        let icon = slot(&warning, Some("git"), GraphicsSupport::Direct).image();

        assert_eq!(icon, png("alert"));
    }

    #[test]
    fn should_show_the_brand_when_the_spec_icon_cannot_be_drawn() {
        let row = |icon: &str| item("x", SuggestionKind::Subcommand, "").with_icon(Some(icon.into()));
        let narrow_emoji_and_remote_url = [row("\u{2699}\u{fe0f}"), row("https://fig.io/x.png")];

        let icons = narrow_emoji_and_remote_url.map(|it| slot(&it, Some("git"), GraphicsSupport::Direct).image());

        assert_eq!(icons, [png("git"); 2]);
    }

    #[test]
    fn should_keep_the_kind_icon_on_rows_that_are_neither_subcommands_nor_arguments() {
        let kinds = [
            SuggestionKind::Option,
            SuggestionKind::File,
            SuggestionKind::Folder,
            SuggestionKind::History,
            SuggestionKind::Shortcut,
            SuggestionKind::Special,
            SuggestionKind::AutoExecute,
        ];

        let branded: Vec<_> = kinds
            .into_iter()
            .filter(|&kind| {
                let row = item("x", kind, "");
                slot(&row, Some("git"), GraphicsSupport::Direct) != slot(&row, None, GraphicsSupport::Direct)
            })
            .collect();

        assert_eq!(branded, Vec::new());
    }

    #[test]
    fn should_show_the_command_icon_for_a_command_without_a_brand() {
        let sub = item("sub", SuggestionKind::Subcommand, "su");

        let icon = slot(&sub, Some("mytool"), GraphicsSupport::Direct).image();

        assert_eq!(icon, png("command"));
    }

    #[test]
    fn should_draw_the_brand_as_placeholders_behind_tmux() {
        let checkout = item("checkout", SuggestionKind::Subcommand, "ch");
        let git = png("git").unwrap();

        let shown = slot(&checkout, Some("git"), GraphicsSupport::Tmux);

        assert_eq!((shown.text(), shown.image()), (placeholders(git.value()).as_str(), Some(git)));
    }

    #[test]
    fn should_fill_the_slot_with_the_brand_colour_without_images() {
        let checkout = item("checkout", SuggestionKind::Subcommand, "ch");

        let shown = slot(&checkout, Some("git"), GraphicsSupport::Off);

        assert_eq!((shown.text(), shown.tile_color()), (" >", Some((0xff, 0x40, 0x1e))));
    }

    #[test]
    fn should_fill_a_history_row_with_grey_without_images() {
        let earlier = item("git status", SuggestionKind::History, "");

        let shown = slot(&earlier, None, GraphicsSupport::Off);

        assert_eq!((shown.text(), shown.tile_color()), (" h", Some((0x8e, 0x8e, 0x93))));
    }

    #[test]
    fn resolves_like_q() {
        let it = |icon: Option<&str>, kind| item("x", kind, "").with_icon(icon.map(String::from));
        // The current-branch star is an emoji: text in both modes.
        assert_eq!(
            slot(
                &it(Some("\u{2b50}\u{fe0f}"), SuggestionKind::Subcommand),
                None,
                GraphicsSupport::Direct
            )
            .text(),
            "⭐"
        );
        // A named icon is its PNG; an unknown name is the plain template.
        let s = slot(
            &it(Some("fig://icon?type=git"), SuggestionKind::Subcommand),
            None,
            GraphicsSupport::Direct,
        );
        assert_eq!(
            s.image().map(IconId::value),
            ICON_NAMES.iter()
                .position(|n| *n == "git")
                .map(|i| i as u32 + 1)
        );
        let s = slot(
            &it(Some("fig://icon?type=txt"), SuggestionKind::Subcommand),
            None,
            GraphicsSupport::Direct,
        );
        assert_eq!(
            s.image().map(IconId::value),
            ICON_NAMES.iter()
                .position(|n| *n == "template")
                .map(|i| i as u32 + 1)
        );
        // Template badges: the tinted square and the badge in white.
        let s = slot(
            &it(
                Some("fig://template?color=2ecc71&badge=\u{1f525}"),
                SuggestionKind::Arg,
            ),
            None,
            GraphicsSupport::Direct,
        );
        assert!(
            s.is_badge()
                && s.text() == "🔥"
                && matches!(s.image(), Some(IconId::Tinted(..)))
        );
        // Type defaults with images; ASCII without.
        assert!(slot(&it(None, SuggestionKind::Option), None, GraphicsSupport::Direct)
            .image()
            .is_some());
        assert_eq!(slot(&it(None, SuggestionKind::Option), None, GraphicsSupport::Off).text(), " -");
        assert!(slot(&it(None, SuggestionKind::Option), None, GraphicsSupport::Off)
            .image()
            .is_none());
        assert_eq!(
            slot(&it(None, SuggestionKind::History), None, GraphicsSupport::Direct).text(),
            "📚"
        );
        // Narrow emoji and remote URLs fall back to the type.
        assert_eq!(
            slot(
                &it(Some("\u{2699}\u{fe0f}"), SuggestionKind::Arg),
                None,
                GraphicsSupport::Direct
            )
            .text(),
            "  "
        );
        assert_eq!(
            slot(
                &it(Some("https://fig.io/x.png"), SuggestionKind::Arg),
                None,
                GraphicsSupport::Off
            )
            .text(),
            " *"
        );
        for k in [
            SuggestionKind::Subcommand,
            SuggestionKind::Option,
            SuggestionKind::Arg,
            SuggestionKind::Special,
            SuggestionKind::File,
            SuggestionKind::Folder,
            SuggestionKind::History,
            SuggestionKind::Shortcut,
            SuggestionKind::AutoExecute,
        ] {
            assert_eq!(
                width(slot(&it(None, k), None, GraphicsSupport::Direct).text()),
                2,
                "{k:?}"
            );
            assert_eq!(width(slot(&it(None, k), None, GraphicsSupport::Off).text()), 2, "{k:?}");
            assert_eq!(width(slot(&it(None, k), None, GraphicsSupport::Tmux).text()), 2, "{k:?}");
        }
    }

    #[test]
    fn tmux_uses_placeholders() {
        let it = |icon: Option<&str>, kind| item("x", kind, "").with_icon(icon.map(String::from));
        // A PNG: two placeholder cells, its id in a 256-colour foreground.
        let s = slot(&it(None, SuggestionKind::Option), None, GraphicsSupport::Tmux);
        let id = s.image().map(IconId::value).unwrap();
        assert!(id < 256);
        assert_eq!(s.text().matches('\u{10eeee}').count(), 2);
        assert_eq!(width(s.text()), 2);
        assert_eq!(color(id), format!(";38;5;{id}"));
        // A tinted template: 24-bit colour plus the top-byte diacritic.
        let s = slot(
            &it(Some("fig://template?color=2ecc71"), SuggestionKind::Arg),
            None,
            GraphicsSupport::Tmux,
        );
        assert_eq!(
            s.text(),
            "\u{10eeee}\u{305}\u{305}\u{30d}\u{10eeee}\u{305}\u{30d}\u{30d}"
        );
        assert_eq!(color(0x0100_0000 | 0x2ecc71), ";38;2;46;204;113");
        // A badge cannot sit on a placeholder: text alone.
        let s = slot(&it(None, SuggestionKind::Shortcut), None, GraphicsSupport::Tmux);
        assert!(s.image().is_none() && s.text() == "💡");
    }
}
