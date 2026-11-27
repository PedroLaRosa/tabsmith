//! simple-icons' `data/simple-icons.json`: the colour and licence of every brand, by slug.

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
        let licence = brand["license"]["type"].as_str();
        if !is_allowed(licence) {
            let licence = licence.unwrap_or_default();
            return Err(format!("{} is licensed {licence}, which this project does not use", entry.slug));
        }
        Ok(())
    }
}

/// Licences whose terms allow a logo as a row icon: CC0 and the attribution-only permissive ones. Each attribution-licensed
/// icon is credited in `assets/icons/README.md`. Anything else (share-alike, copyleft, non-commercial, no-derivatives,
/// custom) is refused.
const ALLOWED_LICENCES: &[&str] = &["CC0-1.0", "MIT", "Apache-2.0", "BSD-3-Clause", "CC-BY-3.0", "CC-BY-4.0"];

/// No licence field means the entry falls under simple-icons' own CC0-1.0.
fn is_allowed(licence: Option<&str>) -> bool {
    licence.is_none_or(|licence| ALLOWED_LICENCES.contains(&licence))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = r#"[
        {"title": "pnpm", "slug": "pnpm", "hex": "F69220", "source": "https://pnpm.io"},
        {"title": "Deno", "slug": "deno", "hex": "000000", "source": "https://deno.com", "license": {"type": "MIT"}},
        {"title": "Rust", "slug": "rust", "hex": "000000", "source": "https://rust-lang.org",
         "license": {"type": "CC-BY-SA-4.0"}}
    ]"#;

    fn entry(slug: &str, colour: Rgb) -> Entry {
        Entry { slug: slug.into(), colour }
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
    fn should_refuse_an_icon_whose_licence_does_not_allow_this_use() {
        let catalogue = Catalogue::from_json(DATA).unwrap();

        let checked = catalogue.check(&entry("rust", Rgb(0, 0, 0)));

        assert_eq!(checked, Err("rust is licensed CC-BY-SA-4.0, which this project does not use".to_string()));
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
    fn should_allow_cc0_and_attribution_only_permissive_licences() {
        let licences = ["CC0-1.0", "MIT", "Apache-2.0", "BSD-3-Clause", "CC-BY-3.0", "CC-BY-4.0"];

        let allowed = licences.map(|licence| is_allowed(Some(licence)));

        assert_eq!(allowed, [true; 6]);
    }
}
