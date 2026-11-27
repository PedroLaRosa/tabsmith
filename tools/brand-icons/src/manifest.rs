//! `assets/icons/simple-icons.txt`: one drawn icon per line, `<slug> <HEX>`, then `guidelines-ok` when the brand has
//! guidelines of its own and they were read and allow this use. The icon is named after its simple-icons slug, and the
//! colour must equal the one in the pinned simple-icons data file.

use crate::colour::Rgb;

#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub slug: String,
    pub colour: Rgb,
    /// The brand's guidelines were read and allow a row icon.
    pub guidelines_ok: bool,
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    text.lines()
        .enumerate()
        .map(|(index, line)| (index + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .map(|(number, line)| entry(line).map_err(|e| format!("line {number}: {e}")))
        .collect()
}

/// The 42 PNGs taken from Amazon Q (B3.4): the tool writes `<slug>.png`, so a slug among these would overwrite one.
const AMAZON_Q_ICONS: &[&str] = &[
    "alert", "android", "apple", "asterisk", "aws", "azure", "box", "carrot", "characters", "command", "commandkey",
    "commit", "cpu", "database", "discord", "docker", "file", "firebase", "flag", "folder", "gcloud", "gear", "git",
    "github", "gitlab", "gradle", "heroku", "invite", "kubernetes", "netlify", "node", "npm", "okteto", "option",
    "package", "slack", "string", "symlink", "template", "twitter", "vercel", "yarn",
];

fn entry(line: &str) -> Result<Entry, String> {
    match line.split_whitespace().collect::<Vec<_>>()[..] {
        [slug, ..] if AMAZON_Q_ICONS.contains(&slug) => {
            Err(format!("{slug} is an Amazon Q icon in assets/icons, never redrawn"))
        }
        [slug, hex] => Ok(Entry { slug: slug.to_string(), colour: Rgb::from_hex(hex)?, guidelines_ok: false }),
        [slug, hex, "guidelines-ok"] => {
            Ok(Entry { slug: slug.to_string(), colour: Rgb::from_hex(hex)?, guidelines_ok: true })
        }
        _ => Err(format!("expected \"<slug> <HEX> [guidelines-ok]\", got {line:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colour::Rgb;

    #[test]
    fn should_read_one_icon_per_line_skipping_comments_and_blank_lines() {
        let text = "# Brand icons drawn from simple-icons.\n\npnpm F69220\nturborepo FF1E56\n";

        let entries = parse(text);

        assert_eq!(
            entries,
            Ok(vec![
                Entry { slug: "pnpm".into(), colour: Rgb(0xF6, 0x92, 0x20), guidelines_ok: false },
                Entry { slug: "turborepo".into(), colour: Rgb(0xFF, 0x1E, 0x56), guidelines_ok: false },
            ])
        );
    }

    #[test]
    fn should_read_that_a_brands_guidelines_were_reviewed() {
        let text = "python 3776AB guidelines-ok\npnpm F69220\n";

        let reviewed = parse(text).map(|entries| entries.iter().map(|e| e.guidelines_ok).collect::<Vec<_>>());

        assert_eq!(reviewed, Ok(vec![true, false]));
    }

    #[test]
    fn should_name_the_line_that_is_not_a_slug_and_a_colour() {
        let lines = ["# header\npnpm\n", "# header\npnpm F69220 extra\n", "# header\npnpm orange\n"];

        let errors: Vec<bool> = lines
            .iter()
            .map(|text| parse(text).is_err_and(|e| e.starts_with("line 2: ")))
            .collect();

        assert_eq!(errors, [true, true, true]);
    }

    #[test]
    fn should_refuse_an_icon_named_like_one_of_the_amazon_q_icons() {
        let text = "# header\npnpm F69220\ndocker 2496ED\n";

        let entries = parse(text);

        assert_eq!(entries, Err("line 3: docker is an Amazon Q icon in assets/icons, never redrawn".to_string()));
    }
}
