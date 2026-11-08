//! The embedded icon pixels (origin and licence in `assets/icons/README.md`): the 42 PNGs, in the order of `ICON_NAMES`,
//! and the template the badge colours tint.

use crate::modules::dropdown::domain::value_objects::icon_id::IconId;
use crate::modules::terminal_session::infrastructure::adapters::kitty_graphics::{KittyImage, KittyPixels};

macro_rules! pngs {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("../../../../../assets/icons/", $name, ".png")) as &[u8])),*]
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
const TEMPLATE_RGBA: &[u8] = include_bytes!("../../../../../assets/icons/template.rgba");

/// The kitty image of an icon: its embedded PNG, or the template tinted with its colour (made when it is sent).
pub(crate) fn kitty_image(icon: IconId) -> KittyImage {
    let src = match icon {
        IconId::Png(index) => KittyPixels::Png(PNGS[index].1),
        IconId::Tinted(r, g, b) => KittyPixels::Rgba(tinted(r, g, b)),
    };
    KittyImage {
        id: icon.value(),
        src,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::completion::domain::test_fixtures::item;
    use crate::modules::completion::domain::value_objects::suggestion::Suggestion;
    use crate::modules::dropdown::domain::services::icon_resolver::slot;
    use crate::modules::dropdown::domain::value_objects::graphics_support::GraphicsSupport;
    use crate::modules::dropdown::domain::value_objects::icon_id::ICON_NAMES;
    use crate::modules::specs::domain::value_objects::suggestion_kind::SuggestionKind;
    use crate::modules::terminal_session::infrastructure::adapters::kitty_graphics::transmit;

    #[test]
    fn should_embed_the_png_assets_in_the_order_of_the_icon_ids() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let on_disk: Vec<(&str, Vec<u8>)> = ICON_NAMES
            .iter()
            .map(|name| {
                let file = std::fs::read(format!("{manifest}/assets/icons/{name}.png")).unwrap();
                (*name, file)
            })
            .collect();

        let embedded: Vec<(&str, Vec<u8>)> = PNGS
            .iter()
            .map(|(name, bytes)| (*name, bytes.to_vec()))
            .collect();

        assert!(
            embedded == on_disk,
            "the embedded PNGs differ from assets/icons/<name>.png in ICON_NAMES order"
        );
    }

    #[test]
    fn should_transmit_a_png_icon_with_its_embedded_bytes_under_its_id() {
        let git = ICON_NAMES.iter().position(|n| *n == "git").unwrap();
        let file = include_bytes!("../../../../../assets/icons/git.png");

        let sent = transmit(&kitty_image(IconId::Png(git)), GraphicsSupport::Direct);

        let expected = KittyImage {
            id: git as u32 + 1,
            src: KittyPixels::Png(file),
        };
        assert_eq!(sent, transmit(&expected, GraphicsSupport::Direct));
    }

    #[test]
    fn should_transmit_a_tinted_icon_as_32_by_32_rgba() {
        let tint = IconId::Tinted(0x34, 0x98, 0xdb);
        let first_chunk = format!("\x1b_Ga=t,i={},f=32,s=32,v=32,q=2,m=1;", tint.value());
        let expected = KittyImage {
            id: tint.value(),
            src: KittyPixels::Rgba(tinted(0x34, 0x98, 0xdb)),
        };

        let sent = transmit(&kitty_image(tint), GraphicsSupport::Direct);

        assert!(sent.starts_with(&first_chunk), "{}", &sent[..40]);
        assert_eq!(sent, transmit(&expected, GraphicsSupport::Direct));
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
    fn assets_are_pngs() {
        for (n, bytes) in PNGS {
            assert!(bytes.starts_with(b"\x89PNG"), "{n}");
        }
    }

    /// 64-bit FNV-1a: a stable hash that needs no crate and does not change between Rust versions.
    fn fnv1a(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
        })
    }

    /// Everything one row icon puts on the wire in `g`: the slot (text, badge flag, image id) and the transmission of
    /// its image. This is the one function that says HOW the values are produced.
    fn icon_fingerprint(it: &Suggestion, g: GraphicsSupport) -> String {
        let s = slot(it, g);
        let id = s.image().map(IconId::value);
        let sent = s
            .image()
            .map(|icon| transmit(&kitty_image(icon), g))
            .unwrap_or_default();
        format!("{:?}|{}|{:?}|{}", s.text(), s.is_badge(), id, sent)
    }

    /// (label, row) of every icon input: the 42 PNG names, then four templates (two tinted type defaults with a
    /// badge, a tinted one without, an untinted one with a badge).
    fn icon_inputs() -> Vec<(String, Suggestion)> {
        let row = |kind, icon: Option<&str>| item("x", kind, "").with_icon(icon.map(String::from));
        let mut inputs: Vec<(String, Suggestion)> = PNGS
            .iter()
            .map(|(name, _)| {
                let icon = format!("fig://icon?type={name}");
                (
                    format!("png {name}"),
                    row(SuggestionKind::Subcommand, Some(&icon)),
                )
            })
            .collect();
        inputs.push(("tint special".into(), row(SuggestionKind::Special, None)));
        inputs.push(("tint shortcut".into(), row(SuggestionKind::Shortcut, None)));
        inputs.push((
            "tint 2ecc71".into(),
            row(SuggestionKind::Arg, Some("fig://template?color=2ecc71")),
        ));
        inputs.push((
            "template badge".into(),
            row(SuggestionKind::Arg, Some("fig://template?badge=ab")),
        ));
        inputs
    }

    #[test]
    fn should_render_every_icon_exactly_as_at_the_phase_8_start() {
        // The slot and the kitty transmission of every icon, hashed. Generated once at the phase8-start tag; a
        // refactor may change how a value is produced (`icon_fingerprint`), never a value.
        const EXPECTED: &[(&str, &str, u64)] = &[
            ("png alert", "direct", 0x7d404841b200a873),
            ("png alert", "tmux", 0x3fab539c2b7ca314),
            ("png android", "direct", 0xaf8e7bdf9655898d),
            ("png android", "tmux", 0x6c52a13cb3ff5a5a),
            ("png apple", "direct", 0xc3f98d77c63a57f5),
            ("png apple", "tmux", 0xe488a811a009c6a6),
            ("png asterisk", "direct", 0x8be06551395b4aa7),
            ("png asterisk", "tmux", 0xb8e9f7a306d2494c),
            ("png aws", "direct", 0x9863f3b0d1d6830e),
            ("png aws", "tmux", 0x78b5655337421fc7),
            ("png azure", "direct", 0xb4f2daeef00416cd),
            ("png azure", "tmux", 0x0d077507c1fbdcb6),
            ("png box", "direct", 0xcbe160b84c017519),
            ("png box", "tmux", 0x1bdf6a1d23bb3a86),
            ("png carrot", "direct", 0xe000c1840efe28e3),
            ("png carrot", "tmux", 0xc6b53b1d06452cb0),
            ("png characters", "direct", 0x6a2669fe17174ab7),
            ("png characters", "tmux", 0x4476e883c2e64690),
            ("png command", "direct", 0x317b760b9d456c32),
            ("png command", "tmux", 0x99bde194eca918ab),
            ("png commandkey", "direct", 0x0ce55d3c72ef8ded),
            ("png commandkey", "tmux", 0x01d5cfbcc538dcf1),
            ("png commit", "direct", 0x9264a8d55abdf39c),
            ("png commit", "tmux", 0x96dc43e88f21d631),
            ("png cpu", "direct", 0x6d76a56ba1c52ccd),
            ("png cpu", "tmux", 0x6015bed9cae819ea),
            ("png database", "direct", 0x8a6262aedfb5c61e),
            ("png database", "tmux", 0x2e41b8eb6215f90a),
            ("png discord", "direct", 0x284e8cd869c3fc8e),
            ("png discord", "tmux", 0xeaea57e7323e3fb7),
            ("png docker", "direct", 0x55871b0ee4ac00ec),
            ("png docker", "tmux", 0xe230d27368ab45d8),
            ("png file", "direct", 0x43b60cf922e56ca6),
            ("png file", "tmux", 0x92969722fcdec477),
            ("png firebase", "direct", 0x2a01b536dbdc4e97),
            ("png firebase", "tmux", 0x41d481d3e7b03f28),
            ("png flag", "direct", 0xbba944c77d9e211d),
            ("png flag", "tmux", 0xe4ba99d1e7bc30ce),
            ("png folder", "direct", 0x2242e373dd03b019),
            ("png folder", "tmux", 0x2e609659939f8b56),
            ("png gcloud", "direct", 0x9f2f4fe4af54fdd6),
            ("png gcloud", "tmux", 0x122fb0f85ab0e26f),
            ("png gear", "direct", 0xf29f01183eeb9593),
            ("png gear", "tmux", 0x493b088c5511c7e4),
            ("png git", "direct", 0x1c301f480d72a1f8),
            ("png git", "tmux", 0xbdbb7aca656e6595),
            ("png github", "direct", 0x212a2f103ac32f85),
            ("png github", "tmux", 0x6b05e944e01509d2),
            ("png gitlab", "direct", 0xc7e6f9113e29428f),
            ("png gitlab", "tmux", 0x8533977d17ab45cc),
            ("png gradle", "direct", 0x9cdc49470e1c3d72),
            ("png gradle", "tmux", 0x0e86983f362fc56f),
            ("png heroku", "direct", 0x334c65f95e90bdc5),
            ("png heroku", "tmux", 0x492b819712a36a53),
            ("png invite", "direct", 0x89cd7bbe8d07129f),
            ("png invite", "tmux", 0x74c6a808cfe0cac0),
            ("png kubernetes", "direct", 0x35080ec591e46891),
            ("png kubernetes", "tmux", 0xe60136e36b5d17a1),
            ("png netlify", "direct", 0xba0d969052c96596),
            ("png netlify", "tmux", 0x5542b12d40f901bb),
            ("png node", "direct", 0x0a53ca5f7b5ee725),
            ("png node", "tmux", 0xbe1c3bdd1a538bd2),
            ("png npm", "direct", 0xf81af9821d42fcc8),
            ("png npm", "tmux", 0xf6e6841dffa73746),
            ("png okteto", "direct", 0xb0ce18e6f8727ee4),
            ("png okteto", "tmux", 0x3c42ac60b0712e35),
            ("png option", "direct", 0xaa4a361377415055),
            ("png option", "tmux", 0x2f888629b0425fee),
            ("png package", "direct", 0xfe35581fed847092),
            ("png package", "tmux", 0xa2127fc9bb4adfaa),
            ("png slack", "direct", 0xa37df088d4c7ba78),
            ("png slack", "tmux", 0x2f3b4c3a0b43a48d),
            ("png string", "direct", 0xc844441e2a56be5e),
            ("png string", "tmux", 0x54346600495e623f),
            ("png symlink", "direct", 0x93c4eb42daa7dddf),
            ("png symlink", "tmux", 0x9a22d2e60ef444ae),
            ("png template", "direct", 0x0cfca74ac9a76f55),
            ("png template", "tmux", 0xbbb983c339323de6),
            ("png twitter", "direct", 0xe22574159a7c14d1),
            ("png twitter", "tmux", 0xaaa9e94744cecd51),
            ("png vercel", "direct", 0xa49cff9962e3b8a9),
            ("png vercel", "tmux", 0x1410b89a4761d4a6),
            ("png yarn", "direct", 0x8c7a7da12bbabfb6),
            ("png yarn", "tmux", 0x22fef5c33e74c627),
            ("tint special", "direct", 0x37a2b91fc886f390),
            ("tint special", "tmux", 0xd5482069cbc60063),
            ("tint shortcut", "direct", 0xfe51868b7a4e9e97),
            ("tint shortcut", "tmux", 0xefdcd71b327a6bec),
            ("tint 2ecc71", "direct", 0xcaa9f2e6391f8596),
            ("tint 2ecc71", "tmux", 0x7e583289aadadc36),
            ("template badge", "direct", 0xee29695d360ba10b),
            ("template badge", "tmux", 0xae2ee3b4f2089db9),
        ];
        let mut actual = Vec::new();
        for (label, it) in icon_inputs() {
            for (mode, g) in [
                ("direct", GraphicsSupport::Direct),
                ("tmux", GraphicsSupport::Tmux),
            ] {
                let hash = fnv1a(icon_fingerprint(&it, g).as_bytes());
                actual.push((label.clone(), mode, hash));
            }
        }

        assert_eq!(actual.len(), 92);
        assert_eq!(actual.len(), EXPECTED.len());
        for ((label, mode, hash), (want_label, want_mode, want_hash)) in actual.iter().zip(EXPECTED)
        {
            assert_eq!((label.as_str(), *mode), (*want_label, *want_mode));
            assert_eq!(hash, want_hash, "{label} in {mode}");
        }
    }
}
