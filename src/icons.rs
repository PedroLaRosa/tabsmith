//! Row icons: the PNGs in `assets/icons` (origin and licence in its README). Terminals with the kitty graphics
//! protocol get the pixels; the others get ASCII.

use std::fmt::Write;

use unicode_width::UnicodeWidthChar;

use crate::completion::domain::value_objects::suggestion::Suggestion;
use crate::specs::domain::value_objects::suggestion_kind::SuggestionKind;

macro_rules! pngs {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../assets/icons/", $name, ".png")) as &[u8])),*]
    };
}

/// Row icon names plus the local-only `database`, `file`, `folder` and `symlink`. Order = image ids (1-based).
const PNGS: &[(&str, &[u8])] = pngs![
    "alert",
    "android",
    "apple",
    "asterisk",
    "aws",
    "azure",
    "box",
    "carrot",
    "characters",
    "command",
    "commandkey",
    "commit",
    "cpu",
    "database",
    "discord",
    "docker",
    "file",
    "firebase",
    "flag",
    "folder",
    "gcloud",
    "gear",
    "git",
    "github",
    "gitlab",
    "gradle",
    "heroku",
    "invite",
    "kubernetes",
    "netlify",
    "node",
    "npm",
    "okteto",
    "option",
    "package",
    "slack",
    "string",
    "symlink",
    "template",
    "twitter",
    "vercel",
    "yarn",
];

/// `template.png` as RGBA8 (32×32), tinted per badge colour.
const TEMPLATE_RGBA: &[u8] = include_bytes!("../assets/icons/template.rgba");

/// Base64 payload per kitty chunk (a multiple of 4, the protocol's limit).
const CHUNK: usize = 4096;

/// What a kitty placement shows.
pub struct Image {
    pub id: u32,
    pub src: Source,
}

pub enum Source {
    Png(&'static [u8]),
    /// 32×32 RGBA8.
    Rgba(Vec<u8>),
}

/// The 2-column icon slot of a row: `text` always spans two columns; `image` (kitty only) covers it, under the text.
pub struct Slot {
    pub text: String,
    pub image: Option<Image>,
    /// Badge text is drawn white.
    pub badge: bool,
}

/// How row icons reach the terminal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Graphics {
    /// ASCII icons.
    Off,
    /// Kitty graphics placements under the text (kitty, Ghostty).
    Direct,
    /// Kitty or Ghostty behind tmux: images go through tmux's passthrough as virtual placements and the slot holds
    /// kitty Unicode placeholders, which tmux keeps, redraws and clears like any text.
    Tmux,
}

/// Kitty graphics protocol is used for Ghostty and kitty only. Inside tmux, tmux tells the outer terminal and whether
/// it passes sequences through (`allow-passthrough`, off by default).
pub fn detect() -> Graphics {
    let var = |k: &str| std::env::var(k).ok();
    if var("TMUX").is_none() {
        let on = var("KITTY_WINDOW_ID").is_some()
            || var("TERM_PROGRAM").as_deref() == Some("ghostty")
            || var("TERM").as_deref() == Some("xterm-kitty");
        return if on { Graphics::Direct } else { Graphics::Off };
    }
    let mut cmd = std::process::Command::new("tmux");
    cmd.args(["display", "-p"]);
    if let Some(pane) = var("TMUX_PANE") {
        cmd.args(["-t", &pane]);
    }
    let out = cmd
        .arg("#{allow-passthrough} #{client_termname} #{client_termtype}")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    tmux_graphics(&out)
}

/// `allow-passthrough client_termname client_termtype` → Tmux when passthrough is on and the client is kitty or
/// Ghostty (by `TERM` or by its XTVERSION reply, e.g. `kitty(0.47.1)`).
fn tmux_graphics(out: &str) -> Graphics {
    match out.trim().split_once(' ') {
        Some((pass, client))
            if matches!(pass, "on" | "all")
                && (client.contains("kitty") || client.contains("ghostty")) =>
        {
            Graphics::Tmux
        }
        _ => Graphics::Off,
    }
}

/// Slot of a row: the spec's `icon` first (emoji text, then `fig://` images), else the row type.
pub fn slot(it: &Suggestion, g: Graphics) -> Slot {
    let spec = it.icon.as_deref();
    if let Some(emoji) = spec.and_then(wide_emoji) {
        return text(&emoji);
    }
    if g == Graphics::Off {
        return ascii_slot(it.kind);
    }
    let s = spec
        .and_then(spec_image)
        .unwrap_or_else(|| type_image(it.kind));
    if g != Graphics::Tmux {
        return s;
    }
    // A placeholder takes its whole cell, so nothing draws over it: a badge stays text alone.
    match s.image {
        Some(img) if !s.badge => Slot {
            text: placeholders(img.id),
            image: Some(img),
            badge: false,
        },
        _ => text(&s.text),
    }
}

/// Two placeholder cells (row 0, columns 0 and 1) of image `id`, which the foreground colour carries (`color`). A
/// third diacritic holds the id's top byte: 1 for the tinted templates, absent for the PNGs.
fn placeholders(id: u32) -> String {
    let top = if id >> 24 == 0 { "" } else { "\u{30d}" };
    format!("\u{10eeee}\u{305}\u{305}{top}\u{10eeee}\u{305}\u{30d}{top}")
}

/// SGR parameters (after a pen) for the foreground that names image `id` to its placeholders.
pub fn color(id: u32) -> String {
    if id < 256 {
        format!(";38;5;{id}")
    } else {
        format!(";38;2;{};{};{}", id >> 16 & 255, id >> 8 & 255, id & 255)
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
fn spec_image(s: &str) -> Option<Slot> {
    if let Some(q) = s.strip_prefix("fig://icon?") {
        let name = param(q, "type").unwrap_or("template");
        return Some(png_slot(name));
    }
    let q = s.strip_prefix("fig://template?")?;
    let color = param(q, "color").and_then(hex);
    Some(badge_slot(color, param(q, "badge").unwrap_or("")))
}

/// Default icon per row type. History rows keep their 📚 from the history source.
fn type_image(kind: SuggestionKind) -> Slot {
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
fn ascii_slot(kind: SuggestionKind) -> Slot {
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

fn text(s: &str) -> Slot {
    Slot {
        text: pad2(s),
        image: None,
        badge: false,
    }
}

/// The PNG for `name`; a name with no PNG is the plain template.
fn png_slot(name: &str) -> Slot {
    let find = |n: &str| PNGS.iter().position(|(p, _)| *p == n);
    let index = find(name)
        .or_else(|| find("template"))
        .expect("template is in PNGS");
    Slot {
        text: "  ".into(),
        image: Some(Image {
            id: index as u32 + 1,
            src: Source::Png(PNGS[index].1),
        }),
        badge: false,
    }
}

/// Template square (tinted when `color` is given) with `badge` text on top. Without a colour, the plain template PNG.
fn badge_slot(color: Option<(u8, u8, u8)>, badge: &str) -> Slot {
    let image = match color {
        Some((r, g, b)) => Image {
            id: 0x0100_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
            src: Source::Rgba(tinted(r, g, b)),
        },
        None => png_slot("template").image.expect("template is in PNGS"),
    };
    let badge = badge.replace('\u{fe0f}', "");
    Slot {
        text: if badge.is_empty() {
            "  ".into()
        } else {
            pad2(&badge)
        },
        image: Some(image),
        badge: !badge.is_empty(),
    }
}

/// Per-channel scaling: the template's own shading is kept, its colour is multiplied in.
fn tinted(r: u8, g: u8, b: u8) -> Vec<u8> {
    fn scale(a: u8, b: u8) -> u8 {
        ((a as u16 * b as u16) / 256) as u8
    }
    TEMPLATE_RGBA
        .chunks_exact(4)
        .flat_map(|p| [scale(p[0], r), scale(p[1], g), scale(p[2], b), p[3]])
        .collect()
}

/// Exactly two columns: a one-column glyph gets a space.
fn pad2(s: &str) -> String {
    if width(s) == 1 {
        format!("{s} ")
    } else {
        s.to_string()
    }
}

fn width(s: &str) -> usize {
    s.chars().map(|c| c.width().unwrap_or(0)).sum()
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

/// Kitty graphics: transmit `img` as image `id` without displaying it. `q=2` keeps the terminal from answering on
/// stdin. Payloads go in 4096-byte base64 chunks, `m=1` while more follow. Behind tmux each chunk goes through the
/// passthrough, and the first one also makes the slot-sized virtual placement the placeholders show (`U=1`).
pub fn transmit(img: &Image, g: Graphics) -> String {
    let (format, data) = match &img.src {
        Source::Png(b) => ("f=100", *b),
        Source::Rgba(b) => ("f=32,s=32,v=32", b.as_slice()),
    };
    let enc = base64(data);
    let mut out = String::new();
    for (k, chunk) in enc.as_bytes().chunks(CHUNK).enumerate() {
        let more = u8::from((k + 1) * CHUNK < enc.len());
        let chunk = std::str::from_utf8(chunk).unwrap_or_default();
        let apc = if k == 0 {
            let action = if g == Graphics::Tmux {
                "a=T,U=1,c=2,r=1"
            } else {
                "a=t"
            };
            format!(
                "\x1b_G{action},i={},{format},q=2,m={more};{chunk}\x1b\\",
                img.id
            )
        } else {
            format!("\x1b_Gm={more};{chunk}\x1b\\")
        };
        if g == Graphics::Tmux {
            let _ = write!(out, "\x1bPtmux;{}\x1b\\", apc.replace('\x1b', "\x1b\x1b"));
        } else {
            out += &apc;
        }
    }
    out
}

/// Kitty graphics: show `img` at the cursor, one cell row by two columns, under the text (`z=-1`). `p` tells the
/// placements of one frame apart; `C=1` keeps the cursor where it is.
pub fn place(img: &Image, p: usize) -> String {
    format!("\x1b_Ga=p,i={},p={p},c=2,r=1,C=1,z=-1,q=2\x1b\\", img.id)
}

/// Kitty graphics: remove every placement on screen. Image data stays, so it is not sent again.
pub const CLEAR_PLACEMENTS: &str = "\x1b_Ga=d,d=a,q=2\x1b\\";

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = u32::from(c[0]) << 16
            | u32::from(c.get(1).copied().unwrap_or(0)) << 8
            | u32::from(c.get(2).copied().unwrap_or(0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::tests::item;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"M"), "TQ==");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
    }

    #[test]
    fn transmit_chunks() {
        let small = transmit(
            &Image {
                id: 7,
                src: Source::Png(b"Man"),
            },
            Graphics::Direct,
        );
        assert_eq!(small, "\x1b_Ga=t,i=7,f=100,q=2,m=0;TWFu\x1b\\");
        // 3072 bytes → 4096 base64 chars: one chunk; 4000 bytes → 5336 chars: two chunks.
        let big = transmit(
            &Image {
                id: 1,
                src: Source::Rgba(vec![0; 4000]),
            },
            Graphics::Direct,
        );
        let parts: Vec<&str> = big.split("\x1b\\").filter(|s| !s.is_empty()).collect();
        assert_eq!(parts.len(), 2);
        assert!(
            parts[0].starts_with("\x1b_Ga=t,i=1,f=32,s=32,v=32,q=2,m=1;"),
            "{}",
            &parts[0][..40]
        );
        assert!(parts[1].starts_with("\x1b_Gm=0;"));
    }

    #[test]
    fn tint_multiplies_colour_into_template() {
        // scale(a, b) = a * b / 256. Alpha is kept.
        let t = tinted(0x34, 0x98, 0xdb);
        assert_eq!(t.len(), 32 * 32 * 4);
        let (r, g, b, a) = (
            TEMPLATE_RGBA[0],
            TEMPLATE_RGBA[1],
            TEMPLATE_RGBA[2],
            TEMPLATE_RGBA[3],
        );
        assert_eq!(
            &t[..4],
            &[scale(r, 0x34), scale(g, 0x98), scale(b, 0xdb), a]
        );
        assert_eq!(scale(255, 255), 254);
        fn scale(a: u8, b: u8) -> u8 {
            ((a as u16 * b as u16) / 256) as u8
        }
    }

    #[test]
    fn resolves_like_q() {
        let it = |icon: Option<&str>, kind| Suggestion {
            icon: icon.map(String::from),
            ..item("x", kind, "")
        };
        // The current-branch star is an emoji: text in both modes.
        assert_eq!(
            slot(
                &it(Some("\u{2b50}\u{fe0f}"), SuggestionKind::Subcommand),
                Graphics::Direct
            )
            .text,
            "⭐"
        );
        // A named icon is its PNG; an unknown name is the plain template.
        let s = slot(
            &it(Some("fig://icon?type=git"), SuggestionKind::Subcommand),
            Graphics::Direct,
        );
        assert_eq!(
            s.image.as_ref().map(|i| i.id),
            PNGS.iter()
                .position(|(n, _)| *n == "git")
                .map(|i| i as u32 + 1)
        );
        let s = slot(
            &it(Some("fig://icon?type=txt"), SuggestionKind::Subcommand),
            Graphics::Direct,
        );
        assert_eq!(
            s.image.as_ref().map(|i| i.id),
            PNGS.iter()
                .position(|(n, _)| *n == "template")
                .map(|i| i as u32 + 1)
        );
        // Template badges: the tinted square and the badge in white.
        let s = slot(
            &it(
                Some("fig://template?color=2ecc71&badge=\u{1f525}"),
                SuggestionKind::Arg,
            ),
            Graphics::Direct,
        );
        assert!(
            s.badge
                && s.text == "🔥"
                && matches!(
                    s.image,
                    Some(Image {
                        src: Source::Rgba(_),
                        ..
                    })
                )
        );
        // Type defaults with images; ASCII without.
        assert!(slot(&it(None, SuggestionKind::Option), Graphics::Direct)
            .image
            .is_some());
        assert_eq!(slot(&it(None, SuggestionKind::Option), Graphics::Off).text, " -");
        assert!(slot(&it(None, SuggestionKind::Option), Graphics::Off)
            .image
            .is_none());
        assert_eq!(
            slot(&it(None, SuggestionKind::History), Graphics::Direct).text,
            "📚"
        );
        // Narrow emoji and remote URLs fall back to the type.
        assert_eq!(
            slot(
                &it(Some("\u{2699}\u{fe0f}"), SuggestionKind::Arg),
                Graphics::Direct
            )
            .text,
            "  "
        );
        assert_eq!(
            slot(
                &it(Some("https://fig.io/x.png"), SuggestionKind::Arg),
                Graphics::Off
            )
            .text,
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
                width(&slot(&it(None, k), Graphics::Direct).text),
                2,
                "{k:?}"
            );
            assert_eq!(width(&slot(&it(None, k), Graphics::Off).text), 2, "{k:?}");
            assert_eq!(width(&slot(&it(None, k), Graphics::Tmux).text), 2, "{k:?}");
        }
    }

    #[test]
    fn tmux_uses_placeholders() {
        let it = |icon: Option<&str>, kind| Suggestion {
            icon: icon.map(String::from),
            ..item("x", kind, "")
        };
        // A PNG: two placeholder cells, its id in a 256-colour foreground.
        let s = slot(&it(None, SuggestionKind::Option), Graphics::Tmux);
        let id = s.image.as_ref().map(|i| i.id).unwrap();
        assert!(id < 256);
        assert_eq!(s.text.matches('\u{10eeee}').count(), 2);
        assert_eq!(width(&s.text), 2);
        assert_eq!(color(id), format!(";38;5;{id}"));
        // A tinted template: 24-bit colour plus the top-byte diacritic.
        let s = slot(
            &it(Some("fig://template?color=2ecc71"), SuggestionKind::Arg),
            Graphics::Tmux,
        );
        assert_eq!(
            s.text,
            "\u{10eeee}\u{305}\u{305}\u{30d}\u{10eeee}\u{305}\u{30d}\u{30d}"
        );
        assert_eq!(color(0x0100_0000 | 0x2ecc71), ";38;2;46;204;113");
        // A badge cannot sit on a placeholder: text alone.
        let s = slot(&it(None, SuggestionKind::Shortcut), Graphics::Tmux);
        assert!(s.image.is_none() && s.text == "💡");
        // Every chunk is wrapped for tmux, ESC doubled inside.
        let t = transmit(
            &Image {
                id: 7,
                src: Source::Png(b"Man"),
            },
            Graphics::Tmux,
        );
        assert_eq!(
            t,
            "\x1bPtmux;\x1b\x1b_Ga=T,U=1,c=2,r=1,i=7,f=100,q=2,m=0;TWFu\x1b\x1b\\\x1b\\"
        );
        // Detection from `tmux display`.
        assert_eq!(
            tmux_graphics("all xterm-kitty kitty(0.47.1)\n"),
            Graphics::Tmux
        );
        assert_eq!(
            tmux_graphics("on tmux-256color ghostty 1.1.3\n"),
            Graphics::Tmux
        );
        assert_eq!(
            tmux_graphics("off xterm-kitty kitty(0.47.1)\n"),
            Graphics::Off
        );
        assert_eq!(tmux_graphics(" xterm-256color iTerm2 3.5\n"), Graphics::Off);
        assert_eq!(tmux_graphics(""), Graphics::Off);
    }

    #[test]
    fn assets_are_pngs() {
        for (n, bytes) in PNGS {
            assert!(bytes.starts_with(b"\x89PNG"), "{n}");
        }
    }
}
