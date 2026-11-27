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
