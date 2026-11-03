use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SharedError {
    HomeDirectoryNotFound,
    /// The user's own home (`HOME`, or `USERPROFILE` on Windows), not tabsmith's data directory.
    UserHomeNotFound,
}

impl fmt::Display for SharedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SharedError::HomeDirectoryNotFound => {
                f.write_str("could not determine the home directory (set HOME or TABSMITH_HOME)")
            }
            SharedError::UserHomeNotFound => f.write_str("could not determine the home directory"),
        }
    }
}

impl std::error::Error for SharedError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_an_unknown_user_home_without_the_tabsmith_home_hint() {
        let error = SharedError::UserHomeNotFound;

        let message = error.to_string();

        assert_eq!(message, "could not determine the home directory");
    }
}
