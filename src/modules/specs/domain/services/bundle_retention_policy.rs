use std::time::Duration;

/// A version dir unused this long is removed (sessions of an older binary may still read theirs).
const VERSION_STALE: Duration = Duration::from_secs(7 * 86400);
/// A `.tmp-<pid>` dir untouched this long is an extraction that died (one takes seconds).
const TMP_STALE: Duration = Duration::from_secs(600);

/// Whether cleanup removes the dir `name` that sits next to `current_version`, last used `age` ago. Only
/// tabsmith's own dirs go: a stem of dot-separated digit runs (`1.2.3`, or that with `.tmp-<pid>`) whose whole name
/// is not `current_version` (so a stale `<current>.tmp-N` goes), older than its limit. `user/` or any other dir
/// belongs to the user, and an unknown age keeps the dir.
pub(crate) fn is_stale(name: &str, current_version: &str, age: Option<Duration>) -> bool {
    let (stem, max) = match name.split_once(".tmp-") {
        Some((stem, _)) => (stem, TMP_STALE),
        None => (name, VERSION_STALE),
    };
    // ponytail: npm versions are `1.2.3`; a pre-release suffix would make its dirs look like the user's (kept)
    let ours = stem
        .split('.')
        .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    ours && name != current_version && age.is_some_and(|age| age > max)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Duration = Duration::from_secs(86400);

    #[test]
    fn should_remove_a_version_unused_for_longer_than_seven_days() {
        let age = Some(7 * DAY + Duration::from_secs(1));

        let stale = is_stale("1.0.0", "2.0.0", age);

        assert!(stale);
    }

    #[test]
    fn should_keep_a_version_unused_for_exactly_seven_days() {
        let age = Some(7 * DAY);

        let stale = is_stale("1.0.0", "2.0.0", age);

        assert!(!stale);
    }

    #[test]
    fn should_remove_a_stale_temp_dir_of_the_current_version() {
        let age = Some(Duration::from_secs(601));

        let stale = is_stale("2.0.0.tmp-42", "2.0.0", age);

        assert!(stale);
    }

    #[test]
    fn should_keep_the_current_version_however_old() {
        let age = Some(8 * DAY);

        let stale = is_stale("2.0.0", "2.0.0", age);

        assert!(!stale);
    }

    #[test]
    fn should_keep_the_dirs_tabsmith_did_not_create() {
        let names = ["user", "dev", "notes.tmp-1", "1.2.3-beta"];

        let stale = names.map(|name| is_stale(name, "2.0.0", Some(8 * DAY)));

        assert_eq!(stale, [false; 4]);
    }

    #[test]
    fn should_count_a_lone_number_as_a_version_dir() {
        let age = Some(8 * DAY);

        let stale = is_stale("5", "2.0.0", age);

        assert!(stale);
    }

    #[test]
    fn should_keep_a_dir_whose_age_is_unknown() {
        let age = None;

        let stale = is_stale("1.0.0", "2.0.0", age);

        assert!(!stale);
    }
}
