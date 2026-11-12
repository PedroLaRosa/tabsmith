use std::cmp::Ordering;
use std::fmt;

/// A version as node-semver reads it (build metadata dropped). It is ordered by [`SemanticVersion::compare`] only,
/// never by a derived order: a release sorts after its prereleases and numeric identifiers before alphanumeric
/// ones. `Default` is 0.0.0, what an unparsable version compares as.
#[derive(Debug, Default)]
pub(crate) struct SemanticVersion {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<String>,
}

impl SemanticVersion {
    /// `MAJOR.MINOR.PATCH[-pre][+build]`, optional leading `v`/`=` (node-semver, non-loose).
    pub(crate) fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches(['=', 'v']);
        let s = s.split_once('+').map_or(s, |(v, _)| v);
        let (main, pre) = s.split_once('-').map_or((s, None), |(m, p)| (m, Some(p)));
        let num = |x: &str| {
            (!x.is_empty()
                && x.bytes().all(|c| c.is_ascii_digit())
                && (x == "0" || !x.starts_with('0')))
            .then(|| x.parse().ok())
            .flatten()
        };
        let mut it = main.split('.');
        let v = (num(it.next()?)?, num(it.next()?)?, num(it.next()?)?);
        let pre: Vec<String> = pre.map_or(Vec::new(), |p| p.split('.').map(String::from).collect());
        let ident_ok =
            |i: &String| !i.is_empty() && i.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-');
        (it.next().is_none() && pre.iter().all(ident_ok)).then_some(SemanticVersion {
            major: v.0,
            minor: v.1,
            patch: v.2,
            prerelease: pre,
        })
    }

    /// `semver.compare`.
    pub(crate) fn compare(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    let ident = |x: &String| x.parse::<u64>().map_or(Err(x.clone()), Ok); // numeric < alphanumeric
                    self.prerelease.iter().map(ident).cmp(other.prerelease.iter().map(ident))
                }
            })
    }
}

/// `semver.clean`'s normalised form: `MAJOR.MINOR.PATCH[-pre]`.
impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (a, b, c) = (self.major, self.minor, self.patch);
        if self.prerelease.is_empty() {
            write!(f, "{a}.{b}.{c}")
        } else {
            write!(f, "{a}.{b}.{c}-{}", self.prerelease.join("."))
        }
    }
}

/// `semver.clean`: the normalised version, or None.
pub(crate) fn semver_clean(s: &str) -> Option<String> {
    SemanticVersion::parse(s).map(|version| version.to_string())
}

/// `semver.compare`; unparsable versions sort as 0.0.0.
pub(crate) fn semver_cmp(a: &str, b: &str) -> Ordering {
    let parse = |s: &str| SemanticVersion::parse(s).unwrap_or_default();
    parse(a).compare(&parse(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_helpers() {
        assert_eq!(semver_clean("1.2.3\n").as_deref(), Some("1.2.3"));
        assert_eq!(semver_clean(" =v1.2.3 ").as_deref(), Some("1.2.3"));
        assert_eq!(
            semver_clean("1.2.3-beta.1+build.5").as_deref(),
            Some("1.2.3-beta.1")
        );
        for bad in [
            "heroku/8.1.0 darwin-x64",
            "1.2",
            "01.2.3",
            "1.2.3.4",
            "",
            "git version 2.39.0",
        ] {
            assert_eq!(semver_clean(bad), None, "{bad:?}");
        }
        assert_eq!(semver_cmp("1.10.0", "1.9.0"), Ordering::Greater);
        assert_eq!(semver_cmp("1.0.0-alpha", "1.0.0"), Ordering::Less);
        assert_eq!(
            semver_cmp("1.0.0-alpha.1", "1.0.0-alpha.beta"),
            Ordering::Less
        );
        assert_eq!(semver_cmp("1.0.0-2", "1.0.0-10"), Ordering::Less);
        assert_eq!(semver_cmp("1.0.0-alpha", "1.0.0-alpha.1"), Ordering::Less);
        assert_eq!(semver_cmp("2.53.0", "2.53.0"), Ordering::Equal);
    }
}
