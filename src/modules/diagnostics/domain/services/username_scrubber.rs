/// Hides the account name inside diagnostic text so a report can be shared.
///
/// The user name is replaced as a raw substring after a slash or a backslash,
/// with no segment boundary, so `/usr/sbin` becomes `/usr/USERin` for user `sb`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UsernameScrubber {
    user: String,
}

impl UsernameScrubber {
    pub(crate) fn new(user: String) -> Self {
        UsernameScrubber { user }
    }

    pub(crate) fn clean(&self, text: &str) -> String {
        if self.user.is_empty() {
            return text.to_string();
        }
        text.replace(&format!("/{}", self.user), "/USER")
            .replace(&format!("\\{}", self.user), "\\USER")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scrubber(user: &str) -> UsernameScrubber {
        UsernameScrubber::new(user.to_string())
    }

    #[test]
    fn should_leave_the_text_unchanged_when_the_user_is_empty() {
        let scrubber = scrubber("");

        let cleaned = scrubber.clean("/home/bob/x \\Users\\bob\\x");

        assert_eq!(cleaned, "/home/bob/x \\Users\\bob\\x");
    }

    #[test]
    fn should_replace_the_user_after_a_slash() {
        let scrubber = scrubber("bob");

        let cleaned = scrubber.clean("/home/bob/x");

        assert_eq!(cleaned, "/home/USER/x");
    }

    #[test]
    fn should_replace_the_user_after_a_backslash_on_every_platform() {
        let scrubber = scrubber("bob");

        let cleaned = scrubber.clean("\\Users\\bob\\x");

        assert_eq!(cleaned, "\\Users\\USER\\x");
    }

    #[test]
    fn should_replace_the_user_as_a_raw_prefix_of_a_longer_segment() {
        let scrubber = scrubber("bob");

        let cleaned = scrubber.clean("/home/bob/bobcat");

        assert_eq!(cleaned, "/home/USER/USERcat");
    }

    #[test]
    fn should_replace_the_user_at_the_start_of_a_longer_name() {
        let scrubber = scrubber("bob");

        let cleaned = scrubber.clean("/usr/bobby");

        assert_eq!(cleaned, "/usr/USERby");
    }

    #[test]
    fn should_rewrite_a_system_directory_that_starts_with_a_short_user_name() {
        let scrubber = scrubber("sb");

        let cleaned = scrubber.clean("/usr/sbin");

        assert_eq!(cleaned, "/usr/USERin");
    }

    #[test]
    fn should_replace_the_user_after_a_slash_and_after_a_backslash_in_one_text() {
        let scrubber = scrubber("bob");

        let cleaned = scrubber.clean("/home/bob;C:\\Users\\bob");

        assert_eq!(cleaned, "/home/USER;C:\\Users\\USER");
    }

    #[test]
    fn should_leave_a_text_without_the_user_unchanged() {
        let scrubber = scrubber("bob");

        let cleaned = scrubber.clean("/home/alice/x bob");

        assert_eq!(cleaned, "/home/alice/x bob");
    }
}
