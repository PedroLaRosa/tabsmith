/// A check of the system group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SystemCheck {
    Os,
    /// The bash program is probed through the parent path.
    Bash { program: String },
    Fish,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_compare_bash_checks_by_program() {
        let check = SystemCheck::Bash { program: "/bin/bash".to_string() };
        let other = SystemCheck::Bash { program: "/usr/bin/bash".to_string() };

        assert_ne!(check, other);
    }

    #[test]
    fn should_tell_the_system_checks_apart() {
        let bash = SystemCheck::Bash { program: "bash".to_string() };

        assert_eq!((SystemCheck::Os == SystemCheck::Fish, bash == SystemCheck::Os), (false, false));
    }
}
