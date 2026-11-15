use crate::modules::self_update::domain::value_objects::release_asset::ReleaseAsset;

/// A release's `SHA256SUMS`: one `<hex digest>  <file name>` line per asset, as `sha256sum` writes it (`<hex digest>
/// *<file name>` in binary mode).
pub(crate) struct ChecksumList {
    entries: Vec<(String, String)>,
}

impl ChecksumList {
    pub(crate) fn parse(text: &str) -> Self {
        ChecksumList {
            entries: text.lines().filter_map(file_and_digest).collect(),
        }
    }

    /// The hex digest listed for `asset`.
    pub(crate) fn digest_for(&self, asset: &ReleaseAsset) -> Option<&str> {
        self.entries
            .iter()
            .find(|(file, _)| file == asset.name())
            .map(|(_, digest)| digest.as_str())
    }
}

/// `<digest>  <file>` or `<digest> *<file>` as `(file, digest)`; any other line is not an entry.
fn file_and_digest(line: &str) -> Option<(String, String)> {
    let (digest, marked_file) = line.split_once(' ')?;
    let file = marked_file.strip_prefix([' ', '*'])?;
    Some((file.to_string(), digest.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn darwin_arm64() -> ReleaseAsset {
        ReleaseAsset::for_platform("macos", "aarch64").unwrap()
    }

    #[test]
    fn should_find_the_digest_of_an_asset_in_sha256sum_output() {
        let sums = ChecksumList::parse("aa11  tabsmith-linux-x64\nbb22  tabsmith-darwin-arm64\n");

        let digest = sums.digest_for(&darwin_arm64());

        assert_eq!(digest, Some("bb22"));
    }

    #[test]
    fn should_read_a_binary_mode_line() {
        let sums = ChecksumList::parse("bb22 *tabsmith-darwin-arm64\n");

        let digest = sums.digest_for(&darwin_arm64());

        assert_eq!(digest, Some("bb22"));
    }

    #[test]
    fn should_have_no_digest_for_an_asset_it_does_not_list() {
        let sums = ChecksumList::parse("aa11  tabsmith-linux-x64\n");

        let digest = sums.digest_for(&darwin_arm64());

        assert_eq!(digest, None);
    }
}
