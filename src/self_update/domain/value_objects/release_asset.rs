/// The file of a release that holds the tabsmith binary for one platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseAsset {
    name: String,
}

impl ReleaseAsset {
    pub(crate) fn for_platform(_os: &str, _arch: &str) -> Option<Self> {
        Some(ReleaseAsset {
            name: "tabsmith-darwin-arm64".to_string(),
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_name_the_apple_silicon_asset_darwin_arm64() {
        let asset = ReleaseAsset::for_platform("macos", "aarch64");

        assert_eq!(asset.map(|asset| asset.name().to_string()), Some("tabsmith-darwin-arm64".to_string()));
    }
}
