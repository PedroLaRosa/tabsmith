//! simple-icons' `data/simple-icons.json`: the colour, licence and brand guidelines of every brand, by slug.

use crate::colour::Rgb;
use crate::manifest::Entry;
use serde_json::Value;
use std::collections::HashMap;

pub struct Catalogue {
    brands: HashMap<String, Value>,
}

impl Catalogue {
    pub fn from_json(text: &str) -> Result<Catalogue, String> {
        let list: Vec<Value> = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let brands = list.into_iter().map(|brand| (brand["slug"].as_str().unwrap_or("").to_string(), brand)).collect();
        Ok(Catalogue { brands })
    }

    pub fn check(&self, entry: &Entry) -> Result<(), String> {
        let Some(brand) = self.brands.get(&entry.slug) else {
            return Err(format!("{} is not in the simple-icons data file", entry.slug));
        };
        let hex = brand["hex"].as_str().unwrap_or("");
        if Rgb::from_hex(hex) != Ok(entry.colour) {
            return Err(format!("{} is {} in the manifest but {hex} in simple-icons", entry.slug, entry.colour));
        }
        if let (Some(guidelines), false) = (brand["guidelines"].as_str(), entry.guidelines_ok) {
            return Err(format!(
                "{} has brand guidelines ({guidelines}): read them, and mark the line guidelines-ok if they allow a row icon",
                entry.slug
            ));
        }
        let licence = brand["license"]["type"].as_str();
        if !is_allowed(licence) {
            let licence = licence.unwrap_or_default();
            return Err(format!("{} is licensed {licence}, which this project does not use", entry.slug));
        }
        Ok(())
    }
}

/// Plain CC0 only. Attribution licences (MIT, Apache-2.0, BSD, CC-BY) ask for notices tabsmith's binary does not ship;
/// share-alike, copyleft, non-commercial, no-derivatives and custom terms are refused too.
const ALLOWED_LICENCES: &[&str] = &["CC0-1.0"];

/// No licence field means the entry falls under simple-icons' own CC0-1.0.
fn is_allowed(licence: Option<&str>) -> bool {
    licence.is_none_or(|licence| ALLOWED_LICENCES.contains(&licence))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = r#"[
        {"title": "pnpm", "slug": "pnpm", "hex": "F69220", "source": "https://pnpm.io"},
        {"title": "Python", "slug": "python", "hex": "3776AB", "source": "https://python.org",
         "guidelines": "https://www.python.org/community/logos/"},
        {"title": ".NET", "slug": "dotnet", "hex": "512BD4", "source": "https://dot.net", "license": {"type": "CC0-1.0"}},
        {"title": "Rust", "slug": "rust", "hex": "000000", "source": "https://rust-lang.org",
         "license": {"type": "CC-BY-SA-4.0"}}
    ]"#;

    fn entry(slug: &str, colour: Rgb) -> Entry {
        Entry { slug: slug.into(), colour, guidelines_ok: false }
    }

    #[test]
    fn should_refuse_an_icon_missing_from_the_data_file() {
        let catalogue = Catalogue::from_json(DATA).unwrap();

        let checked = catalogue.check(&entry("turborepo", Rgb(0xFF, 0x1E, 0x56)));

        assert_eq!(checked, Err("turborepo is not in the simple-icons data file".to_string()));
    }

    #[test]
    fn should_refuse_a_colour_that_differs_from_the_data_file() {
        let catalogue = Catalogue::from_json(DATA).unwrap();

        let checked = catalogue.check(&entry("pnpm", Rgb(0xF6, 0x92, 0x21)));

        assert_eq!(checked, Err("pnpm is F69221 in the manifest but F69220 in simple-icons".to_string()));
    }

    #[test]
    fn should_refuse_an_icon_whose_brand_guidelines_were_not_reviewed() {
        let catalogue = Catalogue::from_json(DATA).unwrap();

        let checked = catalogue.check(&entry("python", Rgb(0x37, 0x76, 0xAB)));

        assert_eq!(
            checked,
            Err("python has brand guidelines (https://www.python.org/community/logos/): read them, and mark the line \
                 guidelines-ok if they allow a row icon"
                .to_string())
        );
    }

    #[test]
    fn should_accept_an_icon_whose_brand_guidelines_were_reviewed() {
        let catalogue = Catalogue::from_json(DATA).unwrap();
        let reviewed = Entry { guidelines_ok: true, ..entry("python", Rgb(0x37, 0x76, 0xAB)) };

        let checked = catalogue.check(&reviewed);

        assert_eq!(checked, Ok(()));
    }

    #[test]
    fn should_refuse_an_icon_whose_licence_does_not_allow_this_use() {
        let catalogue = Catalogue::from_json(DATA).unwrap();

        let checked = catalogue.check(&entry("rust", Rgb(0, 0, 0)));

        assert_eq!(checked, Err("rust is licensed CC-BY-SA-4.0, which this project does not use".to_string()));
    }

    #[test]
    fn should_accept_icons_whose_colour_matches_and_whose_licence_allows_this_use() {
        let catalogue = Catalogue::from_json(DATA).unwrap();

        let checked = [entry("pnpm", Rgb(0xF6, 0x92, 0x20)), entry("dotnet", Rgb(0x51, 0x2B, 0xD4))].map(|e| catalogue.check(&e));

        assert_eq!(checked, [Ok(()), Ok(())]);
    }

    #[test]
    fn should_allow_an_entry_without_a_licence_of_its_own() {
        let licence = None;

        let allowed = is_allowed(licence);

        assert!(allowed);
    }

    #[test]
    fn should_reject_share_alike_copyleft_non_commercial_no_derivatives_custom_and_unknown_licences() {
        let licences = [
            "CC-BY-SA-4.0", "GPL-3.0-only", "AGPL-3.0-only", "MPL-2.0", "CC-BY-NC-4.0", "CC-BY-ND-4.0", "custom",
            "Unlicense",
        ];

        let allowed = licences.map(|licence| is_allowed(Some(licence)));

        assert_eq!(allowed, [false; 8]);
    }

    #[test]
    fn should_allow_cc0() {
        let allowed = is_allowed(Some("CC0-1.0"));

        assert!(allowed);
    }

    #[test]
    fn should_reject_attribution_licences_whose_notices_tabsmith_does_not_ship() {
        let licences = ["MIT", "Apache-2.0", "BSD-3-Clause", "CC-BY-3.0", "CC-BY-4.0"];

        let allowed = licences.map(|licence| is_allowed(Some(licence)));

        assert_eq!(allowed, [false; 5]);
    }
}
