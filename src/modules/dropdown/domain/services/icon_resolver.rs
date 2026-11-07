//! Row icons: which icon a row gets, as an id and a two-column slot (`assets/icons` has the PNGs, origin and licence
//! in its README). The pixels and the kitty protocol live in `terminal_session`.

use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
use crate::modules::dropdown::domain::services::text_fitting::width;
use crate::modules::dropdown::domain::value_objects::graphics_support::GraphicsSupport;
use crate::modules::dropdown::domain::value_objects::icon_id::{placeholders, IconId, ICON_NAMES};
use crate::modules::dropdown::domain::value_objects::icon_slot::IconSlot;
use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;

/// Slot of a row: the spec's `icon` first (emoji text, then `fig://` images), else the row type.
pub(crate) fn slot(it: &Suggestion, g: GraphicsSupport) -> IconSlot {
    let spec = it.icon();
    if let Some(emoji) = spec.and_then(wide_emoji) {
        return text(&emoji);
    }
    if g == GraphicsSupport::Off {
        return ascii_slot(it.kind());
    }
    let s = spec
        .and_then(spec_image)
        .unwrap_or_else(|| type_image(it.kind()));
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

/// Default icon per row type. History rows keep their 📚 from the history source.
fn type_image(kind: SuggestionKind) -> IconSlot {
    match kind {
        SuggestionKind::Subcommand => png_slot("command"),
        SuggestionKind::Option => png_slot("option"),
        SuggestionKind::Arg => png_slot("box"),
        SuggestionKind::Special => badge_slot(Some((0x62, 0x8d, 0xad)), "➡️"),
        SuggestionKind::Shortcut => badge_slot(Some((0x34, 0x98, 0xdb)), "💡"),
        SuggestionKind::AutoExecute => png_slot("carrot"),
        SuggestionKind::File => png_slot("file"),
        SuggestionKind::Folder => png_slot("folder"),
        SuggestionKind::History => text("📚"),
    }
}

/// Single-character fallback, right-aligned in the two-column slot (SPEC §7.4.2).
fn ascii_slot(kind: SuggestionKind) -> IconSlot {
    let c = match kind {
        SuggestionKind::Subcommand => '>',
        SuggestionKind::Option => '-',
        SuggestionKind::Arg | SuggestionKind::Special => '*',
        SuggestionKind::File => 'f',
        SuggestionKind::Folder => 'd',
        SuggestionKind::History => 'h',
        SuggestionKind::Shortcut => '@',
        SuggestionKind::AutoExecute => '↪',
    };
    text(&format!(" {c}"))
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

    #[test]
    fn resolves_like_q() {
        let it = |icon: Option<&str>, kind| item("x", kind, "").with_icon(icon.map(String::from));
        // The current-branch star is an emoji: text in both modes.
        assert_eq!(
            slot(
                &it(Some("\u{2b50}\u{fe0f}"), SuggestionKind::Subcommand),
                GraphicsSupport::Direct
            )
            .text(),
            "⭐"
        );
        // A named icon is its PNG; an unknown name is the plain template.
        let s = slot(
            &it(Some("fig://icon?type=git"), SuggestionKind::Subcommand),
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
            GraphicsSupport::Direct,
        );
        assert!(
            s.is_badge()
                && s.text() == "🔥"
                && matches!(s.image(), Some(IconId::Tinted(..)))
        );
        // Type defaults with images; ASCII without.
        assert!(slot(&it(None, SuggestionKind::Option), GraphicsSupport::Direct)
            .image()
            .is_some());
        assert_eq!(slot(&it(None, SuggestionKind::Option), GraphicsSupport::Off).text(), " -");
        assert!(slot(&it(None, SuggestionKind::Option), GraphicsSupport::Off)
            .image()
            .is_none());
        assert_eq!(
            slot(&it(None, SuggestionKind::History), GraphicsSupport::Direct).text(),
            "📚"
        );
        // Narrow emoji and remote URLs fall back to the type.
        assert_eq!(
            slot(
                &it(Some("\u{2699}\u{fe0f}"), SuggestionKind::Arg),
                GraphicsSupport::Direct
            )
            .text(),
            "  "
        );
        assert_eq!(
            slot(
                &it(Some("https://fig.io/x.png"), SuggestionKind::Arg),
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
                width(slot(&it(None, k), GraphicsSupport::Direct).text()),
                2,
                "{k:?}"
            );
            assert_eq!(width(slot(&it(None, k), GraphicsSupport::Off).text()), 2, "{k:?}");
            assert_eq!(width(slot(&it(None, k), GraphicsSupport::Tmux).text()), 2, "{k:?}");
        }
    }

    #[test]
    fn tmux_uses_placeholders() {
        let it = |icon: Option<&str>, kind| item("x", kind, "").with_icon(icon.map(String::from));
        // A PNG: two placeholder cells, its id in a 256-colour foreground.
        let s = slot(&it(None, SuggestionKind::Option), GraphicsSupport::Tmux);
        let id = s.image().map(IconId::value).unwrap();
        assert!(id < 256);
        assert_eq!(s.text().matches('\u{10eeee}').count(), 2);
        assert_eq!(width(s.text()), 2);
        assert_eq!(color(id), format!(";38;5;{id}"));
        // A tinted template: 24-bit colour plus the top-byte diacritic.
        let s = slot(
            &it(Some("fig://template?color=2ecc71"), SuggestionKind::Arg),
            GraphicsSupport::Tmux,
        );
        assert_eq!(
            s.text(),
            "\u{10eeee}\u{305}\u{305}\u{30d}\u{10eeee}\u{305}\u{30d}\u{30d}"
        );
        assert_eq!(color(0x0100_0000 | 0x2ecc71), ";38;2;46;204;113");
        // A badge cannot sit on a placeholder: text alone.
        let s = slot(&it(None, SuggestionKind::Shortcut), GraphicsSupport::Tmux);
        assert!(s.image().is_none() && s.text() == "💡");
    }
}
