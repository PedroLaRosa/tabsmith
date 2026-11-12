/// The file of a release that holds the tabsmith binary for one platform, named as Claude Code names its platforms:
/// `tabsmith-<os>-<arch>`, plus `.exe` on Windows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseAsset {
    name: String,
}

impl ReleaseAsset {
    /// The asset for `os` and `arch` as Rust names them (`std::env::consts::{OS, ARCH}`); `None` when no release
    /// carries a build for that platform.
    pub(crate) fn for_platform(os: &str, arch: &str) -> Option<Self> {
        let (os, extension) = release_os(os)?;
        let arch = release_arch(arch)?;
        Some(ReleaseAsset {
            name: format!("tabsmith-{os}-{arch}{extension}"),
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

/// The release's name for `os`, and the extension its binaries carry.
fn release_os(os: &str) -> Option<(&'static str, &'static str)> {
    match os {
        "macos" => Some(("darwin", "")),
        "linux" => Some(("linux", "")),
        "windows" => Some(("win32", ".exe")),
        _ => None,
    }
}

fn release_arch(arch: &str) -> Option<&'static str> {
    match arch {
        "aarch64" => Some("arm64"),
        "x86_64" => Some("x64"),
        _ => None,
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

    #[test]
    fn should_name_the_windows_asset_win32_with_the_exe_extension() {
        let name = asset_name("windows", "x86_64");

        assert_eq!(name.as_deref(), Some("tabsmith-win32-x64.exe"));
    }

    #[test]
    fn should_have_no_asset_for_an_operating_system_without_a_build() {
        let name = asset_name("freebsd", "x86_64");

        assert_eq!(name, None);
    }

    #[test]
    fn should_have_no_asset_for_an_architecture_without_a_build() {
        let name = asset_name("linux", "riscv64");

        assert_eq!(name, None);
    }
}
