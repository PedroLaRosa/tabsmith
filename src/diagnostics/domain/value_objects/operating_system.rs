#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OperatingSystem {
    Linux,
    Macos,
    Windows,
    Other(String),
}

impl OperatingSystem {
    pub(crate) fn from_name(os: &str) -> Self {
        match os {
            "linux" => OperatingSystem::Linux,
            "macos" => OperatingSystem::Macos,
            "windows" => OperatingSystem::Windows,
            other => OperatingSystem::Other(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_name_the_three_supported_systems() {
        let systems = ["linux", "macos", "windows"].map(OperatingSystem::from_name);

        assert_eq!(
            systems,
            [OperatingSystem::Linux, OperatingSystem::Macos, OperatingSystem::Windows]
        );
    }

    #[test]
    fn should_keep_the_name_of_any_other_system() {
        let system = OperatingSystem::from_name("freebsd");

        assert_eq!(system, OperatingSystem::Other("freebsd".into()));
    }
}
