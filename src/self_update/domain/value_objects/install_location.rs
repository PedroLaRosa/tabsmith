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

    pub(crate) fn is_native(&self) -> bool {
        true
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
}
