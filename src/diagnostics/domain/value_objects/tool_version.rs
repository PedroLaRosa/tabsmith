use std::fmt;

/// The numbers of a tool's version (`GNU bash, version 5.2.37(1)` is 5.2.37); never empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolVersion(Vec<u32>);

impl ToolVersion {
    /// The first dotted token that starts with a digit; a number too large for `u32` rejects the whole text.
    pub(crate) fn parse(text: &str) -> Option<ToolVersion> {
        let token = text
            .split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .find(|t| t.contains('.') && t.starts_with(|c: char| c.is_ascii_digit()))?;
        token
            .split('.')
            .filter(|part| !part.is_empty())
            .map(|part| part.parse().ok())
            .collect::<Option<Vec<u32>>>()
            .map(ToolVersion)
    }

    pub(crate) fn components(&self) -> &[u32] {
        &self.0
    }

    pub(crate) fn major(&self) -> u32 {
        self.0[0]
    }

    pub(crate) fn minor_or_zero(&self) -> u32 {
        self.0.get(1).copied().unwrap_or(0)
    }

    /// Lexicographic on the numbers, without padding zeros: 3.3 is below 3.3.0 and 4 is below 4.4.
    pub(crate) fn is_below(&self, bound: &[u32]) -> bool {
        self.0.as_slice() < bound
    }
}

impl fmt::Display for ToolVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let joined: Vec<String> = self.0.iter().map(u32::to_string).collect();
        f.write_str(&joined.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn components_of(text: &str) -> Option<Vec<u32>> {
        ToolVersion::parse(text).map(|version| version.components().to_vec())
    }

    #[test]
    fn should_read_the_numbers_of_a_bash_banner() {
        let parsed = components_of("GNU bash, version 3.2.57(1)-release (arm64-apple-darwin24)");

        assert_eq!(parsed, Some(vec![3, 2, 57]));
    }

    #[test]
    fn should_read_the_numbers_of_a_fish_banner() {
        let parsed = components_of("fish, version 3.7.1");

        assert_eq!(parsed, Some(vec![3, 7, 1]));
    }

    #[test]
    fn should_read_four_numbers_of_a_windows_version() {
        let parsed = components_of("Microsoft Windows [Version 10.0.19045.3803]");

        assert_eq!(parsed, Some(vec![10, 0, 19045, 3803]));
    }

    #[test]
    fn should_read_a_bare_version_with_a_newline() {
        let parsed = components_of("14.5\n");

        assert_eq!(parsed, Some(vec![14, 5]));
    }

    #[test]
    fn should_drop_the_empty_part_after_a_trailing_dot() {
        let parsed = components_of("3.");

        assert_eq!(parsed, Some(vec![3]));
    }

    #[test]
    fn should_reject_the_whole_text_when_a_number_overflows() {
        let parsed = components_of("99999999999.1");

        assert_eq!(parsed, None);
    }

    #[test]
    fn should_not_retry_a_later_token_after_an_overflow() {
        let parsed = components_of("99999999999.1 then 3.2");

        assert_eq!(parsed, None);
    }

    #[test]
    fn should_reject_a_text_without_a_dotted_number() {
        let parsed = components_of("version 5");

        assert_eq!(parsed, None);
    }

    #[test]
    fn should_use_the_first_dotted_token() {
        let parsed = components_of("tool 1.2 and 3.4");

        assert_eq!(parsed, Some(vec![1, 2]));
    }

    #[test]
    fn should_be_below_a_longer_bound_with_the_same_prefix() {
        let version = ToolVersion::parse("3.3").unwrap();

        assert!(version.is_below(&[3, 3, 0]));
    }

    #[test]
    fn should_be_below_a_bound_with_a_larger_second_number() {
        let version = ToolVersion::parse("4.").unwrap();

        assert!(version.is_below(&[4, 4]));
    }

    #[test]
    fn should_not_be_below_a_bound_it_equals_after_a_zero() {
        let version = ToolVersion::parse("4.4.0").unwrap();

        assert!(!version.is_below(&[4, 4]));
    }

    #[test]
    fn should_be_below_a_bound_with_a_larger_major() {
        let version = ToolVersion::parse("3.2.57").unwrap();

        assert!(version.is_below(&[4, 4]));
    }

    #[test]
    fn should_display_the_components_joined_by_dots() {
        let version = ToolVersion::parse("3.2.2").unwrap();

        assert_eq!(version.to_string(), "3.2.2");
    }

    #[test]
    fn should_give_zero_as_the_minor_of_a_major_only_version() {
        let version = ToolVersion::parse("4.").unwrap();

        assert_eq!((version.major(), version.minor_or_zero()), (4, 0));
    }

    #[test]
    fn should_give_the_second_number_as_the_minor() {
        let version = ToolVersion::parse("5.2.37").unwrap();

        assert_eq!((version.major(), version.minor_or_zero()), (5, 2));
    }

    #[test]
    fn doctor_version_parsing() {
        let version_of = |text: &str| ToolVersion::parse(text).map(|v| v.components().to_vec());
        assert_eq!(
            version_of("GNU bash, version 3.2.57(1)-release (arm64-apple-darwin24)"),
            Some(vec![3, 2, 57])
        );
        assert_eq!(version_of("fish, version 3.7.1"), Some(vec![3, 7, 1]));
        assert_eq!(
            version_of("Microsoft Windows [Version 10.0.19045.3803]"),
            Some(vec![10, 0, 19045, 3803])
        );
        assert_eq!(version_of("14.5\n"), Some(vec![14, 5]));
        assert!(
            [3, 2, 57].as_slice() < [4, 4].as_slice() && [4, 4, 0].as_slice() >= [4, 4].as_slice()
        );
    }
}
