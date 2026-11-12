use std::path::{Path, PathBuf};

/// Where the running tabsmith binary is, and the native install's directory it is judged against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallLocation {
    executable: PathBuf,
    native_dir: PathBuf,
}

impl InstallLocation {
    /// `<user home>/.local/bin`, where the installers put tabsmith.
    pub(crate) fn native_dir(user_home: &Path) -> PathBuf {
        user_home.join(".local").join("bin")
    }

    /// `native_dir` is [`InstallLocation::native_dir`] of the user's home, resolved the same way as `executable`.
    pub(crate) fn new(executable: PathBuf, native_dir: PathBuf) -> Self {
        InstallLocation { executable, native_dir }
    }

    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }

    /// The binary is `tabsmith` (`tabsmith.exe` on Windows) in the native directory.
    pub(crate) fn is_native(&self) -> bool {
        ["tabsmith", "tabsmith.exe"].iter().any(|name| self.executable == self.native_dir.join(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(executable: &str) -> InstallLocation {
        InstallLocation::new(PathBuf::from(executable), InstallLocation::native_dir(Path::new("/home/ana")))
    }

    #[test]
    fn should_be_native_in_the_local_bin_of_the_user_home() {
        let location = location("/home/ana/.local/bin/tabsmith");

        let native = location.is_native();

        assert!(native);
    }

    #[test]
    fn should_not_be_native_as_a_cargo_build_in_target() {
        let location = location("/home/ana/src/tabsmith/target/release/tabsmith");

        let native = location.is_native();

        assert!(!native);
    }

    #[test]
    fn should_be_native_as_the_windows_executable_in_the_local_bin_of_the_user_home() {
        let location = location("/home/ana/.local/bin/tabsmith.exe");

        let native = location.is_native();

        assert!(native);
    }

    #[test]
    fn should_not_be_native_as_a_cargo_install() {
        let location = location("/home/ana/.cargo/bin/tabsmith");

        let native = location.is_native();

        assert!(!native);
    }
}
