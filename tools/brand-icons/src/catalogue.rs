fn is_allowed(licence: Option<&str>) -> bool {
    licence.is_none()
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
}
