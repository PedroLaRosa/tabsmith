//! `assets/icons/simple-icons.txt`: one drawn icon per line, `<slug> <HEX>`. The icon is named after its simple-icons
//! slug, and the colour must equal the one in the pinned simple-icons data file.

use crate::colour::Rgb;

#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub slug: String,
    pub colour: Rgb,
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (slug, hex) = line.split_once(' ').unwrap();
            Ok(Entry { slug: slug.to_string(), colour: Rgb::from_hex(hex)? })
        })
        .collect()
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
                Entry { slug: "pnpm".into(), colour: Rgb(0xF6, 0x92, 0x20) },
                Entry { slug: "turborepo".into(), colour: Rgb(0xFF, 0x1E, 0x56) },
            ])
        );
    }
}
