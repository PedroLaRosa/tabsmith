use crate::self_update::domain::value_objects::release_asset::ReleaseAsset;

/// A release's `SHA256SUMS`: one `<hex digest>  <file name>` line per asset, as `sha256sum` writes it.
pub(crate) struct ChecksumList {
    entries: Vec<(String, String)>,
}

impl ChecksumList {
    pub(crate) fn parse(text: &str) -> Self {
        let entries = text
            .lines()
            .filter_map(|line| line.split_once("  "))
            .map(|(digest, file)| (file.to_string(), digest.to_string()))
            .collect();
        ChecksumList { entries }
    }

    /// The hex digest listed for `asset`.
    pub(crate) fn digest_for(&self, asset: &ReleaseAsset) -> Option<&str> {
        self.entries
            .iter()
            .find(|(file, _)| file == asset.name())
            .map(|(_, digest)| digest.as_str())
    }
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
}
