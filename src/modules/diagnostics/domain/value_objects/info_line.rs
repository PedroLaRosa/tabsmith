/// One line of a failed check's details; the reporter paints the `command` span when it prints.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InfoLine {
    Text(String),
    Command {
        before: String,
        command: String,
        after: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_keep_the_command_apart_from_the_text_around_it() {
        let line = InfoLine::Command {
            before: "Run ".into(),
            command: "tabsmith integrations install dotfiles bash".into(),
            after: " to reinstall".into(),
        };

        let plain = InfoLine::Text("Run tabsmith integrations install dotfiles bash to reinstall".into());

        assert_ne!(line, plain);
    }
}
