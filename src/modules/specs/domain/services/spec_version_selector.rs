use crate::modules::specs::domain::value_objects::semantic_version::{semver_cmp, SemanticVersion};
use std::cmp::Ordering;

/// The versions whose diffs apply, in the order they apply: semver order (stable, so equal versions keep their
/// input order), every one `<= target`, cumulatively (all of them when `target` is None). Unparsable versions
/// count as 0.0.0.
pub(crate) fn versions_to_apply<'a>(
    versions: impl IntoIterator<Item = &'a str>,
    target: Option<&str>,
) -> Vec<&'a str> {
    let mut keys: Vec<&str> = versions.into_iter().collect();
    keys.sort_by(|a, b| semver_cmp(a, b));
    // ponytail: an unparsable target applies everything (JS semver would throw).
    let target = target.filter(|t| SemanticVersion::parse(t).is_some());
    keys.retain(|k| target.is_none_or(|t| semver_cmp(k, t) != Ordering::Greater));
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_apply_every_version_up_to_the_target_in_semantic_order() {
        let versions = ["9.0.0", "8.10.0", "8.2.0"];

        let applied = versions_to_apply(versions, Some("8.10.0"));

        assert_eq!(applied, ["8.2.0", "8.10.0"]);
    }

    #[test]
    fn should_apply_every_version_without_a_target() {
        let versions = ["9.0.0", "8.10.0", "8.2.0"];

        let applied = versions_to_apply(versions, None);

        assert_eq!(applied, ["8.2.0", "8.10.0", "9.0.0"]);
    }

    #[test]
    fn should_apply_every_version_when_the_target_is_not_a_version() {
        let versions = ["9.0.0", "8.10.0", "8.2.0"];

        let applied = versions_to_apply(versions, Some("garbage"));

        assert_eq!(applied, ["8.2.0", "8.10.0", "9.0.0"]);
    }

    #[test]
    fn should_count_an_unparsable_version_as_zero() {
        let versions = ["2.0.0", "junk", "0.1.0"];

        let applied = versions_to_apply(versions, Some("0.5.0"));

        assert_eq!(applied, ["junk", "0.1.0"]);
    }

    #[test]
    fn should_keep_equal_versions_in_their_input_order() {
        // 40 keys: below about 20, even sort_unstable keeps equal keys in order (it insertion-sorts).
        let builds = 0..20;
        let versions: Vec<String> = builds
            .clone()
            .flat_map(|build| [format!("2.0.0+{build}"), format!("v1.0.0+{build}")])
            .collect();

        let applied = versions_to_apply(versions.iter().map(String::as_str), None);

        let expected: Vec<String> = ["v1.0.0", "2.0.0"]
            .into_iter()
            .flat_map(|release| builds.clone().map(move |build| format!("{release}+{build}")))
            .collect();
        assert_eq!(applied, expected);
    }

    #[test]
    fn should_leave_out_the_release_of_a_prerelease_target() {
        let versions = ["1.0.0", "0.9.0"];

        let applied = versions_to_apply(versions, Some("1.0.0-beta"));

        assert_eq!(applied, ["0.9.0"]);
    }
}
