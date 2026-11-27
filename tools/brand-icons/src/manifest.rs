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
        .enumerate()
        .map(|(index, line)| (index + 1, line.trim()))
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .map(|(number, line)| entry(line).map_err(|e| format!("line {number}: {e}")))
        .collect()
}

fn entry(line: &str) -> Result<Entry, String> {
    match line.split_whitespace().collect::<Vec<_>>()[..] {
        [slug, hex] => Ok(Entry { slug: slug.to_string(), colour: Rgb::from_hex(hex)? }),
        _ => Err(format!("expected \"<slug> <HEX>\", got {line:?}")),
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
                Entry { slug: "pnpm".into(), colour: Rgb(0xF6, 0x92, 0x20) },
                Entry { slug: "turborepo".into(), colour: Rgb(0xFF, 0x1E, 0x56) },
            ])
        );
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
}
