use std::path::PathBuf;

/// Where the running tabsmith binary is, and the user home it is judged against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallLocation {
    executable: PathBuf,
    user_home: PathBuf,
}

impl InstallLocation {
    pub(crate) fn new(executable: PathBuf, user_home: PathBuf) -> Self {
        InstallLocation { executable, user_home }
    }

    /// The binary is `<user home>/.local/bin/tabsmith` (`tabsmith.exe` on Windows), where the installers put it.
    pub(crate) fn is_native(&self) -> bool {
        let native_dir = self.user_home.join(".local").join("bin");
        ["tabsmith", "tabsmith.exe"].iter().any(|name| self.executable == native_dir.join(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(executable: &str) -> InstallLocation {
        InstallLocation::new(PathBuf::from(executable), PathBuf::from("/home/ana"))
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
}
