/// The file of a release that holds the tabsmith binary for one platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseAsset {
    name: String,
}

impl ReleaseAsset {
    pub(crate) fn for_platform(os: &str, arch: &str) -> Option<Self> {
        let os = match os {
            "macos" => "darwin",
            _ => "linux",
        };
        let arch = match arch {
            "aarch64" => "arm64",
            _ => "x64",
        };
        Some(ReleaseAsset {
            name: format!("tabsmith-{os}-{arch}"),
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset_name(os: &str, arch: &str) -> Option<String> {
        ReleaseAsset::for_platform(os, arch).map(|asset| asset.name().to_string())
    }

    #[test]
    fn should_name_the_apple_silicon_asset_darwin_arm64() {
        let name = asset_name("macos", "aarch64");

        assert_eq!(name.as_deref(), Some("tabsmith-darwin-arm64"));
    }

    #[test]
    fn should_name_the_intel_linux_asset_linux_x64() {
        let name = asset_name("linux", "x86_64");

        assert_eq!(name.as_deref(), Some("tabsmith-linux-x64"));
    }
}
